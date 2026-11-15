//! Zero-dependency OAuth2 PKCE (MyAnimeList) & Implicit Loopback (AniList) browser flows.

use crate::{
    api::{
        anilist::AniListClient,
        mal::{MalClient, MalTokenResponse},
    },
    config::TrackerProvider,
    error::{AppError, Result},
};
use rand::Rng;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::watch,
};

pub const OAUTH_CALLBACK_PORT: u16 = 41932;

pub fn mal_redirect_uri() -> String {
    format!("http://127.0.0.1:{}/mal/callback", OAUTH_CALLBACK_PORT)
}

pub fn anilist_redirect_uri() -> String {
    format!("http://127.0.0.1:{}/anilist/callback", OAUTH_CALLBACK_PORT)
}

/// Completed OAuth session details returned to the main UI event loop.
#[derive(Debug, Clone)]
pub struct OAuthSessionResult {
    pub provider: TrackerProvider,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub user_id: i64,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// Generate a 64-character URL-safe PKCE code_verifier (also valid as `plain` challenge on MAL).
pub fn generate_pkce_verifier() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::thread_rng();
    (0..64)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

pub fn generate_state_nonce() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

pub fn build_mal_authorize_url(client_id: &str, verifier: &str, state: &str) -> String {
    format!(
        "https://myanimelist.net/v1/oauth2/authorize?response_type=code&client_id={}&code_challenge={}&code_challenge_method=plain&state={}&redirect_uri={}",
        percent_encode(client_id),
        percent_encode(verifier),
        percent_encode(state),
        percent_encode(&mal_redirect_uri())
    )
}

pub fn build_anilist_authorize_url(client_id: &str) -> String {
    format!(
        "https://anilist.co/api/v2/oauth/authorize?client_id={}&response_type=token",
        percent_encode(client_id)
    )
}

/// Open the user's default system browser non-blockingly.
pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

/// Exchange an already-obtained MAL authorization code + PKCE verifier for live tokens + profile.
pub async fn exchange_mal_code(
    client_id: &str,
    code: &str,
    verifier: &str,
    now: i64,
) -> Result<OAuthSessionResult> {
    let http = reqwest::Client::new();
    let redirect = mal_redirect_uri();
    let resp = http
        .post("https://myanimelist.net/v1/oauth2/token")
        .form(&[
            ("client_id", client_id),
            ("code", code.trim()),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect.as_str()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "MAL OAuth code exchange failed ({}): {}",
            status, body
        )));
    }

    let tok: MalTokenResponse = resp.json().await?;
    let mal_client = MalClient::new(Some(client_id.to_string()), Some(tok.access_token.clone()));
    let me = mal_client.fetch_me().await?;

    Ok(OAuthSessionResult {
        provider: TrackerProvider::Mal,
        access_token: tok.access_token,
        refresh_token: tok.refresh_token,
        expires_at: tok.expires_in.map(|secs| now + secs),
        user_id: me.id,
        username: me.name,
        avatar_url: me.picture,
    })
}

/// Verify a manually pasted or loopback-captured AniList Bearer token via `Viewer`.
pub async fn verify_anilist_token(access_token: &str) -> Result<OAuthSessionResult> {
    let clean = access_token.trim().to_string();
    if clean.is_empty() {
        return Err(AppError::Auth("AniList access token cannot be empty.".into()));
    }
    let client = AniListClient::with_token(Some(clean.clone()));
    let viewer = client.viewer().await?;
    Ok(OAuthSessionResult {
        provider: TrackerProvider::AniList,
        access_token: clean,
        refresh_token: None,
        expires_at: None,
        user_id: viewer.id,
        username: viewer.name,
        avatar_url: viewer.avatar_url,
    })
}

/// Listen on `127.0.0.1:41932` until MAL redirects back with `?code=...`, then complete OAuth.
pub async fn await_mal_browser_oauth(
    client_id: String,
    verifier: String,
    expected_state: String,
    mut cancel_rx: watch::Receiver<bool>,
    now_fn: fn() -> i64,
) -> Result<OAuthSessionResult> {
    let listener = TcpListener::bind(("127.0.0.1", OAUTH_CALLBACK_PORT))
        .await
        .map_err(|e| {
            AppError::Auth(format!(
                "Could not bind local OAuth port {}: {}",
                OAUTH_CALLBACK_PORT, e
            ))
        })?;

    let deadline = tokio::time::sleep(std::time::Duration::from_secs(300));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                return Err(AppError::Auth("MAL browser sign-in timed out after 5 minutes.".into()));
            }
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    return Err(AppError::Auth("MAL sign-in cancelled.".into()));
                }
            }
            accepted = listener.accept() => {
                let (mut stream, _) = accepted?;
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = parse_request_target(&req).unwrap_or_default();

                if let Some((code_opt, state_opt, err_opt)) = parse_mal_callback_query(&path) {
                    if let Some(err) = err_opt {
                        write_html_response(&mut stream, "Authorization Denied", &format!("MyAnimeList returned: {}", err), false).await;
                        return Err(AppError::Auth(format!("MAL authorization denied: {}", err)));
                    }
                    if state_opt.as_deref() != Some(expected_state.as_str()) {
                        write_html_response(&mut stream, "State Mismatch", "Security state check failed. Please retry from Solus.", false).await;
                        return Err(AppError::Auth("MAL OAuth state mismatch.".into()));
                    }
                    if let Some(code) = code_opt {
                        write_html_response(&mut stream, "Connected to Solus", "MyAnimeList linked! You can close this browser tab now.", true).await;
                        return exchange_mal_code(&client_id, &code, &verifier, now_fn()).await;
                    }
                }
                write_html_response(&mut stream, "Solus OAuth", "Waiting for MyAnimeList callback...", true).await;
            }
        }
    }
}

/// Listen on `127.0.0.1:41932` until AniList redirects back and its inline fragment script posts `#access_token`.
pub async fn await_anilist_browser_oauth(
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<OAuthSessionResult> {
    let listener = TcpListener::bind(("127.0.0.1", OAUTH_CALLBACK_PORT))
        .await
        .map_err(|e| {
            AppError::Auth(format!(
                "Could not bind local OAuth port {}: {}",
                OAUTH_CALLBACK_PORT, e
            ))
        })?;

    let deadline = tokio::time::sleep(std::time::Duration::from_secs(300));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                return Err(AppError::Auth("AniList browser sign-in timed out.".into()));
            }
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    return Err(AppError::Auth("AniList sign-in cancelled.".into()));
                }
            }
            accepted = listener.accept() => {
                let (mut stream, _) = accepted?;
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = parse_request_target(&req).unwrap_or_default();

                if path.starts_with("/anilist/token?") {
                    if let Some(token) = extract_query_param(&path, "token") {
                        write_html_response(&mut stream, "Connected to Solus", "AniList connected! You can close this tab.", true).await;
                        return verify_anilist_token(&token).await;
                    }
                }

                // First landing from AniList implicit redirect has `#access_token=...` only in JS `window.location.hash`.
                let bridge = r#"<!doctype html><html><head><meta charset="utf-8"><title>Solus AniList Sync</title></head>
<body style="background:#0f1019;color:#ebeefa;font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
<div style="border:1px solid #bd93f9;border-radius:12px;padding:28px 36px;background:#161826;text-align:center">
<h2 style="color:#bd93f9;margin-top:0">Solus // AniList OAuth</h2>
<p id="msg">Completing secure authorization...</p>
</div>
<script>
const params = new URLSearchParams(window.location.hash.replace(/^#/, ''));
const tok = params.get('access_token');
if (tok) {
  fetch('/anilist/token?token=' + encodeURIComponent(tok))
    .then(() => { document.getElementById('msg').textContent = 'Connected to Solus! You can close this tab.'; })
    .catch(() => { document.getElementById('msg').textContent = 'Copy token to Solus: ' + tok; });
} else {
  document.getElementById('msg').textContent = 'No access_token found in redirect URL.';
}
</script></body></html>"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    bridge.len(),
                    bridge
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        }
    }
}

fn parse_request_target(req: &str) -> Option<String> {
    let first_line = req.lines().next()?;
    let mut parts = first_line.split_whitespace();
    let _method = parts.next()?;
    let target = parts.next()?;
    Some(target.to_string())
}

pub fn parse_mal_callback_query(target: &str) -> Option<(Option<String>, Option<String>, Option<String>)> {
    if !target.starts_with("/mal/callback") {
        return None;
    }
    let code = extract_query_param(target, "code");
    let state = extract_query_param(target, "state");
    let err = extract_query_param(target, "error");
    Some((code, state, err))
}

fn extract_query_param(target: &str, key: &str) -> Option<String> {
    let query = target.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(percent_decode(v));
            }
        }
    }
    None
}

async fn write_html_response(
    stream: &mut tokio::net::TcpStream,
    heading: &str,
    message: &str,
    ok: bool,
) {
    let accent = if ok { "#50fa7b" } else { "#ff5f5f" };
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head>\
<body style=\"background:#0f1019;color:#ebeefa;font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0\">\
<div style=\"border:1px solid {};border-radius:12px;padding:28px 36px;background:#161826;text-align:center\">\
<h2 style=\"color:{};margin-top:0\">Solus // {}</h2><p>{}</p></div></body></html>",
        heading, accent, accent, heading, message
    );
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(resp.as_bytes()).await;
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_and_mal_auth_url_format() {
        let verifier = generate_pkce_verifier();
        assert_eq!(verifier.len(), 64);
        let url = build_mal_authorize_url("my_client_123", &verifier, "abcSTATE");
        assert!(url.contains("https://myanimelist.net/v1/oauth2/authorize"));
        assert!(url.contains("client_id=my_client_123"));
        assert!(url.contains("code_challenge_method=plain"));
        assert!(url.contains("state=abcSTATE"));
        assert!(url.contains("http%3A%2F%2F127.0.0.1%3A41932%2Fmal%2Fcallback"));
    }

    #[test]
    fn test_parse_mal_callback_query_extracts_code_and_state() {
        let parsed = parse_mal_callback_query("/mal/callback?code=authCode99%2B1&state=nonceXYZ")
            .expect("should match /mal/callback");
        assert_eq!(parsed.0.as_deref(), Some("authCode99+1"));
        assert_eq!(parsed.1.as_deref(), Some("nonceXYZ"));
        assert_eq!(parsed.2, None);
    }
}

//! MyAnimeList REST v2 API client (plus public username fallback).

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

const MAL_API_BASE: &str = "https://api.myanimelist.net/v2";
const MAL_OAUTH_TOKEN_URL: &str = "https://myanimelist.net/v1/oauth2/token";
const MAL_MOBILE_AUTH_TOKEN_URL: &str = "https://api.myanimelist.net/v2/auth/token";

/// Authenticated MAL user summary from `/v2/users/@me`.
#[derive(Debug, Clone, Deserialize)]
pub struct MalUserInfo {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub picture: Option<String>,
}

/// OAuth2 token payload returned by MAL `/v1/oauth2/token`.
#[derive(Debug, Clone, Deserialize)]
pub struct MalTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
}

/// Normalized anime record fetched from MyAnimeList before `idMal -> id` crosswalk resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MalListEntry {
    pub mal_id: i64,
    pub title: String,
    pub title_english: Option<String>,
    pub status: String, // "CURRENT" | "COMPLETED" | "PLANNING" | "PAUSED" | "DROPPED"
    pub progress: u32,
    pub total_episodes: Option<i64>,
    pub score: Option<i64>, // normalized 0..=100
    pub format: Option<String>,
    pub season: Option<String>,
    pub season_year: Option<i64>,
    pub genres: Vec<String>,
    pub cover_url: Option<String>,
    pub updated_at: Option<i64>,
}

pub struct MalClient {
    http: reqwest::Client,
    client_id: Option<String>,
    access_token: Option<String>,
}

impl MalClient {
    pub fn new(client_id: Option<String>, access_token: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id: client_id
                .filter(|s| !s.trim().is_empty())
                .or_else(|| Some(crate::config::DEFAULT_MAL_CLIENT_ID.to_string())),
            access_token: access_token.filter(|s| !s.trim().is_empty()),
        }
    }

    pub fn with_token(client_id: Option<String>, token: Option<String>) -> Self {
        Self::new(client_id, token)
    }

    fn apply_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut req = req;
        if let Some(ref token) = self.access_token {
            req = req.header("Authorization", format!("Bearer {}", token.trim()));
        }
        if let Some(ref cid) = self.client_id {
            req = req.header("X-MAL-CLIENT-ID", cid.trim());
        }
        req
    }

    /// Fetch authenticated user profile (`/v2/users/@me`).
    pub async fn fetch_me(&self) -> Result<MalUserInfo> {
        let req = self.apply_headers(
            self.http
                .get(format!("{}/users/@me?fields=anime_statistics", MAL_API_BASE)),
        );
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "MyAnimeList profile request failed ({}): {}",
                code, body
            )));
        }
        Ok(resp.json::<MalUserInfo>().await?)
    }

    /// Refresh an expired or expiring MAL access token using `refresh_token`.
    pub async fn refresh_access_token(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<MalTokenResponse> {
        let resp = self
            .http
            .post(MAL_OAUTH_TOKEN_URL)
            .form(&[
                ("client_id", client_id),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "MAL token refresh failed ({}): {}",
                code, body
            )));
        }
        Ok(resp.json::<MalTokenResponse>().await?)
    }

    /// Sign in directly with a MyAnimeList username and password via official MAL mobile auth (`grant_type=password`).
    pub async fn login_with_password(&self, username: &str, password: &str) -> Result<MalTokenResponse> {
        let clean_user = username.trim();
        if clean_user.is_empty() || password.is_empty() {
            return Err(AppError::Auth(
                "MyAnimeList username and password cannot be empty.".into(),
            ));
        }

        let cid = self
            .client_id
            .as_deref()
            .unwrap_or(crate::config::DEFAULT_MAL_CLIENT_ID);

        let resp = self
            .http
            .post(MAL_MOBILE_AUTH_TOKEN_URL)
            .header("X-MAL-CLIENT-ID", cid)
            .form(&[
                ("client_id", cid),
                ("grant_type", "password"),
                ("username", clean_user),
                ("password", password),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if body.contains("invalid_grant") || body.contains("Incorrect username or password") {
                return Err(AppError::Auth(
                    "Incorrect MyAnimeList username or password. (Use your MAL username, not email; if you use Google/Apple sign-in or 2FA, press [o] Browser Login.)".into(),
                ));
            }
            return Err(AppError::Auth(format!(
                "MyAnimeList sign-in failed ({}): {}",
                code, body
            )));
        }
        Ok(resp.json::<MalTokenResponse>().await?)
    }

    /// Fetch full anime list for `"@me"` (OAuth / direct login) or any public username via official MAL API v2.
    pub async fn fetch_user_animelist(&self, username_or_me: &str) -> Result<Vec<MalListEntry>> {
        let target = username_or_me.trim();
        if target.is_empty() {
            return Err(AppError::Auth("MyAnimeList username cannot be empty.".into()));
        }

        if target == "@me" && self.access_token.is_none() {
            return Err(AppError::Auth(
                "Press [p] Password Login to sign in to MyAnimeList first.".into(),
            ));
        }

        self.fetch_official_animelist(target).await
    }

    async fn fetch_official_animelist(&self, username_or_me: &str) -> Result<Vec<MalListEntry>> {
        let mut next_url = Some(format!(
            "{}/users/{}/animelist?fields=list_status,num_episodes,media_type,mean,start_season,genres,status,main_picture,alternative_titles&limit=1000&nsfw=true",
            MAL_API_BASE, username_or_me
        ));
        let mut out = Vec::new();

        while let Some(url) = next_url.take() {
            let resp = self.apply_headers(self.http.get(&url)).send().await?;
            let status = resp.status();
            if status.as_u16() == 404 {
                return Err(AppError::Auth(format!(
                    "MyAnimeList user '{}' was not found, or their anime list is set to Private. Tip: Press [p] Password Login to sign in directly (works on private lists too!).",
                    username_or_me
                )));
            }
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(AppError::Auth(format!(
                    "MyAnimeList sync error ({}): {}",
                    status, body
                )));
            }

            let raw: serde_json::Value = resp.json().await?;
            out.extend(parse_official_mal_page(&raw));
            next_url = raw
                .pointer("/paging/next")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }

        Ok(out)
    }

    /// Update (or create) an anime entry on MyAnimeList (`PUT /v2/anime/{mal_id}/my_list_status`).
    pub async fn update_list_status(
        &self,
        mal_id: i64,
        status: &str,
        num_watched_episodes: Option<u32>,
        score_10: Option<u8>,
    ) -> Result<()> {
        if self.access_token.is_none() {
            return Err(AppError::Auth(
                "MyAnimeList writeback requires Browser OAuth2 authorization.".into(),
            ));
        }

        let mal_status = to_mal_status(status).to_string();
        let mut params: Vec<(&str, String)> = vec![("status", mal_status)];
        if let Some(ep) = num_watched_episodes {
            params.push(("num_watched_episodes", ep.to_string()));
        }
        if let Some(score) = score_10 {
            params.push(("score", score.min(10).to_string()));
        }

        let url = format!("{}/anime/{}/my_list_status", MAL_API_BASE, mal_id);
        let resp = self.apply_headers(self.http.put(&url).form(&params)).send().await?;
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "Failed to update MAL entry {} ({}): {}",
                mal_id, code, body
            )));
        }
        Ok(())
    }

    /// Remove an anime entry from MyAnimeList (`DELETE /v2/anime/{mal_id}/my_list_status`).
    pub async fn delete_list_entry(&self, mal_id: i64) -> Result<()> {
        if self.access_token.is_none() {
            return Err(AppError::Auth(
                "MyAnimeList delete requires Browser OAuth2 authorization.".into(),
            ));
        }
        let url = format!("{}/anime/{}/my_list_status", MAL_API_BASE, mal_id);
        let resp = self.apply_headers(self.http.delete(&url)).send().await?;
        if resp.status().as_u16() == 404 {
            return Ok(());
        }
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "Failed to delete MAL entry {} ({}): {}",
                mal_id, code, body
            )));
        }
        Ok(())
    }
}

pub fn normalize_mal_status(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "watching" | "current" => "CURRENT",
        "completed" => "COMPLETED",
        "plan_to_watch" | "plantowatch" | "planning" => "PLANNING",
        "on_hold" | "onhold" | "paused" => "PAUSED",
        "dropped" => "DROPPED",
        _ => "PLANNING",
    }
}

pub fn to_mal_status(solus_status: &str) -> &'static str {
    match solus_status.trim().to_ascii_uppercase().as_str() {
        "CURRENT" | "WATCHING" => "watching",
        "COMPLETED" => "completed",
        "PAUSED" | "ON_HOLD" => "on_hold",
        "DROPPED" => "dropped",
        _ => "plan_to_watch",
    }
}

pub fn parse_official_mal_page(raw: &serde_json::Value) -> Vec<MalListEntry> {
    let Some(items) = raw.get("data").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for item in items {
        let Some(node) = item.get("node") else {
            continue;
        };
        let Some(mal_id) = node.get("id").and_then(|v| v.as_i64()) else {
            continue;
        };
        let title = node
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Title")
            .to_string();
        let title_english = node
            .pointer("/alternative_titles/en")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string);

        let list_status = item.get("list_status");
        let status_raw = list_status
            .and_then(|ls| ls.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("plan_to_watch");
        let progress = list_status
            .and_then(|ls| {
                ls.get("num_episodes_watched")
                    .or_else(|| ls.get("num_watched_episodes"))
            })
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let score_10 = list_status
            .and_then(|ls| ls.get("score"))
            .and_then(|v| v.as_i64())
            .filter(|&s| s > 0);
        let updated_at = list_status
            .and_then(|ls| ls.get("updated_at"))
            .and_then(|v| v.as_str())
            .and_then(parse_iso8601_unix);

        let total_episodes = node
            .get("num_episodes")
            .and_then(|v| v.as_i64())
            .filter(|&n| n > 0);
        let format = node
            .get("media_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_ascii_uppercase());
        let season = node
            .pointer("/start_season/season")
            .and_then(|v| v.as_str())
            .map(|s| s.to_ascii_uppercase());
        let season_year = node
            .pointer("/start_season/year")
            .and_then(|v| v.as_i64());
        let cover_url = node
            .pointer("/main_picture/large")
            .or_else(|| node.pointer("/main_picture/medium"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let genres = node
            .get("genres")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| g.get("name").and_then(|n| n.as_str()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        out.push(MalListEntry {
            mal_id,
            title,
            title_english,
            status: normalize_mal_status(status_raw).to_string(),
            progress,
            total_episodes,
            score: score_10.map(|s| (s * 10).clamp(0, 100)),
            format,
            season,
            season_year,
            genres,
            cover_url,
            updated_at,
        });
    }
    out
}

/// Lightweight UTC RFC3339 parser (`YYYY-MM-DDTHH:MM:SS...`) returning Unix seconds.
pub fn parse_iso8601_unix(ts: &str) -> Option<i64> {
    if ts.len() < 19 {
        return None;
    }
    let y: i64 = ts.get(0..4)?.parse().ok()?;
    let m: i64 = ts.get(5..7)?.parse().ok()?;
    let d: i64 = ts.get(8..10)?.parse().ok()?;
    let hh: i64 = ts.get(11..13)?.parse().ok()?;
    let mm: i64 = ts.get(14..16)?.parse().ok()?;
    let ss: i64 = ts.get(17..19)?.parse().ok()?;

    // Howard Hinnant civil-from-days inverse for UTC seconds.
    let y_adj = y - if m <= 2 { 1 } else { 0 };
    let era = y_adj.div_euclid(400);
    let yoe = y_adj.rem_euclid(400);
    let m_adj = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    Some(days * 86_400 + hh * 3_600 + mm * 60 + ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_official_mal_animelist_page() {
        let json = serde_json::json!({
            "data": [
                {
                    "node": {
                        "id": 52991,
                        "title": "Sousou no Frieren",
                        "alternative_titles": { "en": "Frieren: Beyond Journey's End" },
                        "num_episodes": 28,
                        "media_type": "tv",
                        "start_season": { "year": 2023, "season": "fall" },
                        "main_picture": { "large": "https://cdn.myanimelist.net/frieren.jpg" },
                        "genres": [{ "id": 2, "name": "Adventure" }, { "id": 8, "name": "Drama" }]
                    },
                    "list_status": {
                        "status": "watching",
                        "score": 9,
                        "num_episodes_watched": 15,
                        "updated_at": "2026-11-10T12:00:00+00:00"
                    }
                },
                {
                    "node": {
                        "id": 9253,
                        "title": "Steins;Gate",
                        "num_episodes": 24,
                        "media_type": "tv"
                    },
                    "list_status": {
                        "status": "plan_to_watch",
                        "score": 0,
                        "num_watched_episodes": 7
                    }
                }
            ],
            "paging": {}
        });

        let parsed = parse_official_mal_page(&json);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].mal_id, 52991);
        assert_eq!(
            parsed[0].title_english.as_deref(),
            Some("Frieren: Beyond Journey's End")
        );
        assert_eq!(parsed[0].status, "CURRENT");
        assert_eq!(parsed[0].progress, 15);
        assert_eq!(parsed[0].score, Some(90));
        assert!(parsed[0].updated_at.is_some());
        assert_eq!(parsed[1].mal_id, 9253);
        assert_eq!(parsed[1].status, "PLANNING");
        assert_eq!(parsed[1].progress, 7);
        assert_eq!(parsed[1].score, None);
    }

    #[test]
    fn test_mal_status_mapping_roundtrip() {
        assert_eq!(normalize_mal_status("watching"), "CURRENT");
        assert_eq!(normalize_mal_status("plan_to_watch"), "PLANNING");
        assert_eq!(normalize_mal_status("on_hold"), "PAUSED");
        assert_eq!(to_mal_status("CURRENT"), "watching");
        assert_eq!(to_mal_status("PLANNING"), "plan_to_watch");
        assert_eq!(to_mal_status("PAUSED"), "on_hold");
    }

    #[test]
    fn test_iso8601_unix_conversion() {
        // 2026-11-15T00:00:00Z
        let ts = parse_iso8601_unix("2026-11-15T00:00:00+00:00").unwrap();
        assert!(ts > 1_790_000_000);
    }
}

//! Zero-Latency Local LAN Mobile Remote Companion v2.0 (`solus remote`).
//! Serves an embedded, dependency-free mobile touch client over local Wi-Fi,
//! provides persistent WebSocket bidirectional event delivery (< 1ms latency),
//! renders terminal ASCII QR codes, and manages multi-track subtitle and episode switching.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

#[cfg(unix)]
type IpcStream = tokio::net::UnixStream;

#[cfg(windows)]
type IpcStream = tokio::net::windows::named_pipe::NamedPipeClient;

#[cfg(not(any(unix, windows)))]
type IpcStream = tokio::io::DuplexStream;

/// Individual subtitle track representation for the mobile companion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteSubtitleTrack {
    pub id: u32,
    pub code: String,
    pub label: String,
    pub is_active: bool,
    #[serde(default)]
    pub url: Option<String>,
}

/// Individual audio / dub language track representation for the mobile companion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteAudioTrack {
    pub id: u32,
    pub code: String,
    pub label: String,
    pub is_active: bool,
    #[serde(default)]
    pub is_dub: bool,
    #[serde(default)]
    pub mpv_aid: Option<u32>,
}

/// Format a consistent display label for an audio track and indicate whether it is a dub (`true`) or original Japanese (`false`).
pub fn format_audio_track_label(code: &str, base_name: &str) -> (String, bool) {
    let lower_code = code.to_lowercase();
    if lower_code.starts_with("jpn") || lower_code == "ja" {
        ("Japanese (Original / 日本語)".to_string(), false)
    } else if lower_code.starts_with("eng") || lower_code == "en" {
        ("English (Dub)".to_string(), true)
    } else {
        let trimmed = base_name.trim();
        let label = if trimmed.is_empty() {
            format!("{} (Dub)", code.to_uppercase())
        } else if trimmed.to_lowercase().contains("dub")
            || trimmed.to_lowercase().contains("original")
        {
            trimmed.to_string()
        } else {
            format!("{trimmed} (Dub)")
        };
        (label, true)
    }
}

/// Individual episode entry for the mobile episode switcher.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteEpisodeItem {
    pub ep: u32,
    pub title: String,
    pub is_watched: bool,
    pub is_current: bool,
}

/// Queued item exposed to the mobile companion Queue drawer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteQueueItem {
    pub index:   usize,
    pub title:   String,
    pub episode: u32,
    pub status:  String,
    pub quality: String,
    pub dub:     bool,
}

fn default_remote_true() -> bool {
    true
}

/// Shared live playback state visible to the mobile companion.
/// Connected mobile WebSocket client count for TUI Settings & Telemetry display.
static CONNECTED_WS_CLIENTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Return the number of currently connected mobile companion WebSocket clients.
pub fn connected_client_count() -> usize {
    CONNECTED_WS_CLIENTS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Curated mobile remote visual theme metadata shared between Settings Control Hub and Web UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteThemeMeta {
    pub id:              &'static str,
    pub name:            &'static str,
    pub genre:           &'static str,
    pub icon_pack:       &'static str,
    pub shape_style:     &'static str,
    pub default_art_idx: usize,
    pub swatch_bg:       (u8, u8, u8),
    pub swatch_accent:   (u8, u8, u8),
}

/// Twelve distinct multi-genre themes spanning Y2K, Anime/Manga, Gaming, Zen, and Hi-Fi Deck.
pub const REMOTE_THEMES: [RemoteThemeMeta; 12] = [
    RemoteThemeMeta {
        id: "lain-wired",
        name: "Wired CRT",
        genre: "Y2K",
        icon_pack: "wired",
        shape_style: "chamfer",
        default_art_idx: 0,
        swatch_bg: (6, 11, 8),
        swatch_accent: (110, 231, 135),
    },
    RemoteThemeMeta {
        id: "chrome-y2k",
        name: "Chrome 2000",
        genre: "Y2K",
        icon_pack: "chrome",
        shape_style: "bezel",
        default_art_idx: 4,
        swatch_bg: (15, 20, 32),
        swatch_accent: (125, 211, 252),
    },
    RemoteThemeMeta {
        id: "neon-cat",
        name: "Shinjuku Synth",
        genre: "Y2K",
        icon_pack: "wired",
        shape_style: "bezel",
        default_art_idx: 5,
        swatch_bg: (14, 8, 26),
        swatch_accent: (244, 114, 182),
    },
    RemoteThemeMeta {
        id: "shibuya-punk",
        name: "Shibuya Punk",
        genre: "Anime",
        icon_pack: "comic",
        shape_style: "comic",
        default_art_idx: 6,
        swatch_bg: (246, 241, 232),
        swatch_accent: (239, 35, 60),
    },
    RemoteThemeMeta {
        id: "manga-ink",
        name: "Manga Ink",
        genre: "Anime",
        icon_pack: "comic",
        shape_style: "comic",
        default_art_idx: 2,
        swatch_bg: (250, 249, 246),
        swatch_accent: (24, 24, 27),
    },
    RemoteThemeMeta {
        id: "akira-tokyo",
        name: "Neo-Tokyo 88",
        genre: "Anime",
        icon_pack: "mecha",
        shape_style: "chamfer",
        default_art_idx: 3,
        swatch_bg: (18, 8, 10),
        swatch_accent: (244, 63, 94),
    },
    RemoteThemeMeta {
        id: "nerv-hud",
        name: "Tactical HUD",
        genre: "Gaming",
        icon_pack: "mecha",
        shape_style: "chamfer",
        default_art_idx: 7,
        swatch_bg: (12, 9, 6),
        swatch_accent: (251, 146, 60),
    },
    RemoteThemeMeta {
        id: "arcade-16bit",
        name: "Arcade 16-Bit",
        genre: "Gaming",
        icon_pack: "pixel",
        shape_style: "pixel",
        default_art_idx: 8,
        swatch_bg: (19, 13, 46),
        swatch_accent: (250, 204, 21),
    },
    RemoteThemeMeta {
        id: "pocket-dmg",
        name: "Pocket DMG",
        genre: "Gaming",
        icon_pack: "pixel",
        shape_style: "pixel",
        default_art_idx: 9,
        swatch_bg: (202, 220, 159),
        swatch_accent: (48, 98, 48),
    },
    RemoteThemeMeta {
        id: "kyoto-sumi",
        name: "Kyoto Sumi",
        genre: "Zen",
        icon_pack: "zen",
        shape_style: "zen",
        default_art_idx: 10,
        swatch_bg: (17, 22, 21),
        swatch_accent: (134, 189, 162),
    },
    RemoteThemeMeta {
        id: "washi-zen",
        name: "Washi Paper",
        genre: "Zen",
        icon_pack: "zen",
        shape_style: "zen",
        default_art_idx: 4,
        swatch_bg: (244, 239, 230),
        swatch_accent: (166, 75, 52),
    },
    RemoteThemeMeta {
        id: "cassette-hifi",
        name: "Hi-Fi Deck",
        genre: "Hi-Fi",
        icon_pack: "deck",
        shape_style: "deck",
        default_art_idx: 1,
        swatch_bg: (20, 18, 16),
        swatch_accent: (245, 158, 11),
    },
];

/// Look up a theme by id (falling back to the default `lain-wired` theme if unknown).
pub fn find_remote_theme(id: &str) -> &'static RemoteThemeMeta {
    let trimmed = id.trim();
    REMOTE_THEMES
        .iter()
        .find(|t| t.id.eq_ignore_ascii_case(trimmed))
        .unwrap_or(&REMOTE_THEMES[0])
}

/// Advance to the next or previous theme id across the 12 curated remote themes.
pub fn cycle_remote_theme_id(current_id: &str, forward: bool) -> &'static RemoteThemeMeta {
    let idx = REMOTE_THEMES
        .iter()
        .position(|t| t.id.eq_ignore_ascii_case(current_id.trim()))
        .unwrap_or(0);
    let next_idx = if forward {
        (idx + 1) % REMOTE_THEMES.len()
    } else {
        (idx + REMOTE_THEMES.len() - 1) % REMOTE_THEMES.len()
    };
    &REMOTE_THEMES[next_idx]
}

/// Normalize character art presentation mode for the mobile companion.
pub fn normalize_art_mode(mode: &str) -> &'static str {
    match mode.trim().to_ascii_lowercase().as_str() {
        "blend" => "blend",
        "poster" => "poster",
        "off" | "none" => "off",
        _ => "companion",
    }
}

/// Cycle through character art presentation modes (`companion` -> `blend` -> `poster` -> `off`).
pub fn cycle_remote_art_mode(current: &str) -> &'static str {
    match normalize_art_mode(current) {
        "companion" => "blend",
        "blend" => "poster",
        "poster" => "off",
        _ => "companion",
    }
}

fn default_network_mode_str() -> String {
    "normal".to_string()
}

fn default_network_tag_str() -> String {
    "NET:FULL".to_string()
}

fn default_remote_theme_id() -> String {
    "lain-wired".to_string()
}

fn default_remote_art_mode() -> String {
    "companion".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePlaybackState {
    pub is_playing: bool,
    pub title: String,
    pub episode: u32,
    pub episode_title: String,
    pub poster_url: Option<String>,
    pub time_pos: f64,
    pub duration: f64,
    pub volume: i32,
    pub speed: f64,
    pub audio_mode: String,
    #[serde(default)]
    pub audio_lang: String,
    #[serde(default)]
    pub audio_tracks: Vec<RemoteAudioTrack>,
    pub sub_active: bool,
    pub sub_lang: String,
    pub sub_delay: f64,
    pub sub_preset: String,
    pub subtitles: Vec<RemoteSubtitleTrack>,
    pub episodes: Vec<RemoteEpisodeItem>,
    pub anime4k: String,
    #[serde(default)]
    pub anime_id: Option<i64>,
    #[serde(default)]
    pub season_year: Option<i64>,
    #[serde(default)]
    pub era_badge: String,
    #[serde(default)]
    pub active_profile: String,
    #[serde(default)]
    pub profile_icon: String,
    #[serde(default)]
    pub queue: Vec<RemoteQueueItem>,
    #[serde(default = "default_remote_true")]
    pub queue_auto_advance: bool,
    #[serde(default)]
    pub queue_loop: bool,
    #[serde(default = "default_network_mode_str")]
    pub network_mode: String,
    #[serde(default = "default_network_tag_str")]
    pub network_tag: String,
    #[serde(default = "default_remote_true")]
    pub auto_abr: bool,
    #[serde(default)]
    pub force_ipv4: bool,
    #[serde(default)]
    pub current_quality: String,
    #[serde(default)]
    pub buffer_secs: f64,
    #[serde(default)]
    pub cache_mb: f64,
    #[serde(default)]
    pub cache_speed_kbps: f64,
    #[serde(default)]
    pub paused_for_cache: bool,
    #[serde(default)]
    pub buffering_pct: u8,
    #[serde(default)]
    pub stall_count: u32,
    #[serde(default = "default_remote_theme_id")]
    pub remote_theme: String,
    #[serde(default = "default_remote_art_mode")]
    pub remote_art_mode: String,
    #[serde(default)]
    pub remote_art_index: usize,
}

impl Default for RemotePlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            title: "No Active Stream".to_string(),
            episode: 1,
            episode_title: "Idle".to_string(),
            poster_url: None,
            time_pos: 0.0,
            duration: 0.0,
            volume: 80,
            speed: 1.0,
            audio_mode: "sub".to_string(),
            audio_lang: "Japanese (Original / 日本語)".to_string(),
            audio_tracks: vec![
                RemoteAudioTrack {
                    id: 1,
                    code: "jpn".to_string(),
                    label: "Japanese (Original / 日本語)".to_string(),
                    is_active: true,
                    is_dub: false,
                    mpv_aid: Some(1),
                },
                RemoteAudioTrack {
                    id: 2,
                    code: "eng".to_string(),
                    label: "English (Dub)".to_string(),
                    is_active: false,
                    is_dub: true,
                    mpv_aid: Some(2),
                },
            ],
            sub_active: true,
            sub_lang: "eng".to_string(),
            sub_delay: 0.0,
            sub_preset: "Studio Minimal".to_string(),
            subtitles: vec![
                RemoteSubtitleTrack {
                    id: 1,
                    code: "eng".to_string(),
                    label: "English (Official)".to_string(),
                    is_active: true,
                    url: None,
                },
                RemoteSubtitleTrack {
                    id: 2,
                    code: "ara".to_string(),
                    label: "Arabic / العربية".to_string(),
                    is_active: false,
                    url: None,
                },
                RemoteSubtitleTrack {
                    id: 3,
                    code: "spa".to_string(),
                    label: "Spanish / Español".to_string(),
                    is_active: false,
                    url: None,
                },
            ],
            episodes: Vec::new(),
            anime4k: "off".to_string(),
            anime_id: None,
            season_year: None,
            era_badge: "MODERN HD".to_string(),
            active_profile: "Default".to_string(),
            profile_icon: "◈".to_string(),
            queue: Vec::new(),
            queue_auto_advance: true,
            queue_loop: false,
            network_mode: "normal".to_string(),
            network_tag: "NET:FULL".to_string(),
            auto_abr: true,
            force_ipv4: false,
            current_quality: String::new(),
            buffer_secs: 0.0,
            cache_mb: 0.0,
            cache_speed_kbps: 0.0,
            paused_for_cache: false,
            buffering_pct: 0,
            stall_count: 0,
            remote_theme: "lain-wired".to_string(),
            remote_art_mode: "companion".to_string(),
            remote_art_index: 0,
        }
    }
}

/// Action payload sent from the mobile touch companion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteActionRequest {
    pub action: String,
    pub delta: Option<f64>,
    pub pos: Option<f64>,
    pub value: Option<i32>,
    #[serde(default)]
    pub audio_id: Option<u32>,
    pub sub_id: Option<u32>,
    pub sub_url: Option<String>,
    pub episode: Option<u32>,
    pub speed: Option<f64>,
    pub preset: Option<String>,
    pub mode: Option<String>,
    #[serde(default)]
    pub queue_index: Option<usize>,
    #[serde(default)]
    pub theme_id: Option<String>,
    #[serde(default)]
    pub art_mode: Option<String>,
    #[serde(default)]
    pub art_index: Option<usize>,
}

/// Resolves the host machine's primary local LAN IP address.
pub fn get_local_ip() -> String {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0");
    if let Ok(s) = socket {
        if s.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = s.local_addr() {
                let ip = addr.ip().to_string();
                if ip != "0.0.0.0" && ip != "127.0.0.1" {
                    return ip;
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

/// Generates a large, crisp terminal-renderable QR code (`2 cols x 1 row` per module) for roomy modals.
pub fn generate_terminal_qr(url: &str) -> Option<String> {
    let code = qrcode::QrCode::new(url.as_bytes()).ok()?;
    let w = code.width();
    let colors = code.to_colors();
    let quiet = 2usize;
    let total_w = w + quiet * 2;
    let mut out = String::with_capacity((total_w * 2 + 1) * (w + quiet * 2));
    for r in 0..(w + quiet * 2) {
        if r > 0 {
            out.push('\n');
        }
        for c in 0..total_w {
            let is_dark = r >= quiet
                && r < w + quiet
                && c >= quiet
                && c < w + quiet
                && colors[(r - quiet) * w + (c - quiet)] == qrcode::Color::Dark;
            out.push_str(if is_dark { "██" } else { "  " });
        }
    }
    Some(out)
}

/// Generates a compact terminal-renderable QR code using Unicode Dense1x2 half-blocks for smaller panes.
pub fn generate_compact_terminal_qr(url: &str) -> Option<String> {
    let code = qrcode::QrCode::new(url.as_bytes()).ok()?;
    let qr_str = code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Dark)
        .light_color(qrcode::render::unicode::Dense1x2::Light)
        .build();
    Some(qr_str)
}

/// Locates the active mpv IPC Unix domain socket or Windows named pipe, checking active registry, preferred paths, and /tmp sockets.
pub async fn resolve_active_socket(preferred: &str) -> Option<String> {
    #[cfg(unix)]
    {
        // 0. Process-wide active socket registered by ipc.rs / start_playback
        if let Some(active) = crate::services::ipc::get_active_socket_path() {
            if std::path::Path::new(&active).exists() {
                return Some(active);
            }
        }

        // 1. Direct path check
        if !preferred.is_empty() && std::path::Path::new(preferred).exists() {
            return Some(preferred.to_string());
        }

        // 2. Canonical symlink check
        if std::path::Path::new("/tmp/solus-mpv.sock").exists() {
            return Some("/tmp/solus-mpv.sock".to_string());
        }

        // 3. Scan /tmp for any active solus-mpv-*.sock files (newest by modified time)
        if let Ok(entries) = std::fs::read_dir("/tmp") {
            let mut socks: Vec<(std::time::SystemTime, String)> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with("solus-mpv") && name.ends_with(".sock")
                })
                .filter_map(|e| {
                    let meta = e.metadata().ok()?;
                    let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    Some((modified, e.path().to_string_lossy().to_string()))
                })
                .collect();

            socks.sort_by_key(|a| std::cmp::Reverse(a.0));
            if let Some((_, path)) = socks.first() {
                return Some(path.clone());
            }
        }

        None
    }
    #[cfg(windows)]
    {
        if let Some(active) = crate::services::ipc::get_active_socket_path() {
            if !active.trim().is_empty() {
                return Some(active);
            }
        }
        if !preferred.is_empty() {
            Some(preferred.to_string())
        } else {
            Some(r"\\.\pipe\solus-mpv".to_string())
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        if let Some(active) = crate::services::ipc::get_active_socket_path() {
            return Some(active);
        }
        if !preferred.is_empty() {
            Some(preferred.to_string())
        } else {
            None
        }
    }
}

async fn connect_remote_ipc(target_sock: &str) -> Option<IpcStream> {
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(target_sock).await.ok()
    }
    #[cfg(windows)]
    {
        for attempt in 0..5 {
            match tokio::net::windows::named_pipe::ClientOptions::new().open(target_sock) {
                Ok(client) => return Some(client),
                Err(e) if attempt < 4 && e.raw_os_error() == Some(231) => {
                    // ERROR_PIPE_BUSY (231): wait briefly and retry
                    tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
                }
                Err(_) => return None,
            }
        }
        None
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = target_sock;
        None
    }
}

/// Persistent connection cache for mpv socket / named pipe IPC.
static CACHED_IPC: Mutex<Option<(String, IpcStream)>> = Mutex::const_new(None);

/// Global broadcast channel for streaming state changes to all connected WebSockets.
static BROADCAST_CHANNEL: std::sync::OnceLock<broadcast::Sender<String>> = std::sync::OnceLock::new();

/// Access the broadcast sender for streaming playback state updates.
pub fn get_broadcast_sender() -> &'static broadcast::Sender<String> {
    BROADCAST_CHANNEL.get_or_init(|| {
        let (tx, _) = broadcast::channel(64);
        tx
    })
}

/// Broadcasts current state to all connected WebSocket clients.
pub async fn broadcast_state(state: &Arc<RwLock<RemotePlaybackState>>) {
    let st = state.read().await;
    if let Ok(json) = serde_json::to_string(&*st) {
        let _ = get_broadcast_sender().send(json);
    }
}

/// Dispatches an mpv JSON-RPC command over the active IPC monitor channel, falling back to direct socket / named pipe connection.
pub async fn send_mpv_json_command(preferred_sock: &str, cmd_value: serde_json::Value) -> bool {
    let payload = format!("{}\n", cmd_value);

    // Primary zero-overhead path: reuse the already-open IPC monitor writer (avoids Windows NPFS busy/name mismatches)
    if crate::services::ipc::try_send_via_active_monitor(&payload) {
        return true;
    }

    let active_override = crate::services::ipc::get_active_socket_path();
    let mut guard = CACHED_IPC.lock().await;

    // Reuse the cached stream only if it matches the active session socket/pipe
    if let Some((ref cached_sock, ref mut stream)) = *guard {
        let matches_active = active_override
            .as_ref()
            .is_none_or(|active| active == cached_sock);
        if matches_active
            && tokio::time::timeout(
                tokio::time::Duration::from_millis(750),
                async {
                    stream.write_all(payload.as_bytes()).await?;
                    stream.flush().await
                },
            )
            .await
            .is_ok_and(|result| result.is_ok())
        {
            return true;
        }
        *guard = None;
    }
    drop(guard);

    // Resolve the socket only after the fast path fails, avoiding a /tmp scan
    // on every healthy command.
    let target_sock = match resolve_active_socket(preferred_sock).await {
        Some(s) => s,
        None => return false,
    };

    if let Some(mut stream) = connect_remote_ipc(&target_sock).await {
        let ok = tokio::time::timeout(
            tokio::time::Duration::from_millis(750),
            async {
                stream.write_all(payload.as_bytes()).await?;
                stream.flush().await
            },
        )
        .await
        .is_ok_and(|result| result.is_ok());
        if ok {
            let mut guard = CACHED_IPC.lock().await;
            *guard = Some((target_sock, stream));
        }
        return ok;
    }

    false
}

/// Sends `quit` to the currently connected player and drops the cached IPC stream so the
/// next command never targets a dead socket. Used for instant episode switching.
pub async fn quit_active_player() {
    let sent_via_monitor =
        crate::services::ipc::try_send_via_active_monitor("{\"command\": [\"quit\"]}\n");
    crate::services::ipc::set_active_ipc_sender(None);

    let mut guard = CACHED_IPC.lock().await;
    if !sent_via_monitor {
        if let Some((_, ref mut stream)) = *guard {
            let _ = tokio::time::timeout(
                tokio::time::Duration::from_millis(750),
                async {
                    stream.write_all(b"{\"command\": [\"quit\"]}\n").await?;
                    stream.flush().await
                },
            )
            .await;
        } else {
            drop(guard);
            if let Some(sock) = resolve_active_socket("").await {
                if let Some(mut st) = connect_remote_ipc(&sock).await {
                    let _ = tokio::time::timeout(
                        tokio::time::Duration::from_millis(750),
                        async {
                            st.write_all(b"{\"command\": [\"quit\"]}\n").await?;
                            st.flush().await
                        },
                    )
                    .await;
                }
            }
            guard = CACHED_IPC.lock().await;
        }
    }
    *guard = None;
    crate::services::ipc::set_active_socket_path(None);
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file("/tmp/solus-mpv.sock");
    }
}

/// Dispatches an mpv JSON-RPC string array command.
pub async fn send_mpv_command(sock_path: &str, command: &[&str]) -> bool {
    let cmd_json = serde_json::json!({
        "command": command
    });
    send_mpv_json_command(sock_path, cmd_json).await
}

/// Spawns the background HTTP & WebSocket server for the mobile remote companion.
pub fn spawn_remote_server(
    port: u16,
    state: Arc<RwLock<RemotePlaybackState>>,
    sock_path: String,
    shutdown: Arc<AtomicBool>,
    tx_app: Option<tokio::sync::mpsc::Sender<crate::AppMessage>>,
) {
    tokio::spawn(async move {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to bind Solus remote server on {}: {}", addr, e);
                return;
            }
        };

        while !shutdown.load(Ordering::Relaxed) {
            let (socket, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    continue;
                }
            };

            // Disable Nagle's algorithm so 30-byte WebSocket touch commands flush in <1ms
            let _ = socket.set_nodelay(true);

            let state_clone = Arc::clone(&state);
            let sock_path_clone = sock_path.clone();
            let tx_app_clone = tx_app.clone();

            tokio::spawn(async move {
                // Peek bytes to determine whether this is an HTTP request or a WebSocket upgrade
                let mut peek_buf = [0u8; 1024];
                let n = socket.peek(&mut peek_buf).await.unwrap_or(0);
                let peek_str = String::from_utf8_lossy(&peek_buf[..n]);

                if peek_str.contains("Upgrade: websocket") || peek_str.contains("upgrade: websocket") {
                    if let Ok(ws) = tokio_tungstenite::accept_async(socket).await {
                        handle_ws_client(ws, state_clone, sock_path_clone, tx_app_clone).await;
                    }
                } else {
                    handle_http_client(socket, state_clone, sock_path_clone, tx_app_clone).await;
                }
            });
        }
    });
}

/// Parse `/assets/art/{index}.jpg` into a bounded curated portrait slot index.
pub fn parse_art_index(path: &str) -> Option<usize> {
    let tail = path.strip_prefix("/assets/art/")?;
    let digits = tail.strip_suffix(".jpg").unwrap_or(tail);
    digits
        .parse::<usize>()
        .ok()
        .filter(|&i| i < crate::services::loading_art::Y2K_ART_COUNT)
}

/// Decrements active WS client count when a mobile companion disconnects or errors out.
struct WsClientGuard {
    tx_app: Option<tokio::sync::mpsc::Sender<crate::AppMessage>>,
}

impl Drop for WsClientGuard {
    fn drop(&mut self) {
        let prev = CONNECTED_WS_CLIENTS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        let remaining = prev.saturating_sub(1);
        if let Some(ref tx) = self.tx_app {
            let _ = tx.try_send(crate::AppMessage::RemoteClientsChanged(remaining));
        }
    }
}

/// Manages a live, persistent WebSocket client session.
async fn handle_ws_client(
    ws: WebSocketStream<TcpStream>,
    state: Arc<RwLock<RemotePlaybackState>>,
    sock_path: String,
    tx_app: Option<tokio::sync::mpsc::Sender<crate::AppMessage>>,
) {
    let connected = CONNECTED_WS_CLIENTS
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        .saturating_add(1);
    if let Some(ref tx) = tx_app {
        let _ = tx.send(crate::AppMessage::RemoteClientsChanged(connected)).await;
    }
    let _ws_guard = WsClientGuard {
        tx_app: tx_app.clone(),
    };

    let (mut ws_sender, mut ws_receiver) = ws.split();
    let mut broadcast_rx = get_broadcast_sender().subscribe();

    // 1. Push immediate full state to newly connected phone
    {
        let st = state.read().await;
        if let Ok(json) = serde_json::to_string(&*st) {
            let _ = ws_sender.send(Message::Text(json.into())).await;
        }
    }

    // 2. Event loop: bidirectional sub-millisecond dispatch
    loop {
        tokio::select! {
            // Incoming action from mobile touch UI
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str = text.as_str();
                        if let Ok(req) = serde_json::from_str::<RemoteActionRequest>(text_str) {
                            handle_remote_action(&req, &sock_path, &state, tx_app.as_ref()).await;
                            broadcast_state(&state).await;
                        }
                    }
                    Some(Ok(Message::Ping(p))) => {
                        let _ = ws_sender.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            // Outgoing playback progress / metadata update from Solus
            Ok(json_payload) = broadcast_rx.recv() => {
                if ws_sender.send(Message::Text(json_payload.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Fallback HTTP handler for initial page load, REST health checks, and non-WebSocket clients.
async fn handle_http_client(
    mut socket: TcpStream,
    state: Arc<RwLock<RemotePlaybackState>>,
    sock_path: String,
    tx_app: Option<tokio::sync::mpsc::Sender<crate::AppMessage>>,
) {
    let mut buffer = [0u8; 4096];
    let n = match socket.read(&mut buffer).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buffer[..n]);
    let mut lines = request.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let path = parts[1];

    if method == "GET" && (path == "/" || path == "/index.html") {
        let body = REMOTE_HTML;
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
            Content-Type: text/html; charset=utf-8\r\n\
            Content-Length: {}\r\n\
            Connection: close\r\n\
            \r\n{}",
            body.len(),
            body
        );
        let _ = socket.write_all(resp.as_bytes()).await;
    } else if method == "GET" && path.starts_with("/assets/art/") {
        if let Some(idx) = parse_art_index(path) {
            let (bytes, _tag) = crate::services::loading_art::raw_y2k_art(idx);
            let hdr = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nCache-Control: public, max-age=86400\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            let _ = socket.write_all(hdr.as_bytes()).await;
            let _ = socket.write_all(bytes).await;
        } else {
            let _ = socket
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
        }
    } else if method == "GET" && path == "/api/status" {
        let st = state.read().await;
        let json_bytes = serde_json::to_vec(&*st).unwrap_or_default();
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
            Content-Type: application/json\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Content-Length: {}\r\n\
            Connection: close\r\n\
            \r\n{}",
            json_bytes.len(),
            String::from_utf8_lossy(&json_bytes)
        );
        let _ = socket.write_all(resp.as_bytes()).await;
    } else if method == "POST" && path == "/api/action" {
        let body = if let Some(idx) = request.find("\r\n\r\n") {
            &request[idx + 4..]
        } else {
            ""
        };

        if let Ok(req) = serde_json::from_str::<RemoteActionRequest>(body) {
            handle_remote_action(&req, &sock_path, &state, tx_app.as_ref()).await;
            broadcast_state(&state).await;
        }

        let resp = "HTTP/1.1 200 OK\r\n\
            Content-Type: application/json\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Content-Length: 15\r\n\
            Connection: close\r\n\
            \r\n{\"status\":\"ok\"}";
        let _ = socket.write_all(resp.as_bytes()).await;
    } else if method == "OPTIONS" {
        let resp = "HTTP/1.1 204 No Content\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
            Access-Control-Allow-Headers: Content-Type\r\n\
            Connection: close\r\n\r\n";
        let _ = socket.write_all(resp.as_bytes()).await;
    } else {
        let resp = "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n";
        let _ = socket.write_all(resp.as_bytes()).await;
    }
}

/// Deterministically select an audio track by ID (via direct mpv `set_property aid` or dual-stream fallback).
async fn apply_audio_track_selection(
    audio_id: u32,
    sock_path: &str,
    state: &Arc<RwLock<RemotePlaybackState>>,
    tx_app: Option<&tokio::sync::mpsc::Sender<crate::AppMessage>>,
) {
    let target = {
        let st = state.read().await;
        st.audio_tracks.iter().find(|t| t.id == audio_id).cloned()
    };
    let Some(track) = target else {
        return;
    };

    if let Some(aid) = track.mpv_aid {
        let cmd = serde_json::json!({ "command": ["set_property", "aid", aid] });
        if send_mpv_json_command(sock_path, cmd).await {
            // Force mpv to flush and re-buffer the newly selected HLS audio rendition at the
            // exact current playback position so audio never cuts out until a manual seek.
            let _ = send_mpv_json_command(
                sock_path,
                serde_json::json!({ "command": ["seek", 0.0, "relative+exact"] }),
            )
            .await;

            let osd = format!("✦ Audio: {}", track.label);
            let _ = send_mpv_json_command(
                sock_path,
                serde_json::json!({ "command": ["show-text", osd, 2500] }),
            )
            .await;

            let mut st = state.write().await;
            for item in &mut st.audio_tracks {
                item.is_active = item.id == audio_id;
            }
            st.audio_lang = track.label;
            st.audio_mode = if track.is_dub { "dub" } else { "sub" }.to_string();
        }
    } else {
        {
            let mut st = state.write().await;
            for item in &mut st.audio_tracks {
                item.is_active = item.id == audio_id;
            }
            st.audio_lang = track.label;
            st.audio_mode = if track.is_dub { "dub" } else { "sub" }.to_string();
        }
        if let Some(tx) = tx_app {
            let _ = tx
                .send(crate::AppMessage::RemoteSwitchAudioStream(track.is_dub))
                .await;
        }
    }
}

/// Executes touch remote commands against mpv IPC with optimistic local state sync and app message relay.
async fn handle_remote_action(
    req: &RemoteActionRequest,
    sock_path: &str,
    state: &Arc<RwLock<RemotePlaybackState>>,
    tx_app: Option<&tokio::sync::mpsc::Sender<crate::AppMessage>>,
) {
    match req.action.as_str() {
        "play_pause" => {
            let cmd = serde_json::json!({ "command": ["cycle", "pause"] });
            if send_mpv_json_command(sock_path, cmd).await {
                let mut st = state.write().await;
                st.is_playing = !st.is_playing;
            }
        }
        "seek" => {
            let delta = req.delta.unwrap_or(10.0);
            let cmd = serde_json::json!({ "command": ["seek", delta, "relative"] });
            if send_mpv_json_command(sock_path, cmd).await {
                let mut st = state.write().await;
                st.time_pos = (st.time_pos + delta).max(0.0);
            }
        }
        "set_pos" => {
            if let Some(pos) = req.pos {
                let cmd = serde_json::json!({ "command": ["set_property", "time-pos", pos] });
                if send_mpv_json_command(sock_path, cmd).await {
                    let mut st = state.write().await;
                    st.time_pos = pos;
                }
            }
        }
        "volume" => {
            let delta = req.delta.unwrap_or(5.0);
            let cmd = serde_json::json!({ "command": ["add", "volume", delta] });
            if send_mpv_json_command(sock_path, cmd).await {
                let mut st = state.write().await;
                st.volume = (st.volume + delta as i32).clamp(0, 100);
            }
        }
        "set_volume" => {
            if let Some(val) = req.value {
                let clamped = val.clamp(0, 100);
                let cmd = serde_json::json!({ "command": ["set_property", "volume", clamped] });
                if send_mpv_json_command(sock_path, cmd).await {
                    let mut st = state.write().await;
                    st.volume = clamped;
                }
            }
        }
        "set_sub" => {
            if let Some(sub_id) = req.sub_id {
                let (target_url, target_code, target_label, anime_id, episode, is_playing) = {
                    let st = state.read().await;
                    let track = st.subtitles.iter().find(|s| s.id == sub_id);
                    (
                        track.and_then(|s| s.url.clone()).or_else(|| req.sub_url.clone()),
                        track.map(|s| s.code.clone()).unwrap_or_default(),
                        track.map(|s| s.label.clone()).unwrap_or_default(),
                        st.anime_id,
                        st.episode,
                        st.is_playing,
                    )
                };

                if target_code.eq_ignore_ascii_case("none") {
                    let sid_ok = send_mpv_json_command(
                        sock_path,
                        serde_json::json!({ "command": ["set_property", "sid", "no"] }),
                    )
                    .await;
                    let vis_ok = send_mpv_json_command(
                        sock_path,
                        serde_json::json!({ "command": ["set_property", "sub-visibility", "no"] }),
                    )
                    .await;
                    if !is_playing || sid_ok || vis_ok {
                        let mut st = state.write().await;
                        for s in &mut st.subtitles {
                            s.is_active = s.id == sub_id;
                        }
                        st.sub_active = false;
                        st.sub_lang = "No subtitles".to_string();
                        if let Some(tx) = tx_app {
                            let _ = tx
                                .send(crate::AppMessage::RemoteSubtitleSelectionChanged(
                                    crate::api::subtitles::SubtitleSelection::None,
                                ))
                                .await;
                        }
                    }
                    return;
                }

                let mut sub_applied = false;

                if let Some(ref url) = target_url {
                    if url.starts_with("http://") || url.starts_with("https://") {
                        let client = reqwest::Client::new();
                        let aid = anime_id.unwrap_or(0);
                        let (local_path, _) = crate::services::subsync::ensure_synchronized_subtitle(
                            &client,
                            url,
                            None,
                            None,
                            aid,
                            episode,
                            &target_code,
                        )
                        .await;

                        let title = if target_label.is_empty() { "Subtitle" } else { &target_label };
                        let lang = if target_code.is_empty() { "sub" } else { &target_code };
                        let cmd = serde_json::json!({
                            "command": ["sub-add", local_path, "select", title, lang]
                        });
                        sub_applied = send_mpv_json_command(sock_path, cmd).await;
                    } else if !url.is_empty() {
                        let title = if target_label.is_empty() { "Subtitle" } else { &target_label };
                        let lang = if target_code.is_empty() { "sub" } else { &target_code };
                        let cmd = serde_json::json!({
                            "command": ["sub-add", url, "select", title, lang]
                        });
                        sub_applied = send_mpv_json_command(sock_path, cmd).await;
                    }
                }

                if !sub_applied {
                    let cmd = serde_json::json!({ "command": ["set_property", "sid", sub_id] });
                    sub_applied = send_mpv_json_command(sock_path, cmd).await;
                }

                let vis_cmd = serde_json::json!({ "command": ["set_property", "sub-visibility", "yes"] });
                let vis_ok = send_mpv_json_command(sock_path, vis_cmd).await;

                if !is_playing || sub_applied || vis_ok {
                    let mut st = state.write().await;
                    let mut active_label = None;
                    for s in &mut st.subtitles {
                        s.is_active = s.id == sub_id;
                        if s.is_active {
                            active_label = Some(s.label.clone());
                        }
                    }
                    if let Some(lbl) = active_label {
                        st.sub_lang = lbl;
                    }
                    st.sub_active = true;
                    if let Some(tx) = tx_app {
                        let _ = tx
                            .send(crate::AppMessage::RemoteSubtitleSelectionChanged(
                                crate::api::subtitles::SubtitleSelection::Track {
                                    code: target_code,
                                    name: target_label,
                                    url: target_url,
                                },
                            ))
                            .await;
                    }
                }
            }
        }
        "toggle_sub_visibility" => {
            let cmd = serde_json::json!({ "command": ["cycle", "sub-visibility"] });
            if send_mpv_json_command(sock_path, cmd).await {
                let mut st = state.write().await;
                st.sub_active = !st.sub_active;
            }
        }
        "set_sub_delay" => {
            let delta_ms = (req.delta.unwrap_or(0.25) * 1000.0).round() as i32;
            let cur_ms = {
                let st = state.read().await;
                (st.sub_delay * 1000.0).round() as i32
            };
            let new_delay_ms = cur_ms + delta_ms;
            let new_delay_secs = new_delay_ms as f64 / 1000.0;
            let cmd = serde_json::json!({ "command": ["set_property", "sub-delay", new_delay_secs] });
            if send_mpv_json_command(sock_path, cmd).await {
                {
                    let mut st = state.write().await;
                    st.sub_delay = new_delay_secs;
                }
                if let Some(tx) = tx_app {
                    let _ = tx.send(crate::AppMessage::RemoteSubDelayChanged(new_delay_ms)).await;
                }
            }
        }
        "reset_sub_delay" => {
            let cmd = serde_json::json!({ "command": ["set_property", "sub-delay", 0.0] });
            if send_mpv_json_command(sock_path, cmd).await {
                {
                    let mut st = state.write().await;
                    st.sub_delay = 0.0;
                }
                if let Some(tx) = tx_app {
                    let _ = tx.send(crate::AppMessage::RemoteSubDelayChanged(0)).await;
                }
            }
        }
        "set_sub_preset" => {
            if let Some(ref preset_str) = req.preset {
                let preset = match preset_str.to_lowercase().as_str() {
                    s if s.contains("netflix") => crate::services::substyler::SubtitlePreset::NetflixClean,
                    s if s.contains("fansub") => crate::services::substyler::SubtitlePreset::AnimeFansub,
                    s if s.contains("oled") => crate::services::substyler::SubtitlePreset::OledPlate,
                    _ => crate::services::substyler::SubtitlePreset::StudioMinimal,
                };
                let style_cfg = crate::services::substyler::SubtitleStyleConfig::from_preset(preset);
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-font", style_cfg.font_family] })).await;
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-font-size", style_cfg.font_size] })).await;
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-border-size", style_cfg.border_size] })).await;
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-shadow-offset", style_cfg.shadow_offset] })).await;
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-margin-y", style_cfg.margin_y] })).await;
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-color", style_cfg.primary_color] })).await;
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-border-color", style_cfg.border_color] })).await;
                let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-ass-override", "force"] })).await;
                if style_cfg.backdrop_opacity > 0.0 {
                    let alpha_hex = format!("{:02X}", (style_cfg.backdrop_opacity * 255.0).round() as u8);
                    let back_color = format!("#{}000000", alpha_hex);
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-back-color", back_color] })).await;
                } else {
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "sub-back-color", "#00000000"] })).await;
                }
                let mut st = state.write().await;
                st.sub_preset = preset_str.clone();
            }
        }
        "set_anime4k" => {
            if let Some(ref mode_str) = req.mode {
                let mode = match mode_str.to_lowercase().as_str() {
                    s if s.contains("balanced") => crate::config::Anime4KMode::Balanced,
                    s if s.contains("ultra") => crate::config::Anime4KMode::Ultra,
                    _ => crate::config::Anime4KMode::Off,
                };
                let season_year = {
                    let st = state.read().await;
                    st.season_year
                };
                let era = crate::services::shaders::AnimeEra::from_year(season_year);
                let era_label = era.label(season_year);

                // 1. Clear all currently active GLSL shaders using mpv's runtime list command
                let clr_cmd = serde_json::json!({ "command": ["change-list", "glsl-shaders", "clr", ""] });
                let _ = send_mpv_json_command(sock_path, clr_cmd).await;

                // 2. Append each shader file individually for the active mode & era profile
                let shader_files = crate::services::shaders::build_shader_files(mode, season_year);
                for file_path in &shader_files {
                    let append_cmd = serde_json::json!({
                        "command": ["change-list", "glsl-shaders", "append", file_path]
                    });
                    let _ = send_mpv_json_command(sock_path, append_cmd).await;
                }

                // 3. Also set glsl-shaders property for players that inspect property directly
                let shader_arg = crate::services::shaders::build_shader_arg_for_year(mode, season_year).unwrap_or_default();
                let prop_cmd = serde_json::json!({ "command": ["set_property", "glsl-shaders", shader_arg] });
                let _ = send_mpv_json_command(sock_path, prop_cmd).await;

                // 4. Toggle hardware debanding & high-precision scaling alongside shaders
                if mode == crate::config::Anime4KMode::Off {
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "deband", "no"] })).await;
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "scale", "spline36"] })).await;
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "cscale", "spline36"] })).await;
                } else {
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "scale", "ewa_lanczossharp"] })).await;
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "cscale", "ewa_lanczossharp"] })).await;
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "deband", "yes"] })).await;
                    let iterations = if mode == crate::config::Anime4KMode::Ultra { 2 } else { 1 };
                    let _ = send_mpv_json_command(sock_path, serde_json::json!({ "command": ["set_property", "deband-iterations", iterations] })).await;
                }

                // 5. Show instant visual confirmation banner on the video screen with Era profile
                let osd_label = match mode {
                    crate::config::Anime4KMode::Off => "✦ Anime4K: OFF (Native Stream)".to_string(),
                    crate::config::Anime4KMode::Balanced => format!("✦ Anime4K: BALANCED 2x [{era_label} Profile]"),
                    crate::config::Anime4KMode::Ultra => format!("✦ Anime4K: ULTRA 4K [{era_label} Mastering]"),
                };
                let _ = send_mpv_json_command(
                    sock_path,
                    serde_json::json!({ "command": ["show-text", osd_label, 3000] }),
                )
                .await;

                if let Some(tx) = tx_app {
                    let _ = tx.send(crate::AppMessage::RemoteAnime4K(mode)).await;
                }
                let mut st = state.write().await;
                st.anime4k = mode.as_str().to_string();
                st.era_badge = era.short_badge().to_string();
            }
        }
        "select_episode" => {
            if let Some(ep) = req.episode {
                if let Some(tx) = tx_app {
                    let _ = tx.send(crate::AppMessage::RemotePlayEpisode(ep)).await;
                }
                let mut st = state.write().await;
                st.episode = ep;
                st.is_playing = true;
                for item in &mut st.episodes {
                    item.is_current = item.ep == ep;
                }
            }
        }
        "set_audio" => {
            if let Some(audio_id) = req.audio_id {
                apply_audio_track_selection(audio_id, sock_path, state, tx_app).await;
            }
        }
        "toggle_audio" => {
            let target_id = {
                let st = state.read().await;
                let active_is_dub = st
                    .audio_tracks
                    .iter()
                    .find(|t| t.is_active)
                    .map(|t| t.is_dub)
                    .unwrap_or(st.audio_mode == "dub");
                if active_is_dub {
                    st.audio_tracks
                        .iter()
                        .find(|t| !t.is_dub)
                        .or_else(|| st.audio_tracks.first())
                        .map(|t| t.id)
                } else {
                    st.audio_tracks
                        .iter()
                        .find(|t| t.is_dub && t.code.starts_with("eng"))
                        .or_else(|| st.audio_tracks.iter().find(|t| t.is_dub))
                        .map(|t| t.id)
                }
            };
            if let Some(id) = target_id {
                apply_audio_track_selection(id, sock_path, state, tx_app).await;
            }
        }
        "set_speed" => {
            if let Some(speed) = req.speed {
                let cmd = serde_json::json!({ "command": ["set_property", "speed", speed] });
                if send_mpv_json_command(sock_path, cmd).await {
                    let mut st = state.write().await;
                    st.speed = speed;
                }
            }
        }
        "skip_intro" => {
            let cmd = serde_json::json!({ "command": ["seek", 85.0, "relative"] });
            if send_mpv_json_command(sock_path, cmd).await {
                let mut st = state.write().await;
                st.time_pos = (st.time_pos + 85.0).max(0.0);
            }
        }
        "toggle_fullscreen" => {
            let cmd = serde_json::json!({ "command": ["cycle", "fullscreen"] });
            let _ = send_mpv_json_command(sock_path, cmd).await;
        }
        "queue_skip" => {
            if let Some(tx) = tx_app {
                let _ = tx.send(crate::AppMessage::RemoteQueueSkip).await;
            }
        }
        "queue_play" => {
            if let Some(idx) = req.queue_index {
                if let Some(tx) = tx_app {
                    let _ = tx.send(crate::AppMessage::RemoteQueuePlayIndex(idx)).await;
                }
            }
        }
        "queue_toggle_auto" => {
            {
                let mut st = state.write().await;
                st.queue_auto_advance = !st.queue_auto_advance;
            }
            if let Some(tx) = tx_app {
                let _ = tx.send(crate::AppMessage::RemoteQueueToggleAuto).await;
            }
        }
        "set_network_mode" => {
            if let Some(ref mode_raw) = req.mode {
                let mode = crate::config::NetworkMode::from_str_lossy(mode_raw);
                {
                    let mut st = state.write().await;
                    st.network_mode = mode.as_str().to_string();
                    st.network_tag = mode.short_tag().to_string();
                }
                if let Some(tx) = tx_app {
                    let _ = tx
                        .send(crate::AppMessage::RemoteNetworkModeChanged(
                            mode.as_str().to_string(),
                        ))
                        .await;
                }
            }
        }
        "cycle_network_mode" => {
            let next_mode = {
                let mut st = state.write().await;
                let next = crate::config::NetworkMode::from_str_lossy(&st.network_mode).next();
                st.network_mode = next.as_str().to_string();
                st.network_tag = next.short_tag().to_string();
                next
            };
            if let Some(tx) = tx_app {
                let _ = tx
                    .send(crate::AppMessage::RemoteNetworkModeChanged(
                        next_mode.as_str().to_string(),
                    ))
                    .await;
            }
        }
        "toggle_auto_abr" => {
            let enabled = {
                let mut st = state.write().await;
                st.auto_abr = !st.auto_abr;
                st.auto_abr
            };
            if let Some(tx) = tx_app {
                let _ = tx.send(crate::AppMessage::RemoteAutoAbrToggled(enabled)).await;
            }
        }
        "set_remote_theme" => {
            if let Some(ref theme_raw) = req.theme_id {
                let meta = find_remote_theme(theme_raw);
                let art_idx = req.art_index.map(|i| i % crate::services::loading_art::Y2K_ART_COUNT);
                {
                    let mut st = state.write().await;
                    st.remote_theme = meta.id.to_string();
                    if let Some(idx) = art_idx {
                        st.remote_art_index = idx;
                    }
                }
                if let Some(tx) = tx_app {
                    let _ = tx
                        .send(crate::AppMessage::RemoteThemeChanged {
                            theme_id: Some(meta.id.to_string()),
                            art_mode: None,
                            art_index: art_idx,
                        })
                        .await;
                }
            }
        }
        "set_remote_art" => {
            let new_mode = req.art_mode.as_deref().map(|m| normalize_art_mode(m).to_string());
            let new_idx = req
                .art_index
                .map(|i| i % crate::services::loading_art::Y2K_ART_COUNT);
            {
                let mut st = state.write().await;
                if let Some(ref m) = new_mode {
                    st.remote_art_mode = m.clone();
                }
                if let Some(idx) = new_idx {
                    st.remote_art_index = idx;
                }
            }
            if let Some(tx) = tx_app {
                let _ = tx
                    .send(crate::AppMessage::RemoteThemeChanged {
                        theme_id: None,
                        art_mode: new_mode,
                        art_index: new_idx,
                    })
                    .await;
            }
        }
        _ => {}
    }
}

/// Single-file zero-dependency responsive OLED mobile touch client v2.0.
const REMOTE_HTML: &str = include_str!("remote_html.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_local_ip_returns_valid_string() {
        let ip = get_local_ip();
        assert!(!ip.is_empty());
    }

    #[test]
    fn test_generate_terminal_qr_creates_dense_blocks() {
        let qr = generate_terminal_qr("http://192.168.1.50:8484");
        assert!(qr.is_some());
        let content = qr.unwrap();
        assert!(content.contains('█') || content.contains('▀') || content.contains('▄'));
    }

    #[test]
    fn test_remote_state_serialization() {
        let st = RemotePlaybackState {
            title: "Frieren".to_string(),
            episode: 14,
            time_pos: 120.0,
            duration: 1440.0,
            subtitles: vec![RemoteSubtitleTrack {
                id: 1,
                code: "eng".to_string(),
                label: "English".to_string(),
                is_active: true,
                url: None,
            }],
            episodes: vec![RemoteEpisodeItem {
                ep: 1,
                title: "Journey's End".to_string(),
                is_watched: true,
                is_current: false,
            }],
            ..Default::default()
        };

        let json = serde_json::to_string(&st).unwrap();
        assert!(json.contains("Frieren"));
        assert!(json.contains("1440.0"));
        assert!(json.contains("English"));
        assert!(json.contains("Journey's End"));
    }

    #[tokio::test]
    async fn test_handle_remote_action_set_anime4k() {
        let _guard = crate::services::ipc::test_ipc_lock().lock().await;
        let state = Arc::new(RwLock::new(RemotePlaybackState::default()));
        let req = RemoteActionRequest {
            action: "set_anime4k".to_string(),
            delta: None,
            pos: None,
            value: None,
            audio_id: None,
            sub_id: None,
            sub_url: None,
            episode: None,
            speed: None,
            preset: None,
            mode: Some("balanced".to_string()),
            queue_index: None,
            ..Default::default()
        };

        handle_remote_action(&req, "/tmp/nonexistent-solus-test.sock", &state, None).await;
        let st = state.read().await;
        assert_eq!(st.anime4k, "balanced 2x");
    }

    #[tokio::test]
    async fn test_handle_remote_action_set_sub_updates_active_track() {
        let _guard = crate::services::ipc::test_ipc_lock().lock().await;
        let state = Arc::new(RwLock::new(RemotePlaybackState::default()));
        let req = RemoteActionRequest {
            action: "set_sub".to_string(),
            delta: None,
            pos: None,
            value: None,
            audio_id: None,
            sub_id: Some(2),
            sub_url: None,
            episode: None,
            speed: None,
            preset: None,
            mode: None,
            queue_index: None,
            ..Default::default()
        };

        handle_remote_action(&req, "/tmp/nonexistent-solus-test.sock", &state, None).await;
        let st = state.read().await;
        assert_eq!(st.sub_lang, "Arabic / العربية");
        assert!(st.sub_active);
        assert!(st.subtitles[1].is_active);
        assert!(!st.subtitles[0].is_active);
    }

    #[tokio::test]
    async fn test_handle_remote_action_set_sub_preset() {
        let _guard = crate::services::ipc::test_ipc_lock().lock().await;
        let state = Arc::new(RwLock::new(RemotePlaybackState::default()));
        let req = RemoteActionRequest {
            action: "set_sub_preset".to_string(),
            delta: None,
            pos: None,
            value: None,
            audio_id: None,
            sub_id: None,
            sub_url: None,
            episode: None,
            speed: None,
            preset: Some("Netflix Clean".to_string()),
            mode: None,
            queue_index: None,
            ..Default::default()
        };

        handle_remote_action(&req, "/tmp/nonexistent-solus-test.sock", &state, None).await;
        let st = state.read().await;
        assert_eq!(st.sub_preset, "Netflix Clean");
    }

    #[tokio::test]
    async fn test_handle_remote_action_set_audio_switches_exact_mpv_aid_without_cycling() {
        let _guard = crate::services::ipc::test_ipc_lock().lock().await;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        crate::services::ipc::set_active_socket_path(Some(r"\\.\pipe\solus-mpv-154587-1".to_string()));
        crate::services::ipc::set_active_ipc_sender(Some(tx));

        let mut init_state = RemotePlaybackState::default();
        init_state.audio_tracks = vec![
            RemoteAudioTrack {
                id: 1,
                code: "jpn".to_string(),
                label: "Japanese (Original / 日本語)".to_string(),
                is_active: true,
                is_dub: false,
                mpv_aid: Some(1),
            },
            RemoteAudioTrack {
                id: 2,
                code: "eng".to_string(),
                label: "English (Dub)".to_string(),
                is_active: false,
                is_dub: true,
                mpv_aid: Some(4),
            },
            RemoteAudioTrack {
                id: 3,
                code: "spa-419".to_string(),
                label: "Spanish (Latin America) (Dub)".to_string(),
                is_active: false,
                is_dub: true,
                mpv_aid: Some(2),
            },
        ];
        let state = Arc::new(RwLock::new(init_state));

        let req_for = |audio_id: u32| RemoteActionRequest {
            action: "set_audio".to_string(),
            delta: None,
            pos: None,
            value: None,
            audio_id: Some(audio_id),
            sub_id: None,
            sub_url: None,
            episode: None,
            speed: None,
            preset: None,
            mode: None,
            queue_index: None,
            ..Default::default()
        };

        // Select English (aid 4), click English repeatedly, then switch back to Japanese (aid 1) and Spanish (aid 2)
        for &(audio_id, expected_aid, expected_mode) in &[
            (2u32, 4u64, "dub"),
            (2u32, 4u64, "dub"),
            (1u32, 1u64, "sub"),
            (3u32, 2u64, "dub"),
        ] {
            handle_remote_action(&req_for(audio_id), r"\\.\pipe\solus-mpv", &state, None).await;
            let cmd_line = rx.recv().await.unwrap();
            let seek_line = rx.recv().await.unwrap();
            let _osd_line = rx.recv().await.unwrap();
            let parsed: serde_json::Value = serde_json::from_str(cmd_line.trim()).unwrap();
            assert_eq!(parsed["command"], serde_json::json!(["set_property", "aid", expected_aid]));
            let parsed_seek: serde_json::Value = serde_json::from_str(seek_line.trim()).unwrap();
            assert_eq!(parsed_seek["command"], serde_json::json!(["seek", 0.0, "relative+exact"]));

            let st = state.read().await;
            assert_eq!(st.audio_mode, expected_mode);
            assert!(st.audio_tracks.iter().find(|t| t.id == audio_id).unwrap().is_active);
            assert_eq!(st.audio_tracks.iter().filter(|t| t.is_active).count(), 1);
        }

        crate::services::ipc::set_active_ipc_sender(None);
        crate::services::ipc::set_active_socket_path(None);
    }

    #[tokio::test]
    async fn test_send_mpv_json_command_routes_through_active_ipc_monitor() {
        let _guard = crate::services::ipc::test_ipc_lock().lock().await;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        crate::services::ipc::set_active_socket_path(Some(r"\\.\pipe\solus-mpv-154587-1".to_string()));
        crate::services::ipc::set_active_ipc_sender(Some(tx));

        let ok = send_mpv_json_command(
            r"\\.\pipe\solus-mpv",
            serde_json::json!({ "command": ["cycle", "pause"] }),
        )
        .await;
        assert!(ok);

        let delivered = rx.recv().await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(delivered.trim()).unwrap();
        assert_eq!(parsed["command"], serde_json::json!(["cycle", "pause"]));

        crate::services::ipc::set_active_ipc_sender(None);
        crate::services::ipc::set_active_socket_path(None);
    }

    #[tokio::test]
    async fn test_handle_remote_action_queue_toggle_and_serialization() {
        let _guard = crate::services::ipc::test_ipc_lock().lock().await;
        let mut init = RemotePlaybackState::default();
        init.queue.push(RemoteQueueItem {
            index: 0,
            title: "Frieren".to_string(),
            episode: 13,
            status: "READY".to_string(),
            quality: "1080p".to_string(),
            dub: false,
        });
        assert!(init.queue_auto_advance);

        let json = serde_json::to_string(&init).unwrap();
        assert!(json.contains("Frieren"));
        assert!(json.contains("READY"));

        let state = Arc::new(RwLock::new(init));
        let req = RemoteActionRequest {
            action: "queue_toggle_auto".to_string(),
            ..Default::default()
        };
        handle_remote_action(&req, "/tmp/nonexistent-solus-test.sock", &state, None).await;
        let st = state.read().await;
        assert!(!st.queue_auto_advance);
    }

    #[test]
    fn test_remote_themes_and_art_route_helpers() {
        assert_eq!(REMOTE_THEMES.len(), 12);
        assert_eq!(find_remote_theme("shibuya-punk").genre, "Anime");
        assert_eq!(find_remote_theme("unknown").id, "lain-wired");
        assert_eq!(cycle_remote_theme_id("cassette-hifi", true).id, "lain-wired");
        assert_eq!(cycle_remote_theme_id("lain-wired", false).id, "cassette-hifi");
        assert_eq!(parse_art_index("/assets/art/0.jpg"), Some(0));
        assert_eq!(parse_art_index("/assets/art/10.jpg"), Some(10));
        assert_eq!(parse_art_index("/assets/art/99.jpg"), None);
        assert_eq!(parse_art_index("/assets/art/bad"), None);
        assert!(REMOTE_HTML.contains("data-theme=\"lain-wired\""));
        assert!(REMOTE_HTML.contains("/assets/art/"));
    }

    #[tokio::test]
    async fn test_handle_remote_network_and_theme_actions() {
        let state = Arc::new(RwLock::new(RemotePlaybackState::default()));
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);

        handle_remote_action(
            &RemoteActionRequest {
                action: "set_network_mode".to_string(),
                mode: Some("congested_wifi".to_string()),
                ..Default::default()
            },
            "/tmp/nonexistent-solus-test.sock",
            &state,
            Some(&tx),
        )
        .await;
        assert_eq!(state.read().await.network_tag, "NET:WIFI");
        assert!(matches!(
            rx.try_recv(),
            Ok(crate::AppMessage::RemoteNetworkModeChanged(m)) if m == "congested_wifi"
        ));

        handle_remote_action(
            &RemoteActionRequest {
                action: "cycle_network_mode".to_string(),
                ..Default::default()
            },
            "/tmp/nonexistent-solus-test.sock",
            &state,
            Some(&tx),
        )
        .await;
        assert_eq!(state.read().await.network_tag, "NET:ECO");

        handle_remote_action(
            &RemoteActionRequest {
                action: "toggle_auto_abr".to_string(),
                ..Default::default()
            },
            "/tmp/nonexistent-solus-test.sock",
            &state,
            Some(&tx),
        )
        .await;
        assert!(!state.read().await.auto_abr);

        handle_remote_action(
            &RemoteActionRequest {
                action: "set_remote_theme".to_string(),
                theme_id: Some("arcade-16bit".to_string()),
                art_index: Some(8),
                ..Default::default()
            },
            "/tmp/nonexistent-solus-test.sock",
            &state,
            Some(&tx),
        )
        .await;
        assert_eq!(state.read().await.remote_theme, "arcade-16bit");
        assert_eq!(state.read().await.remote_art_index, 8);
    }
}

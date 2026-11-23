//! ani-cli subprocess wrapper.
//! Spawns ani-cli for async playback with log streaming.

use crate::error::{AppError, Result};
use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::{Child, Command};

/// Options passed to ani-cli when launching playback.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayOptions {
    /// Title to pass to ani-cli as the search query
    pub title:   String,
    /// Episode number to play
    pub episode: u32,
    /// Quality string e.g. "1080p", "720p", "best"
    pub quality: String,
    /// true = dub, false = sub
    pub dub:     bool,
    /// Preferred player passed through to ani-cli
    pub player:  String,
    /// Subtitle language label (e.g. "Arabic", "Russian", "Hindi", "Indonesian", "English")
    pub sub_lang: Option<String>,
    /// Direct subtitle .vtt URL to pass to player
    pub sub_url:  Option<String>,
    /// Direct verified stream URL (.m3u8) for 100% sync and instant launch
    pub stream_url: Option<String>,
    /// IPC socket path to connect Solus to mpv for live progress tracking and skip cues
    pub ipc_socket: Option<String>,
    /// Playback start position in seconds (for exact resume)
    pub start_pos:  Option<f64>,
    /// Optional Anime4K GLSL shaders argument string (colon-separated)
    pub glsl_shaders: Option<String>,
    /// Optional cinema-grade subtitle styling and collision configuration
    pub sub_style: Option<crate::services::substyler::SubtitleStyleConfig>,
    /// Optional Y2K loading screen image shown in `mpv` while the stream prefetches
    pub loading_image: Option<String>,
    /// Persistent subtitle synchronization delay in milliseconds (0 = no delay)
    #[serde(default)]
    pub sub_delay_ms: i32,
    /// Explicit "No subtitles" toggle (disables subtitle tracks across mpv/iina/vlc)
    #[serde(default)]
    pub subtitles_disabled: bool,
    /// Active network resilience mode shaping buffer cushions & HLS caps
    #[serde(default)]
    pub network_mode: crate::config::NetworkMode,
    /// Force IPv4 socket binding for player / ytdl network requests
    #[serde(default)]
    pub force_ipv4: bool,
    /// Launch external video player in fullscreen mode automatically
    #[serde(default)]
    pub auto_fullscreen: bool,
    /// Launch external video player in floating borderless Picture-in-Picture (PiP) corner mode
    #[serde(default)]
    pub auto_pip: bool,
}

/// Build the ani-cli command arguments for a given play request.
/// Pure function — separated from spawn so it can be tested without forking.
pub fn build_args(opts: &PlayOptions) -> Vec<String> {
    let effective_quality = if let Some(cap) = opts.network_mode.max_height_cap() {
        let q_lower = opts.quality.trim().to_ascii_lowercase();
        if q_lower == "best" {
            format!("{cap}p")
        } else if let Some(h) = crate::api::subtitles::parse_height_from_label(&q_lower) {
            if h > cap {
                format!("{cap}p")
            } else {
                opts.quality.clone()
            }
        } else {
            opts.quality.clone()
        }
    } else {
        opts.quality.clone()
    };

    let mut args = vec![
        "-S".to_string(),
        "1".to_string(),
        "-e".to_string(),
        opts.episode.to_string(),
        "-q".to_string(),
        effective_quality,
        "--exit-after-play".to_string(),
    ];

    // Dub flag
    if opts.dub {
        args.push("--dub".to_string());
    }

    // Title as final positional argument.
    // Note: Do NOT pass -L / sub_lang as command-line arguments to ani-cli because
    // ani-cli's argument loop treats unhandled flags as positional search terms,
    // which pollutes the search query (e.g. "-L Arabic <title>") and causes it to
    // select a completely different anime. Subtitle language is configured via the
    // ANI_CLI_SUB_LANG and ANI_CLI_PLAYER_FLAGS environment variables in spawn_async.
    args.push(opts.title.clone());

    args
}

/// Resolve the actual player command to give ani-cli.
///
/// Preference order:
/// - `mpv` stays the default when installed
/// - on macOS, `mpv` falls back to `iina` / `iina-cli` when `mpv` is unavailable
/// - `vlc` remains an explicit cross-platform option
pub fn resolve_player(preferred: &str) -> String {
    match preferred {
        "mpv" => {
            if command_exists("mpv") {
                #[cfg(windows)]
                if let Some(path) = windows_player_path("mpv") {
                    return path;
                }
                "mpv".to_string()
            } else if let Some(iina) = macos_iina_fallback() {
                iina
            } else if let Some(vlc) = macos_vlc_fallback() {
                vlc
            } else {
                "mpv".to_string()
            }
        }
        "iina" => macos_iina_fallback().unwrap_or_else(|| "iina".to_string()),
        "vlc" => {
            if let Some(vlc) = macos_vlc_fallback() {
                return vlc;
            }
            #[cfg(windows)]
            if let Some(path) = windows_player_path("vlc") {
                return path;
            }
            "vlc".to_string()
        }
        other => other.to_string(),
    }
}

/// Detect whether the relevant runtime dependencies are available on this machine.
pub fn detect_dependencies() -> (bool, bool, bool, bool) {
    let has_vlc = {
        #[cfg(target_os = "macos")]
        {
            command_exists("vlc") || macos_vlc_fallback().is_some()
        }
        #[cfg(windows)]
        {
            command_exists("vlc") || windows_player_path("vlc").is_some()
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            command_exists("vlc")
        }
    };
    (
        command_exists("ani-cli"),
        command_exists("mpv"),
        macos_iina_fallback().is_some(),
        has_vlc,
    )
}

fn command_exists(name: &str) -> bool {
    if let Some(path) = env::var_os("PATH") {
        let found = env::split_paths(&path).any(|dir| {
            #[cfg(unix)]
            {
                executable_exists(&dir.join(name))
            }
            #[cfg(windows)]
            {
                executable_exists(&dir.join(format!("{}.exe", name)))
                    || executable_exists(&dir.join(format!("{}.cmd", name)))
                    || executable_exists(&dir.join(format!("{}.bat", name)))
            }
            #[cfg(not(any(unix, windows)))]
            {
                executable_exists(&dir.join(name))
            }
        });
        if found {
            return true;
        }
    }

    #[cfg(target_os = "macos")]
    {
        for dir in ["/opt/homebrew/bin", "/usr/local/bin"] {
            if executable_exists(&PathBuf::from(dir).join(name)) {
                return true;
            }
        }
    }

    #[cfg(windows)]
    {
        if windows_fallback_exists(name) {
            return true;
        }
    }

    false
}

#[cfg(windows)]
fn windows_player_path(name: &str) -> Option<String> {
    let candidates: Vec<PathBuf> = match name {
        "vlc" => vec![
            PathBuf::from(r"C:\Program Files\VideoLAN\VLC\vlc.exe"),
            PathBuf::from(r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe"),
        ],
        "mpv" => {
            let mut list = Vec::new();
            if let Ok(local_app) = env::var("LOCALAPPDATA") {
                let local = PathBuf::from(local_app);
                list.push(local.join(r"Microsoft\WinGet\Links\mpv.exe"));
                list.push(local.join(r"Programs\mpv\mpv.exe"));
            }
            if let Ok(user_profile) = env::var("USERPROFILE") {
                let user = PathBuf::from(user_profile);
                list.push(user.join(r"scoop\shims\mpv.exe"));
                list.push(user.join(r"scoop\apps\mpv\current\mpv.exe"));
            }
            list.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin\mpv.exe"));
            list
        }
        _ => Vec::new(),
    };

    for path in candidates {
        if executable_exists(&path) {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(windows)]
fn find_windows_ani_cli() -> Option<PathBuf> {
    // 1. Search PATH entries for .cmd, .bat, or .exe
    if let Some(path) = env::var_os("PATH") {
        for dir in env::split_paths(&path) {
            for ext in &["cmd", "bat", "exe"] {
                let candidate = dir.join(format!("ani-cli.{}", ext));
                if executable_exists(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    // 2. WinGet links directory (%LOCALAPPDATA%\Microsoft\WinGet\Links)
    if let Ok(local_app) = env::var("LOCALAPPDATA") {
        let local = PathBuf::from(local_app);
        for sub in &[
            r"Microsoft\WinGet\Links\ani-cli.cmd",
            r"Microsoft\WinGet\Links\ani-cli.bat",
            r"Microsoft\WinGet\Links\ani-cli.exe",
        ] {
            let p = local.join(sub);
            if executable_exists(&p) {
                return Some(p);
            }
        }
    }

    // 3. Scoop shims directory (%USERPROFILE%\scoop\shims)
    if let Ok(user_profile) = env::var("USERPROFILE") {
        let user = PathBuf::from(user_profile);
        for sub in &[
            r"scoop\shims\ani-cli.cmd",
            r"scoop\shims\ani-cli.bat",
            r"scoop\shims\ani-cli.exe",
        ] {
            let p = user.join(sub);
            if executable_exists(&p) {
                return Some(p);
            }
        }
    }

    // 4. Chocolatey directory
    let choco_p = PathBuf::from(r"C:\ProgramData\chocolatey\bin\ani-cli.bat");
    if executable_exists(&choco_p) {
        return Some(choco_p);
    }

    None
}

#[cfg(windows)]
fn windows_fallback_exists(name: &str) -> bool {
    if windows_player_path(name).is_some() {
        return true;
    }
    if name == "ani-cli" {
        return find_windows_ani_cli().is_some();
    }
    false
}

fn executable_exists(path: &Path) -> bool {
    path.is_file()
}

fn macos_iina_fallback() -> Option<String> {
    if env::consts::OS != "macos" {
        return None;
    }

    let app_path = PathBuf::from("/Applications/IINA.app/Contents/MacOS/iina-cli");
    if executable_exists(&app_path) {
        return Some(app_path.to_string_lossy().into_owned());
    }

    if command_exists("iina") {
        return Some("iina".to_string());
    }

    None
}

fn macos_vlc_fallback() -> Option<String> {
    if env::consts::OS != "macos" {
        return None;
    }

    let app_path = PathBuf::from("/Applications/VLC.app/Contents/MacOS/VLC");
    if executable_exists(&app_path) {
        return Some(app_path.to_string_lossy().into_owned());
    }

    if command_exists("vlc") {
        return Some("vlc".to_string());
    }

    None
}

/// Ensure the Solus player wrapper script exists, is executable, and return its path.
/// The wrapper intercepts ani-cli's invocation to:
/// 1. Inject `--audio-file` with the master playlist when HLS multi-audio split streams are detected (e.g. Cyberpunk Edgerunners).
/// 2. Clear and set `--sub-file` so ani-cli's English fallback never overrides the user's chosen subtitle track.
/// 3. Match audio track selection with the user's Dub / Sub preference.
pub fn ensure_player_wrapper(real_player: &str) -> Result<PathBuf> {
    let wrapper_name = if real_player.contains("iina") {
        "solus-iina"
    } else if real_player.contains("vlc") {
        "solus-vlc"
    } else {
        "solus-mpv"
    };

    // We choose a directory path strictly guaranteed to have NO spaces
    // (e.g. ~/.local/share/solus/bin or /tmp/solus-bin) because ani-cli executes
    // $player_function unquoted in bash ($player_function --referrer=...).
    let bin_dir = env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".local/share/solus/bin"))
        .filter(|p| !p.to_string_lossy().contains(' '))
        .unwrap_or_else(|| env::temp_dir().join("solus-bin"));

    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| AppError::Player(format!("Failed to create wrapper dir: {e}")))?;

    let wrapper_path = bin_dir.join(wrapper_name);

    let script_content = r#"#!/bin/bash

REAL_PLAYER="${SOLUS_REAL_PLAYER:-}"
if [ -z "$REAL_PLAYER" ]; then
    REAL_PLAYER="$(command -v mpv || command -v /opt/homebrew/bin/mpv || command -v /usr/local/bin/mpv || echo "mpv")"
fi
if [ ! -x "$REAL_PLAYER" ] && ! command -v "$REAL_PLAYER" >/dev/null 2>&1; then
    echo "Program $REAL_PLAYER not found" >&2
    exit 127
fi

args=()
video_link=""
refr=""

for arg in "$@"; do
    case "$arg" in
        --referrer=*)
            refr="${arg#--referrer=}"
            args+=("$arg")
            ;;
        --sub-file=*)
            # When SOLUS_SUB_URL is set, omit ani-cli's fallback sub-file
            if [ -z "$SOLUS_SUB_URL" ]; then
                args+=("$arg")
            fi
            ;;
        http*://* | /*.m3u8*)
            video_link="$arg"
            ;;
        *)
            args+=("$arg")
            ;;
    esac
done

is_iina=0
is_vlc=0
if [[ "$REAL_PLAYER" =~ iina ]] || [[ "$REAL_PLAYER" =~ IINA ]]; then
    is_iina=1
elif [[ "$REAL_PLAYER" =~ vlc ]] || [[ "$REAL_PLAYER" =~ VLC ]]; then
    is_vlc=1
fi

if [ "$SOLUS_SUB_DISABLED" = "1" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-sid=no" "--mpv-sub-visibility=no")
    elif [ "$is_vlc" -eq 1 ]; then
        args+=("--no-spu")
    else
        args+=("--sid=no" "--sub-visibility=no")
    fi
elif [ -n "$SOLUS_SUB_URL" ]; then
    if [ "$is_iina" -eq 1 ]; then
        escaped_sub="${SOLUS_SUB_URL//:/\\:}"
        args+=("--mpv-sub-files=$escaped_sub" "--mpv-sub-visibility=yes")
    elif [ "$is_vlc" -eq 1 ]; then
        args+=(":sub-file=$SOLUS_SUB_URL")
    else
        args+=("--sub-file=$SOLUS_SUB_URL" "--sub-visibility=yes" "--sub-auto=fuzzy")
    fi
fi

if [ -n "$SOLUS_SUB_DELAY" ] && [ "$SOLUS_SUB_DELAY" != "0" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-sub-delay=$SOLUS_SUB_DELAY")
    elif [ "$is_vlc" -eq 0 ]; then
        args+=("--sub-delay=$SOLUS_SUB_DELAY")
    fi
fi

if [[ "$video_link" =~ /index\.m3u8$ ]]; then
    master_url="${video_link%/*/*}/master.m3u8"
    if [ -n "$master_url" ]; then
        has_audio=$(curl -s --connect-timeout 1 -m 1 -e "$refr" "$master_url" 2>/dev/null | grep -m 1 "TYPE=AUDIO" || true)
        if [ -n "$has_audio" ]; then
            if [ "$is_iina" -eq 1 ]; then
                escaped_master="${master_url//:/\\:}"
                args+=("--mpv-audio-file=$escaped_master")
            elif [ "$is_vlc" -eq 1 ]; then
                video_link="$master_url"
            else
                args+=("--audio-file=$master_url" "--audio-file-auto=all")
            fi
        fi
    fi
fi

if [ "$SOLUS_AUDIO_DUB" = "1" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-alang=en,eng,English")
    elif [ "$is_vlc" -eq 1 ]; then
        args+=("--audio-language=en,eng")
    else
        args+=("--alang=en,eng,English")
    fi
else
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-alang=ja,jpn,Japanese")
    elif [ "$is_vlc" -eq 1 ]; then
        args+=("--audio-language=ja,jpn")
    else
        args+=("--alang=ja,jpn,Japanese")
    fi
fi

if [ -n "$SOLUS_IPC_SOCKET" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-input-ipc-server=$SOLUS_IPC_SOCKET")
    elif [ "$is_vlc" -eq 0 ]; then
        args+=("--input-ipc-server=$SOLUS_IPC_SOCKET")
    fi
fi

if [ -n "$SOLUS_AUDIO_SCRIPT" ] && [ "$is_vlc" -eq 0 ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-script=$SOLUS_AUDIO_SCRIPT")
    else
        args+=("--script=$SOLUS_AUDIO_SCRIPT")
    fi
fi

if [ -n "$SOLUS_START_POS" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-start=$SOLUS_START_POS")
    elif [ "$is_vlc" -eq 1 ]; then
        args+=(":start-time=$SOLUS_START_POS")
    else
        args+=("--start=$SOLUS_START_POS")
    fi
fi

if [ -n "$SOLUS_SHADERS" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-glsl-shaders=$SOLUS_SHADERS" "--mpv-scale=ewa_lanczossharp" "--mpv-cscale=ewa_lanczossharp" "--mpv-dscale=mitchell" "--mpv-correct-downscaling=yes" "--mpv-linear-downscaling=yes" "--mpv-sigmoid-upscaling=yes" "--mpv-deband=yes" "--mpv-deband-iterations=2" "--mpv-deband-threshold=35" "--mpv-deband-range=16")
    elif [ "$is_vlc" -eq 0 ]; then
        args+=("--glsl-shaders=$SOLUS_SHADERS" "--scale=ewa_lanczossharp" "--cscale=ewa_lanczossharp" "--dscale=mitchell" "--correct-downscaling=yes" "--linear-downscaling=yes" "--sigmoid-upscaling=yes" "--deband=yes" "--deband-iterations=2" "--deband-threshold=35" "--deband-range=16")
    fi
fi

if [ "$SOLUS_AUTO_PIP" = "1" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-fullscreen=no" "--mpv-ontop=yes" "--mpv-border=no" "--mpv-keepaspect-window=yes" "--mpv-window-scale=0.30" "--mpv-geometry=30%x30%-24-24" "--mpv-script-opts=solus_auto_pip=1")
    elif [ "$is_vlc" -eq 1 ]; then
        args+=("--video-on-top")
    else
        args+=("--fullscreen=no" "--ontop=yes" "--border=no" "--keepaspect-window=yes" "--window-scale=0.30" "--geometry=30%x30%-24-24" "--script-opts=solus_auto_pip=1")
    fi
elif [ "$SOLUS_AUTO_FULLSCREEN" = "1" ]; then
    if [ "$is_iina" -eq 1 ]; then
        args+=("--mpv-fullscreen=yes")
    elif [ "$is_vlc" -eq 1 ]; then
        args+=("--fullscreen")
    else
        args+=("--fullscreen=yes")
    fi
fi

if [ "$is_vlc" -eq 1 ]; then
    vlc_args=("--play-and-exit" "--no-video-title-show")
    item_opts=()
    for a in "${args[@]}"; do
        case "$a" in
            --input-terminal=*|--force-media-title=*|--sub-files-clr|--sub-auto=*|--keep-running|--no-stdin)
                ;;
            --referrer=*)
                vlc_args+=("--http-referrer=${a#--referrer=}")
                ;;
            :http-referrer=*)
                vlc_args+=("--http-referrer=${a#:http-referrer=}")
                ;;
            --sub-file=*)
                item_opts+=(":sub-file=${a#--sub-file=}")
                ;;
            :sub-file=*)
                item_opts+=("$a")
                ;;
            --start=*)
                item_opts+=(":start-time=${a#--start=}")
                ;;
            :start-time=*)
                item_opts+=("$a")
                ;;
            --audio-language=*|--meta-title=*)
                vlc_args+=("$a")
                ;;
            *)
                vlc_args+=("$a")
                ;;
        esac
    done
    if [ -n "$video_link" ]; then
        exec "$REAL_PLAYER" "${vlc_args[@]}" "$video_link" "${item_opts[@]}"
    else
        exec "$REAL_PLAYER" "${vlc_args[@]}" "${item_opts[@]}"
    fi
elif [ "$is_iina" -eq 1 ]; then
    iina_args=("--keep-running" "--no-stdin")
    if [ -n "$refr" ]; then
        iina_args+=("--mpv-referrer=$refr")
    else
        iina_args+=("--mpv-referrer=https://zokoanime.video/")
    fi
    iina_args+=("--mpv-user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
    for a in "${args[@]}"; do
        case "$a" in
            --input-terminal=*|--keep-running|--no-stdin|--mpv-input-ipc-server=*|--input-ipc-server=*)
                ;;
            --referrer=*)
                ;;
            --force-media-title=*)
                iina_args+=("--mpv-force-media-title=${a#--force-media-title=}")
                ;;
            --sub-file=*)
                iina_args+=("--mpv-sub-file=${a#--sub-file=}" "--mpv-sub-visibility=yes")
                ;;
            --sub-files=*)
                iina_args+=("--mpv-sub-file=${a#--sub-files=}" "--mpv-sub-visibility=yes")
                ;;
            --mpv-sub-files=*)
                iina_args+=("--mpv-sub-file=${a#--mpv-sub-files=}" "--mpv-sub-visibility=yes")
                ;;
            --start=*)
                iina_args+=("--mpv-start=${a#--start=}")
                ;;
            --mpv-*)
                iina_args+=("$a")
                ;;
            --*)
                iina_args+=("--mpv-${a#--}")
                ;;
            *)
                iina_args+=("$a")
                ;;
        esac
    done
    if [ -n "$video_link" ]; then
        exec "$REAL_PLAYER" "${iina_args[@]}" "$video_link"
    else
        exec "$REAL_PLAYER" "${iina_args[@]}"
    fi
else
    if [ -n "$video_link" ]; then
        exec "$REAL_PLAYER" "${args[@]}" "$video_link"
    else
        exec "$REAL_PLAYER" "${args[@]}"
    fi
fi
"#;

    std::fs::write(&wrapper_path, script_content)
        .map_err(|e| AppError::Player(format!("Failed to write player wrapper: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&wrapper_path)
            .map_err(|e| AppError::Player(format!("Failed to read wrapper metadata: {e}")))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&wrapper_path, perms)
            .map_err(|e| AppError::Player(format!("Failed to set wrapper permissions: {e}")))?;
    }

    Ok(wrapper_path)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn derive_master_playlist_url(video_link: &str) -> Option<String> {
    let trimmed = video_link.split('?').next().unwrap_or(video_link);
    if !trimmed.ends_with("/index.m3u8") {
        return None;
    }
    let without_file = trimmed.strip_suffix("/index.m3u8")?;
    let (parent_of_parent, _) = without_file.rsplit_once('/')?;
    if !parent_of_parent.contains("://") || parent_of_parent.ends_with(":/") {
        return None;
    }
    Some(format!("{}/master.m3u8", parent_of_parent))
}

const YOUTUBE_INPUT_CONF: &str = r#"# Solus YouTube-Style Player Keybindings
k cycle pause
K cycle pause
SPACE cycle pause
j seek -10
J seek -10
l seek 10
L seek 10
LEFT seek -5
RIGHT seek 5
UP add volume 5
DOWN add volume -5
m cycle mute
M cycle mute
f cycle fullscreen
F cycle fullscreen
ESC set fullscreen no
c cycle sub-visibility
C cycle sub-visibility
p script-binding solus-toggle-pip
P script-binding solus-cycle-pip-corner
i script-binding solus-toggle-hud
I script-binding solus-pin-hud
F1 script-binding stats/display-stats-toggle
0 seek 0 absolute-percent
1 seek 10 absolute-percent
2 seek 20 absolute-percent
3 seek 30 absolute-percent
4 seek 40 absolute-percent
5 seek 50 absolute-percent
6 seek 60 absolute-percent
7 seek 70 absolute-percent
8 seek 80 absolute-percent
9 seek 90 absolute-percent
< multiply speed 0.8
> multiply speed 1.25
. frame-step
, frame-back-step
"#;

/// Ensure Solus's YouTube-style `mpv` keybindings file exists on disk and return its path.
pub fn ensure_youtube_input_conf() -> Option<PathBuf> {
    let dir = crate::config::Config::cache_dir()
        .unwrap_or_else(|_| env::temp_dir().join("solus-cache"));
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let path = dir.join("youtube-input.conf");
    let needs_write = std::fs::read_to_string(&path)
        .map(|cur| cur != YOUTUBE_INPUT_CONF)
        .unwrap_or(true);
    if needs_write && std::fs::write(&path, YOUTUBE_INPUT_CONF).is_err() {
        return None;
    }
    Some(path)
}

const SOLUS_AUDIO_LUA: &str = r##"-- Solus Unified mpv Companion Script:
--   1. Smart Audio Cycle Bridge (# / Ctrl+a)
--   2. Floating Picture-in-Picture Controller (p toggle, P cycle corner/size)
--   3. Multi-Color Curated Bento Video Info HUD (i 6s toggle, I 1Hz pin, F1 raw stats)
local function solus_cycle_audio()
    local tracks = mp.get_property_native("track-list") or {}
    local audio_tracks = {}
    local cur_aid = mp.get_property_number("aid")
    for _, t in ipairs(tracks) do
        if t.type == "audio" and t.id then
            table.insert(audio_tracks, t)
        end
    end

    if #audio_tracks > 1 then
        local cur_idx = 1
        for i, t in ipairs(audio_tracks) do
            if t.id == cur_aid then
                cur_idx = i
                break
            end
        end
        local next_idx = (cur_idx % #audio_tracks) + 1
        local nxt = audio_tracks[next_idx]
        mp.set_property_number("aid", nxt.id)
        mp.commandv("seek", "0", "relative+exact")
        local label = nxt.title or nxt.lang or ("Track " .. tostring(nxt.id))
        mp.osd_message("✦ Audio: " .. tostring(label), 2.5)
    else
        mp.osd_message("✦ Switching Sub/Dub stream...", 2.5)
        io.write("__SOLUS_AUDIO_CYCLE__\n")
        io.flush()
    end
end

mp.add_forced_key_binding("#", "solus-cycle-audio-hash", solus_cycle_audio)
mp.add_forced_key_binding("SHARP", "solus-cycle-audio-sharp", solus_cycle_audio)
mp.add_forced_key_binding("Ctrl+a", "solus-cycle-audio-ctrl", solus_cycle_audio)

-- =============================================================================
-- 2. Picture-in-Picture (PiP) Floating Window Controller ([p] / [P])
-- =============================================================================
local pip_active = (mp.get_opt("solus_auto_pip") or "0") == "1"
local pip_idx = 1
local pip_presets = {
    { label = "Bottom-Right 30%", scale = 0.30, geom = "30%x30%-24-24" },
    { label = "Bottom-Left 30%",  scale = 0.30, geom = "30%x30%+24-24" },
    { label = "Top-Right 30%",    scale = 0.30, geom = "30%x30%-24+24" },
    { label = "Top-Left 30%",     scale = 0.30, geom = "30%x30%+24+24" },
    { label = "Compact 24%",      scale = 0.24, geom = "24%x24%-20-20" },
    { label = "Theater 38%",      scale = 0.38, geom = "38%x38%-28-28" },
}

local function apply_pip_preset()
    local pr = pip_presets[pip_idx]
    mp.set_property_bool("fullscreen", false)
    mp.set_property_bool("ontop", true)
    mp.set_property_bool("border", false)
    mp.set_property_bool("keepaspect-window", true)
    mp.set_property_number("window-scale", pr.scale)
    mp.set_property("geometry", pr.geom)
    mp.osd_message("◈ PiP Floating: " .. pr.label .. " ([P] corner, [p] exit)", 2.2)
end

local function toggle_pip()
    pip_active = not pip_active
    if pip_active then
        apply_pip_preset()
    else
        mp.set_property_bool("ontop", false)
        mp.set_property_bool("border", true)
        mp.set_property_number("window-scale", 1.0)
        mp.set_property("geometry", "50%:50%")
        mp.osd_message("◈ PiP Floating: OFF (Standard Window)", 2.0)
    end
end

local function cycle_pip_corner()
    if not pip_active then
        pip_active = true
    else
        pip_idx = (pip_idx % #pip_presets) + 1
    end
    apply_pip_preset()
end

mp.add_forced_key_binding("p", "solus-toggle-pip", toggle_pip)
mp.add_forced_key_binding("P", "solus-cycle-pip-corner", cycle_pip_corner)

-- =============================================================================
-- 3. Multi-Color Curated Bento Video Info HUD ([i] 6s / [I] Pinned 1Hz / [F1] Raw)
-- =============================================================================
local hud_ov = mp.create_osd_overlay("ass-events")
hud_ov.res_x = 1280
hud_ov.res_y = 720
local hud_visible = false
local hud_pinned = false
local hud_tick_timer = nil
local hud_hide_timer = nil

local function esc_ass(s)
    if not s then return "" end
    return tostring(s):gsub("[{}]", ""):gsub("[\r\n]+", " ")
end

local function fmt_clock(s)
    if not s or s < 0 then return "--:--" end
    local tot = math.floor(s)
    local h = math.floor(tot / 3600)
    local m = math.floor((tot % 3600) / 60)
    local sec = tot % 60
    if h > 0 then
        return string.format("%d:%02d:%02d", h, m, sec)
    end
    return string.format("%02d:%02d", m, sec)
end

local function fmt_speed(bps)
    if not bps or bps <= 0 then return "" end
    if bps >= 1048576 then
        return string.format("%.1f MB/s", bps / 1048576.0)
    elseif bps >= 1024 then
        return string.format("%d KB/s", math.floor(bps / 1024.0))
    end
    return string.format("%d B/s", math.floor(bps))
end

local function hide_info_hud()
    hud_visible = false
    hud_pinned = false
    if hud_tick_timer then hud_tick_timer:kill(); hud_tick_timer = nil end
    if hud_hide_timer then hud_hide_timer:kill(); hud_hide_timer = nil end
    hud_ov:remove()
end

local function render_info_hud()
    if not hud_visible then
        hud_ov:remove()
        return
    end
    local mtitle = esc_ass(mp.get_property("media-title", "Solus Stream"))
    if #mtitle > 68 then mtitle = string.sub(mtitle, 1, 65) .. "..." end

    local pos = mp.get_property_number("time-pos", 0) or 0
    local dur = mp.get_property_number("duration", 0) or 0
    local rem = math.max(0, dur - pos)
    local pct = mp.get_property_number("percent-pos", 0) or 0
    local spd = mp.get_property_number("speed", 1.0) or 1.0
    local vw = mp.get_property_number("width", 0) or 0
    local vh = mp.get_property_number("height", 0) or 0
    local fps = mp.get_property_number("estimated-vf-fps", 0) or 0
    local disp_fps = mp.get_property_number("display-fps", 0) or 0
    local vcodec = esc_ass(mp.get_property("video-codec", "—")):upper()
    local vbit = mp.get_property_number("video-bitrate", 0) or 0
    local hwdec = esc_ass(mp.get_property("hwdec-current", ""))
    local drops = (mp.get_property_number("decoder-frame-drop-count", 0) or 0) + (mp.get_property_number("frame-drop-count", 0) or 0)
    local vo = esc_ass(mp.get_property("current-vo", "gpu-next"))
    local avs = mp.get_property_number("avsync", 0) or 0
    local shaders_s = esc_ass(mp.get_property("glsl-shaders", ""))

    local acodec = esc_ass(mp.get_property("audio-codec-name", mp.get_property("audio-codec", "—"))):upper()
    local abit = mp.get_property_number("audio-bitrate", 0) or 0
    local sr = mp.get_property_number("audio-params/samplerate", 0) or 0
    local ch_n = mp.get_property_number("audio-params/channel-count", 2) or 2
    local vol = mp.get_property_number("volume", 100) or 100
    local muted = mp.get_property_bool("mute", false)

    local sub_vis = mp.get_property_bool("sub-visibility", false)
    local sub_delay = mp.get_property_number("sub-delay", 0) or 0
    local sub_lbl = sub_vis and string.format("ON (%+.2fs)", sub_delay) or "OFF"

    local c_dur = mp.get_property_number("demuxer-cache-duration", 0) or 0
    local c_spd = mp.get_property_number("cache-speed", 0) or 0
    local c_bytes = 0
    local cst = mp.get_property_native("demuxer-cache-state")
    if cst and cst["total-bytes"] then c_bytes = cst["total-bytes"] end

    local q_tag = (vh >= 2160 and "4K UHD") or (vh >= 1440 and "1440p QHD") or (vh >= 1080 and "1080p FHD") or (vh >= 720 and "720p HD") or "Stream"
    local win_badge = pip_active and "PIP FLOAT" or (mp.get_property_bool("fullscreen", false) and "FULLSCREEN" or "WINDOWED")
    local pin_badge = hud_pinned and "PINNED 1Hz" or "AUTO 6s"
    local hw_lbl = (hwdec ~= "" and hwdec ~= "no") and ("GPU · " .. hwdec) or "CPU Software"
    local hw_col = (hwdec ~= "" and hwdec ~= "no") and "&H99D334&" or "&H24BFFB&"
    local drop_col = (drops > 0) and "&H5E3FF4&" or "&H99D334&"
    local vol_str = muted and "MUTED" or string.format("%d%%", math.floor(vol))
    local vol_col = muted and "&H5E3FF4&" or "&H99D334&"
    local vbit_s = (vbit > 0) and string.format("%.2f Mbps", vbit / 1000000.0) or "HLS ABR"
    local abit_s = (abit > 0) and string.format("%d kbps", math.floor(abit / 1000.0)) or "Auto"
    local sr_s = (sr > 0) and string.format("%.1f kHz", sr / 1000.0) or "Auto"
    local shader_state = (shaders_s ~= "") and "Anime4K Active" or "Off (Native)"
    local c_mib = c_bytes / 1048576.0
    local spd_txt = fmt_speed(c_spd)
    local buf_health = (c_dur >= 12.0) and "Healthy" or ((c_dur >= 3.0) and "Fair" or "Low")
    local buf_col = (c_dur >= 12.0) and "&H99D334&" or ((c_dur >= 3.0) and "&H24BFFB&" or "&H5E3FF4&")

    local prog_w = math.max(8, math.floor(1140 * math.min(1.0, math.max(0.0, pct / 100.0))))
    local buf_frac = math.min(1.0, c_dur / 60.0)
    local buf_w = math.max(12, math.floor(520 * buf_frac))

    -- Glass Bento Card at (x: 42..1238, y: 26..378)
    local ass = string.format(
        "{\\an7\\pos(0,0)\\bord0\\shad0\\1c&H100C09&\\1a&H18&\\p1}m 42 26 l 1238 26 l 1238 378 l 42 378{\\p0}\n" ..
        "{\\an7\\pos(0,0)\\bord2\\1a&HFF&\\3c&HF8BD38&\\p1}m 42 26 l 1238 26 l 1238 378 l 42 378{\\p0}\n" ..
        "{\\an7\\pos(66,42)\\bord0\\shad0\\fs19\\b1\\1c&HF8BD38&}◈ SOLUS INFO HUD   " ..
        "{\\1c&H24BFFB&}[%s]   {\\1c&H99D334&}[%s]   {\\1c&HFA8BA7&}[%s]\n" ..
        "{\\an7\\pos(66,70)\\bord0\\shad0\\fs24\\b1\\1c&HFA8BA7&}%s\n" ..
        "{\\an7\\pos(66,102)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}%s / %s (%d%%)  ·  -%s left  ·  Speed %.2fx\n" ..
        "{\\an7\\pos(66,128)\\bord0\\shad0\\1c&H2B2420&\\p1}m 0 0 l 1140 0 l 1140 6 l 0 6{\\p0}\n" ..
        "{\\an7\\pos(66,128)\\bord0\\shad0\\1c&HFA8BA7&\\p1}m 0 0 l %d 0 l %d 6 l 0 6{\\p0}\n" ..
        "{\\an7\\pos(66,150)\\bord0\\shad0\\fs16\\b1\\1c&HF8BD38&}▶ VIDEO & QUALITY\n" ..
        "{\\an7\\pos(66,176)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Stream  : {\\1c&HFAF4F0&\\b1}%dx%d  %s\n" ..
        "{\\an7\\pos(66,200)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Codec   : {\\1c&HF8BD38&}%s  ·  %s\n" ..
        "{\\an7\\pos(66,224)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Rate    : {\\1c&H99D334&}%.2f fps{\\1c&HC8B8A4&} (Disp %.0fHz)\n" ..
        "{\\an7\\pos(66,248)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Decode  : {\\1c%s}%s\n" ..
        "{\\an7\\pos(460,150)\\bord0\\shad0\\fs16\\b1\\1c&HFA8BA7&}♪ AUDIO & CHANNELS\n" ..
        "{\\an7\\pos(460,176)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Codec   : {\\1c&HFA8BA7&}%s  ·  %s\n" ..
        "{\\an7\\pos(460,200)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Output  : {\\1c&HFAF4F0&}%s  ·  %d ch\n" ..
        "{\\an7\\pos(460,224)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Volume  : {\\1c%s}%s {\\1c&HC8B8A4&}· Speed %.2fx\n" ..
        "{\\an7\\pos(460,248)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Subs    : {\\1c&H24BFFB&}%s\n" ..
        "{\\an7\\pos(860,150)\\bord0\\shad0\\fs16\\b1\\1c&H24BFFB&}◈ CACHE & ENGINE\n" ..
        "{\\an7\\pos(860,176)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Shaders : {\\1c&HFA8BA7&}%s\n" ..
        "{\\an7\\pos(860,200)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}VideoOut: {\\1c&H24BFFB&}%s\n" ..
        "{\\an7\\pos(860,224)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}Dropped : {\\1c%s}%d frames\n" ..
        "{\\an7\\pos(860,248)\\bord0\\shad0\\fs16\\1c&HC8B8A4&}A/V Sync: {\\1c&H99D334&}%+.1f ms\n" ..
        "{\\an7\\pos(66,288)\\bord0\\shad0\\fs16\\b1\\1c&H24BFFB&}⚡ DEMUXER CACHE   {\\1c&HFAF4F0&\\b0}%.1fs ahead  ·  %.1f MiB  %s  ·  {\\1c%s}%s\n" ..
        "{\\an7\\pos(66,314)\\bord0\\shad0\\1c&H2B2420&\\p1}m 0 0 l 520 0 l 520 10 l 0 10{\\p0}\n" ..
        "{\\an7\\pos(66,314)\\bord0\\shad0\\1c%s\\p1}m 0 0 l %d 0 l %d 10 l 0 10{\\p0}\n" ..
        "{\\an7\\pos(66,340)\\bord0\\shad0\\fs15\\1c&HC8B8A4&}Keys: {\\1c&HF8BD38&}[i] 6s HUD  [I] Pin 1Hz  {\\1c&H99D334&}[p] PiP Float  [P] PiP Corner  {\\1c&H24BFFB&}[f] Fullscreen  {\\1c&HFA8BA7&}[F1] Raw Stats",
        q_tag, win_badge, pin_badge,
        mtitle,
        fmt_clock(pos), fmt_clock(dur), math.floor(pct), fmt_clock(rem), spd,
        prog_w, prog_w,
        vw, vh, q_tag,
        vcodec, vbit_s,
        fps, disp_fps,
        hw_col, hw_lbl,
        acodec, abit_s,
        sr_s, ch_n,
        vol_col, vol_str, spd,
        sub_lbl,
        shader_state,
        vo,
        drop_col, drops,
        avs * 1000.0,
        c_dur, c_mib, (spd_txt ~= "") and ("· " .. spd_txt) or "", buf_col, buf_health,
        buf_col, buf_w, buf_w
    )
    hud_ov.data = ass
    hud_ov:update()
end

local function ensure_hud_timer()
    if not hud_tick_timer then
        hud_tick_timer = mp.add_periodic_timer(1.0, render_info_hud)
    end
end

local function toggle_info_hud()
    if hud_visible and not hud_pinned then
        hide_info_hud()
        return
    end
    hud_pinned = false
    hud_visible = true
    if hud_hide_timer then hud_hide_timer:kill() end
    hud_hide_timer = mp.add_timeout(6.0, function()
        if not hud_pinned then hide_info_hud() end
    end)
    ensure_hud_timer()
    render_info_hud()
end

local function pin_info_hud()
    if hud_visible and hud_pinned then
        hide_info_hud()
        return
    end
    if hud_hide_timer then hud_hide_timer:kill(); hud_hide_timer = nil end
    hud_visible = true
    hud_pinned = true
    ensure_hud_timer()
    render_info_hud()
end

mp.add_forced_key_binding("i", "solus-toggle-hud", toggle_info_hud)
mp.add_forced_key_binding("I", "solus-pin-hud", pin_info_hud)
"##;

/// Ensure Solus's `mpv` audio-switch helper script exists on disk and return its path.
pub fn ensure_solus_audio_lua() -> Option<PathBuf> {
    let dir = crate::config::Config::cache_dir()
        .unwrap_or_else(|_| env::temp_dir().join("solus-cache"));
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let path = dir.join("solus-audio.lua");
    let needs_write = std::fs::read_to_string(&path)
        .map(|cur| cur != SOLUS_AUDIO_LUA)
        .unwrap_or(true);
    if needs_write && std::fs::write(&path, SOLUS_AUDIO_LUA).is_err() {
        return None;
    }
    Some(path)
}

pub fn mpv_network_args(
    mode: crate::config::NetworkMode,
    force_ipv4: bool,
    is_local_file: bool,
) -> Vec<String> {
    if is_local_file {
        return Vec::new();
    }
    let mut args = vec![
        "--stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_on_network_error=1,reconnect_delay_max=3,multiple_requests=1".to_string(),
        "--demuxer-lavf-o=http_persistent=1,http_multiple=1".to_string(),
        format!("--hls-bitrate={}", mode.hls_bitrate_cap()),
        format!("--demuxer-readahead-secs={}", mode.vod_readahead_secs()),
        "--cache-pause-initial=yes".to_string(),
        format!("--cache-pause-wait={:.1}", mode.vod_cache_pause_wait_secs()),
        format!("--network-timeout={}", mode.network_timeout_secs()),
    ];
    if mode.use_disk_cache() {
        args.push("--cache-on-disk=yes".to_string());
        if let Ok(cache_root) = crate::config::Config::cache_dir() {
            let mpv_cache = cache_root.join("mpv-cache");
            let _ = std::fs::create_dir_all(&mpv_cache);
            args.push(format!("--demuxer-cache-dir={}", mpv_cache.display()));
        }
    }
    if force_ipv4 {
        args.push("--ytdl-raw-options-append=force-ipv4=".to_string());
    }
    args
}

/// Build direct `mpv` arguments, wrapping the stream in `--{ ... --}` when a Y2K loading screen precedes it.
pub fn build_mpv_direct_args(
    opts: &PlayOptions,
    direct_url: &str,
    youtube_conf: Option<&Path>,
) -> Vec<String> {
    let is_local_file = !(direct_url.starts_with("http://") || direct_url.starts_with("https://"));
    let mut args = vec![
        "--ytdl=no".to_string(),
        "--force-window=immediate".to_string(),
        "--keep-open=no".to_string(),
        "--referrer=https://zokoanime.video/".to_string(),
        "--input-terminal=no".to_string(),
    ];
    if opts.auto_pip {
        args.push("--fullscreen=no".to_string());
        args.push("--ontop=yes".to_string());
        args.push("--border=no".to_string());
        args.push("--keepaspect-window=yes".to_string());
        args.push("--window-scale=0.30".to_string());
        args.push("--geometry=30%x30%-24-24".to_string());
        args.push("--script-opts=solus_auto_pip=1".to_string());
    } else if opts.auto_fullscreen {
        args.push("--fullscreen=yes".to_string());
    }
    args.extend(mpv_network_args(opts.network_mode, opts.force_ipv4, is_local_file));

    if let Some(conf_path) = youtube_conf {
        args.push(format!("--input-conf={}", conf_path.display()));
    }
    if let Some(ref sock) = opts.ipc_socket {
        args.push(format!("--input-ipc-server={}", sock));
    }

    let has_loading = opts
        .loading_image
        .as_deref()
        .is_some_and(|p| !p.trim().is_empty());

    if has_loading {
        args.push("--image-display-duration=4".to_string());
        args.push("--prefetch-playlist=yes".to_string());
        args.push("--loop-playlist=no".to_string());
        if let Some(ref loading_img) = opts.loading_image {
            args.push("--{".to_string());
            args.push(loading_img.clone());
            args.push("--}".to_string());
        }
        args.push("--{".to_string());
    }

    args.push(format!(
        "--force-media-title={} Episode {}",
        opts.title, opts.episode
    ));

    let subs_disabled = opts.subtitles_disabled
        || opts
            .sub_lang
            .as_deref()
            .is_some_and(|l| l.eq_ignore_ascii_case("none"));
    if subs_disabled {
        args.push("--sid=no".to_string());
        args.push("--sub-visibility=no".to_string());
    } else if let Some(ref url) = opts.sub_url {
        if !url.is_empty() {
            args.push(format!("--sub-file={}", url));
            args.push("--sub-visibility=yes".to_string());
            args.push("--sub-auto=fuzzy".to_string());
        }
    }
    if opts.sub_delay_ms != 0 {
        let delay_secs = opts.sub_delay_ms as f64 / 1000.0;
        args.push(format!("--sub-delay={:.3}", delay_secs));
    }
    if let Some(master_url) = derive_master_playlist_url(direct_url) {
        args.push(format!("--audio-file={}", master_url));
    }
    args.push("--audio-file-auto=all".to_string());
    if opts.dub {
        args.push("--alang=en,eng,English".to_string());
    } else {
        args.push("--alang=ja,jpn,Japanese".to_string());
    }
    if let Some(pos) = opts.start_pos {
        args.push(format!("--start={}", pos));
    }
    if let Some(ref shaders) = opts.glsl_shaders {
        args.push(format!("--glsl-shaders={}", shaders));
        args.push("--scale=ewa_lanczossharp".to_string());
        args.push("--cscale=ewa_lanczossharp".to_string());
        args.push("--dscale=mitchell".to_string());
        args.push("--correct-downscaling=yes".to_string());
        args.push("--linear-downscaling=yes".to_string());
        args.push("--sigmoid-upscaling=yes".to_string());
        args.push("--deband=yes".to_string());
        args.push("--deband-iterations=2".to_string());
        args.push("--deband-threshold=35".to_string());
        args.push("--deband-range=16".to_string());
    }
    if let Some(ref style) = opts.sub_style {
        args.extend(style.to_mpv_flags());
    }

    args.push(direct_url.to_string());
    if has_loading {
        args.push("--}".to_string());
    }
    args
}

/// Spawn ani-cli asynchronously. stdout + stderr are piped for log streaming.
/// The player is detached by default (ani-cli default behaviour).
/// Returns a tokio Child — caller is responsible for reading I/O and waiting.
pub fn spawn_async(opts: &PlayOptions) -> Result<Child> {
    let args = build_args(opts);
    let player = resolve_player(&opts.player);
    #[cfg(not(windows))]
    let wrapper_path = ensure_player_wrapper(&player)?;
    #[cfg(not(windows))]
    let bin_dir = wrapper_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| env::temp_dir().join("solus-bin"));
    #[cfg(not(windows))]
    let wrapper_name = wrapper_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "solus-mpv".to_string());

    let yt_conf = ensure_youtube_input_conf();
    let audio_script = ensure_solus_audio_lua();

    // 1. Direct stream playback:
    // If we resolved the exact verified stream URL (.m3u8), launch the player directly.
    // This eliminates ani-cli fuzzy text searches, guaranteeing 100%
    // frame-perfect subtitle synchronization, accurate resolution, and sub-100ms launch.
    if let Some(ref direct_url) = opts.stream_url {
        if !direct_url.is_empty() {
            let is_vlc = player.to_lowercase().contains("vlc");
            let is_iina = player.to_lowercase().contains("iina");
            let is_local_file = !(direct_url.starts_with("http://") || direct_url.starts_with("https://"));

            let mut cmd = Command::new(&player);

            let subs_disabled = opts.subtitles_disabled
                || opts
                    .sub_lang
                    .as_deref()
                    .is_some_and(|l| l.eq_ignore_ascii_case("none"));

            if is_vlc {
                // VLC-native arguments:
                // --play-and-exit ensures VLC immediately quits when playback finishes or the window is closed
                cmd.arg("--play-and-exit");
                cmd.arg("--no-video-title-show");
                if opts.auto_pip {
                    cmd.arg("--video-on-top");
                } else if opts.auto_fullscreen {
                    cmd.arg("--fullscreen");
                }
                cmd.arg(format!("--meta-title={} Episode {}", opts.title, opts.episode));
                cmd.arg("--http-referrer=https://zokoanime.video/");
                cmd.arg("--http-user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
                if !is_local_file {
                    cmd.arg(format!("--network-caching={}", opts.network_mode.vlc_network_caching_ms()));
                    if opts.force_ipv4 {
                        cmd.arg("--ipv4");
                    }
                }
                if opts.dub {
                    cmd.arg("--audio-language=en,eng");
                } else {
                    cmd.arg("--audio-language=ja,jpn");
                }
                if subs_disabled {
                    cmd.arg("--no-spu");
                }
                if opts.sub_delay_ms != 0 {
                    cmd.arg(format!("--sub-delay={}", opts.sub_delay_ms / 100));
                }
                cmd.arg(direct_url);
                if !subs_disabled {
                    if let Some(ref url) = opts.sub_url {
                        if !url.is_empty() {
                            cmd.arg(format!(":sub-file={}", url));
                        }
                    }
                }
                if let Some(pos) = opts.start_pos {
                    cmd.arg(format!(":start-time={}", pos));
                }
            } else if is_iina {
                // IINA-native arguments (--keep-running prevents iina-cli from exiting prematurely)
                // All options must precede the media URL to satisfy iina-cli syntax and header requirements.
                cmd.arg("--keep-running");
                cmd.arg("--no-stdin");
                cmd.arg("--mpv-ytdl=no");
                cmd.arg("--mpv-force-window=immediate");
                if opts.auto_pip {
                    cmd.arg("--mpv-fullscreen=no");
                    cmd.arg("--mpv-ontop=yes");
                    cmd.arg("--mpv-border=no");
                    cmd.arg("--mpv-keepaspect-window=yes");
                    cmd.arg("--mpv-window-scale=0.30");
                    cmd.arg("--mpv-geometry=30%x30%-24-24");
                    cmd.arg("--mpv-script-opts=solus_auto_pip=1");
                } else if opts.auto_fullscreen {
                    cmd.arg("--mpv-fullscreen=yes");
                }
                for net_flag in mpv_network_args(opts.network_mode, opts.force_ipv4, is_local_file) {
                    cmd.arg(format!("--mpv-{}", net_flag.trim_start_matches("--")));
                }
                if let Some(ref conf_path) = yt_conf {
                    cmd.arg(format!("--mpv-input-conf={}", conf_path.display()));
                }
                if let Some(ref script_path) = audio_script {
                    cmd.arg(format!("--mpv-script={}", script_path.display()));
                }
                cmd.arg("--mpv-referrer=https://zokoanime.video/");
                cmd.arg("--mpv-user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
                cmd.arg(format!("--mpv-force-media-title={} Episode {}", opts.title, opts.episode));
                if let Some(ref sock) = opts.ipc_socket {
                    cmd.env("SOLUS_IPC_SOCKET", sock);
                    cmd.arg(format!("--mpv-input-ipc-server={}", sock));
                }
                if subs_disabled {
                    cmd.arg("--mpv-sid=no");
                    cmd.arg("--mpv-sub-visibility=no");
                } else if let Some(ref url) = opts.sub_url {
                    if !url.is_empty() {
                        cmd.arg(format!("--mpv-sub-file={}", url));
                        cmd.arg("--mpv-sub-visibility=yes");
                    }
                }
                if opts.sub_delay_ms != 0 {
                    cmd.arg(format!("--mpv-sub-delay={:.3}", opts.sub_delay_ms as f64 / 1000.0));
                }
                if opts.dub {
                    cmd.arg("--mpv-alang=en,eng,English");
                } else {
                    cmd.arg("--mpv-alang=ja,jpn,Japanese");
                }
                if let Some(pos) = opts.start_pos {
                    cmd.arg(format!("--mpv-start={}", pos));
                }
                if let Some(ref shaders) = opts.glsl_shaders {
                    cmd.arg(format!("--mpv-glsl-shaders={}", shaders));
                    cmd.arg("--mpv-scale=ewa_lanczossharp");
                    cmd.arg("--mpv-cscale=ewa_lanczossharp");
                    cmd.arg("--mpv-dscale=mitchell");
                    cmd.arg("--mpv-correct-downscaling=yes");
                    cmd.arg("--mpv-linear-downscaling=yes");
                    cmd.arg("--mpv-sigmoid-upscaling=yes");
                    cmd.arg("--mpv-deband=yes");
                    cmd.arg("--mpv-deband-iterations=2");
                    cmd.arg("--mpv-deband-threshold=35");
                    cmd.arg("--mpv-deband-range=16");
                }
                if let Some(ref style) = opts.sub_style {
                    for flag in style.to_mpv_flags() {
                        cmd.arg(format!("--mpv-{}", flag.trim_start_matches("--")));
                    }
                }
                // Media URL is placed strictly trailing
                cmd.arg(direct_url);
            } else {
                // Standard MPV engine arguments (shared across Windows/macOS/Linux)
                if let Some(ref sock) = opts.ipc_socket {
                    cmd.env("SOLUS_IPC_SOCKET", sock);
                }
                if let Some(pos) = opts.start_pos {
                    cmd.env("SOLUS_START_POS", pos.to_string());
                }
                if let Some(ref shaders) = opts.glsl_shaders {
                    cmd.env("SOLUS_SHADERS", shaders);
                }
                for arg in build_mpv_direct_args(opts, direct_url, yt_conf.as_deref()) {
                    cmd.arg(arg);
                }
                if let Some(ref script_path) = audio_script {
                    cmd.arg(format!("--script={}", script_path.display()));
                }
            }

            let mut current_paths = match env::var_os("PATH") {
                Some(val) => env::split_paths(&val).collect::<Vec<_>>(),
                None => Vec::new(),
            };
            #[cfg(not(windows))]
            current_paths.insert(0, bin_dir.clone());
            #[cfg(target_os = "macos")]
            {
                current_paths.push(PathBuf::from("/Applications/IINA.app/Contents/MacOS"));
                current_paths.push(PathBuf::from("/Applications/VLC.app/Contents/MacOS"));
                current_paths.push(PathBuf::from("/opt/homebrew/bin"));
                current_paths.push(PathBuf::from("/usr/local/bin"));
            }
            #[cfg(windows)]
            {
                if let Ok(local_app) = env::var("LOCALAPPDATA") {
                    let local = PathBuf::from(local_app);
                    current_paths.push(local.join(r"Microsoft\WinGet\Links"));
                    current_paths.push(local.join(r"Programs\mpv"));
                }
                if let Ok(user_profile) = env::var("USERPROFILE") {
                    let user = PathBuf::from(user_profile);
                    current_paths.push(user.join(r"scoop\shims"));
                }
                current_paths.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
                current_paths.push(PathBuf::from(r"C:\Program Files\VideoLAN\VLC"));
                current_paths.push(PathBuf::from(r"C:\Program Files (x86)\VideoLAN\VLC"));
            }
            if let Ok(new_path) = env::join_paths(current_paths) {
                cmd.env("PATH", new_path);
            }

            cmd.env("SOLUS_REAL_PLAYER", &player);
            cmd.env("SOLUS_AUTO_FULLSCREEN", if opts.auto_fullscreen { "1" } else { "0" });
            cmd.env("SOLUS_AUTO_PIP", if opts.auto_pip { "1" } else { "0" });
            if opts.dub {
                cmd.env("SOLUS_AUDIO_DUB", "1");
            } else {
                cmd.env("SOLUS_AUDIO_DUB", "0");
            }

            if subs_disabled {
                cmd.env("SOLUS_SUB_DISABLED", "1");
            } else if let Some(ref url) = opts.sub_url {
                if !url.is_empty() {
                    cmd.env("SOLUS_SUB_URL", url);
                }
            }
            if opts.sub_delay_ms != 0 {
                cmd.env(
                    "SOLUS_SUB_DELAY",
                    format!("{:.3}", opts.sub_delay_ms as f64 / 1000.0),
                );
            }

            return cmd
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| AppError::Player(format!("Failed to spawn direct player: {e}")));
        }
    }

    // 2. Fallback playback via ani-cli
    #[cfg(windows)]
    let mut cmd = {
        if let Some(target) = find_windows_ani_cli() {
            let ext = target.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat") {
                let mut c = Command::new("cmd");
                c.arg("/C").arg(target);
                c
            } else {
                Command::new(target)
            }
        } else {
            return Err(AppError::Player(
                "ani-cli was not found. Install it via 'scoop install ani-cli' or select AllAnime provider for direct streaming.".to_string(),
            ));
        }
    };
    #[cfg(not(windows))]
    let mut cmd = Command::new("ani-cli");

    // Prepend wrapper directory and well-known tool directories to PATH
    let mut current_paths = match env::var_os("PATH") {
        Some(val) => env::split_paths(&val).collect::<Vec<_>>(),
        None => Vec::new(),
    };
    #[cfg(not(windows))]
    current_paths.insert(0, bin_dir);
    #[cfg(windows)]
    {
        if let Ok(local_app) = env::var("LOCALAPPDATA") {
            let local = PathBuf::from(local_app);
            current_paths.push(local.join(r"Microsoft\WinGet\Links"));
            current_paths.push(local.join(r"Programs\mpv"));
        }
        if let Ok(user_profile) = env::var("USERPROFILE") {
            let user = PathBuf::from(user_profile);
            current_paths.push(user.join(r"scoop\shims"));
        }
        current_paths.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
        current_paths.push(PathBuf::from(r"C:\Program Files\VideoLAN\VLC"));
        current_paths.push(PathBuf::from(r"C:\Program Files\Git\bin"));
        current_paths.push(PathBuf::from(r"C:\Program Files\Git\usr\bin"));
    }
    if let Ok(new_path) = env::join_paths(current_paths) {
        cmd.env("PATH", new_path);
    }

    cmd.args(&args)
        .env("ANI_CLI_EXIT_AFTER_PLAY", "1")
        .env("ANI_CLI_NO_DETACH", "1");

    #[cfg(windows)]
    {
        cmd.env("ANI_CLI_PLAYER", &player);
    }
    #[cfg(not(windows))]
    {
        cmd.env("ANI_CLI_PLAYER", &wrapper_name);
        cmd.env("SOLUS_REAL_PLAYER", &player);
    }

    if opts.dub {
        cmd.env("SOLUS_AUDIO_DUB", "1");
    } else {
        cmd.env("SOLUS_AUDIO_DUB", "0");
    }
    cmd.env("SOLUS_AUTO_FULLSCREEN", if opts.auto_fullscreen { "1" } else { "0" });
    cmd.env("SOLUS_AUTO_PIP", if opts.auto_pip { "1" } else { "0" });
    if let Some(ref script_path) = audio_script {
        cmd.env("SOLUS_AUDIO_SCRIPT", script_path);
    }

    if let Some(ref lang) = opts.sub_lang {
        if !lang.is_empty() {
            cmd.env("ANI_CLI_SUB_LANG", lang);
        }
    }

    if let Some(ref url) = opts.sub_url {
        if !url.is_empty() {
            cmd.env("SOLUS_SUB_URL", url);
        }
    }

    let mut player_flags = Vec::new();
    if player.contains("iina") {
        player_flags.push("--keep-running".to_string());
        player_flags.push("--no-stdin".to_string());
        if opts.auto_pip {
            player_flags.push("--mpv-fullscreen=no".to_string());
            player_flags.push("--mpv-ontop=yes".to_string());
            player_flags.push("--mpv-border=no".to_string());
            player_flags.push("--mpv-keepaspect-window=yes".to_string());
            player_flags.push("--mpv-window-scale=0.30".to_string());
            player_flags.push("--mpv-geometry=30%x30%-24-24".to_string());
        } else if opts.auto_fullscreen {
            player_flags.push("--mpv-fullscreen=yes".to_string());
        }
    } else if player.to_lowercase().contains("vlc") {
        player_flags.push("--play-and-exit".to_string());
        player_flags.push("--no-video-title-show".to_string());
        if opts.auto_pip {
            player_flags.push("--video-on-top".to_string());
        } else if opts.auto_fullscreen {
            player_flags.push("--fullscreen".to_string());
        }
        player_flags.push(format!("--network-caching={}", opts.network_mode.vlc_network_caching_ms()));
        if opts.force_ipv4 {
            player_flags.push("--ipv4".to_string());
        }
    } else {
        player_flags.push("--input-terminal=no".to_string());
        if opts.auto_pip {
            player_flags.push("--fullscreen=no".to_string());
            player_flags.push("--ontop=yes".to_string());
            player_flags.push("--border=no".to_string());
            player_flags.push("--keepaspect-window=yes".to_string());
            player_flags.push("--window-scale=0.30".to_string());
            player_flags.push("--geometry=30%x30%-24-24".to_string());
        } else if opts.auto_fullscreen {
            player_flags.push("--fullscreen=yes".to_string());
        }
        for net_flag in mpv_network_args(opts.network_mode, opts.force_ipv4, false) {
            if !net_flag.contains(' ') {
                player_flags.push(net_flag);
            }
        }
    }
    if let Some(ref conf_path) = yt_conf {
        if player.contains("iina") {
            player_flags.push(format!("--mpv-input-conf={}", conf_path.display()));
        } else if !player.contains("vlc") {
            player_flags.push(format!("--input-conf={}", conf_path.display()));
        }
    }

    if let Some(ref sock) = opts.ipc_socket {
        cmd.env("SOLUS_IPC_SOCKET", sock);
        if player.contains("iina") {
            player_flags.push(format!("--mpv-input-ipc-server={}", sock));
        } else if !player.contains("vlc") {
            player_flags.push(format!("--input-ipc-server={}", sock));
        }
    }

    if let Some(pos) = opts.start_pos {
        cmd.env("SOLUS_START_POS", pos.to_string());
        if player.contains("iina") {
            player_flags.push(format!("--mpv-start={}", pos));
        } else if player.contains("vlc") {
            player_flags.push(format!(":start-time={}", pos));
        } else {
            player_flags.push(format!("--start={}", pos));
        }
    }

    if let Some(ref shaders) = opts.glsl_shaders {
        cmd.env("SOLUS_SHADERS", shaders);
        if player.contains("iina") {
            player_flags.push(format!("--mpv-glsl-shaders={}", shaders));
        } else if !player.contains("vlc") {
            player_flags.push(format!("--glsl-shaders={}", shaders));
        }
    }

    let subs_disabled = opts.subtitles_disabled
        || opts
            .sub_lang
            .as_deref()
            .is_some_and(|l| l.eq_ignore_ascii_case("none"));
    if subs_disabled {
        cmd.env("SOLUS_SUB_DISABLED", "1");
        if player.contains("iina") {
            player_flags.push("--mpv-sid=no".to_string());
            player_flags.push("--mpv-sub-visibility=no".to_string());
        } else if player.contains("vlc") {
            player_flags.push("--no-spu".to_string());
        } else {
            player_flags.push("--sid=no".to_string());
            player_flags.push("--sub-visibility=no".to_string());
        }
    } else if let Some(ref url) = opts.sub_url {
        if !url.is_empty() {
            if player.contains("iina") {
                player_flags.push(format!("--mpv-sub-file={}", url));
                player_flags.push("--mpv-sub-visibility=yes".to_string());
            } else if player.contains("vlc") {
                player_flags.push(format!(":sub-file={}", url));
            } else {
                // Default: mpv. Pass without literal quotes so sh parameter expansion
                // passes a clean argument. --sub-files-clr guarantees any preceding
                // fallback sub-file is eliminated, and the user's chosen subtitle is rendered.
                player_flags.push("--sub-files-clr".to_string());
                player_flags.push(format!("--sub-file={}", url));
                player_flags.push("--sub-visibility=yes".to_string());
                player_flags.push("--sub-auto=fuzzy".to_string());
            }
        }
    }
    if opts.sub_delay_ms != 0 {
        let secs = format!("{:.3}", opts.sub_delay_ms as f64 / 1000.0);
        cmd.env("SOLUS_SUB_DELAY", &secs);
        if player.contains("iina") {
            player_flags.push(format!("--mpv-sub-delay={}", secs));
        } else if !player.contains("vlc") {
            player_flags.push(format!("--sub-delay={}", secs));
        }
    }

    if let Some(ref style) = opts.sub_style {
        for flag in style.to_mpv_flags() {
            if player.contains("iina") {
                player_flags.push(format!("--mpv-{}", flag.trim_start_matches("--")));
            } else if !player.contains("vlc") {
                player_flags.push(flag);
            }
        }
    }

    cmd.env("ANI_CLI_PLAYER_FLAGS", player_flags.join(" "));

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Player(format!("Failed to spawn ani-cli: {e}")))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(title: &str, episode: u32, quality: &str, dub: bool) -> PlayOptions {
        PlayOptions {
            title:   title.to_string(),
            episode,
            quality: quality.to_string(),
            dub,
            player:  "mpv".to_string(),
            sub_lang: None,
            sub_url:  None,
            stream_url: None,
            ipc_socket: None,
            start_pos:  None,
            glsl_shaders: None,
            sub_style: None,
            loading_image: None,
            sub_delay_ms: 0,
            subtitles_disabled: false,
            network_mode: crate::config::NetworkMode::Normal,
            force_ipv4: false,
            auto_fullscreen: false,
            auto_pip: false,
        }
    }

    #[test]
    fn test_build_args_sub_mode() {
        let args = build_args(&opts("Naruto", 1, "1080p", false));
        assert!(args.contains(&"-S".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"-e".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"-q".to_string()));
        assert!(args.contains(&"1080p".to_string()));
        assert!(!args.contains(&"--dub".to_string()));
        assert!(!args.contains(&"--no-detach".to_string()));
        assert!(args.contains(&"Naruto".to_string()));
    }

    #[test]
    fn test_build_args_dub_mode() {
        let args = build_args(&opts("One Piece", 5, "720p", true));
        assert!(args.contains(&"--dub".to_string()));
        assert!(args.contains(&"5".to_string()));
        assert!(args.contains(&"720p".to_string()));
        assert!(args.contains(&"One Piece".to_string()));
    }

    #[test]
    fn test_build_args_title_is_last() {
        let args = build_args(&opts("Frieren", 3, "best", false));
        assert_eq!(args.last().unwrap(), "Frieren");
    }

    #[test]
    fn test_build_args_episode_follows_flag() {
        let args = build_args(&opts("Test", 7, "720p", false));
        let e_pos = args.iter().position(|a| a == "-e").unwrap();
        assert_eq!(args[e_pos + 1], "7");
    }

    #[test]
    fn test_build_args_quality_follows_flag() {
        let args = build_args(&opts("Test", 1, "480p", false));
        let q_pos = args.iter().position(|a| a == "-q").unwrap();
        assert_eq!(args[q_pos + 1], "480p");
    }

    #[test]
    fn test_build_args_v4_14_contract() {
        let args = build_args(&PlayOptions {
            title:   "One Piece".to_string(),
            episode: 5,
            quality: "720p".to_string(),
            dub:     true,
            player:  "mpv".to_string(),
            sub_lang: None,
            sub_url:  None,
            stream_url: None,
            ipc_socket: None,
            start_pos:  None,
            glsl_shaders: None,
            sub_style: None,
            loading_image: None,
            sub_delay_ms: 0,
            subtitles_disabled: false,
            network_mode: crate::config::NetworkMode::Normal,
            force_ipv4: false,
            auto_fullscreen: false,
            auto_pip: false,
        });
        assert_eq!(
            args,
            vec!["-S", "1", "-e", "5", "-q", "720p", "--exit-after-play", "--dub", "One Piece"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<String>>()
        );
    }

    #[test]
    fn test_build_args_select_nth_precedes_title() {
        let args = build_args(&opts("Test", 1, "720p", false));
        let s_pos = args.iter().position(|a| a == "-S").unwrap();
        assert_eq!(args[s_pos + 1], "1");
        assert_eq!(args.last().unwrap(), "Test");
    }

    #[test]
    fn test_build_args_does_not_pollute_query_with_sub_lang() {
        let args = build_args(&PlayOptions {
            title:   "Frieren".to_string(),
            episode: 1,
            quality: "1080p".to_string(),
            dub:     false,
            player:  "mpv".to_string(),
            sub_lang: Some("Arabic".to_string()),
            sub_url:  Some("https://example.com/sub.vtt".to_string()),
            stream_url: None,
            ipc_socket: None,
            start_pos:  None,
            glsl_shaders: None,
            sub_style: None,
            loading_image: None,
            sub_delay_ms: 0,
            subtitles_disabled: false,
            network_mode: crate::config::NetworkMode::Normal,
            force_ipv4: false,
            auto_fullscreen: false,
            auto_pip: false,
        });
        assert_eq!(
            args,
            vec!["-S", "1", "-e", "1", "-q", "1080p", "--exit-after-play", "Frieren"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<String>>()
        );
        assert!(!args.contains(&"-L".to_string()));
        assert!(!args.contains(&"Arabic".to_string()));
    }

    #[test]
    fn test_resolve_player_keeps_vlc() {
        let resolved = resolve_player("vlc");
        assert!(resolved.to_lowercase().contains("vlc"));
    }

    #[test]
    fn test_ensure_player_wrapper_creates_executable_wrapper() {
        let wrapper = ensure_player_wrapper("mpv").expect("Failed to create mpv wrapper");
        assert!(wrapper.exists());
        assert!(wrapper.file_name().unwrap().to_str().unwrap().contains("mpv"));

        let content = std::fs::read_to_string(&wrapper).expect("Failed to read wrapper");
        assert!(content.contains("TYPE=AUDIO"));
        assert!(content.contains("SOLUS_SUB_URL"));
        assert!(content.contains("SOLUS_AUDIO_DUB"));

        let iina_wrapper = ensure_player_wrapper("iina").expect("Failed to create iina wrapper");
        assert!(iina_wrapper.file_name().unwrap().to_str().unwrap().contains("iina"));

        let vlc_wrapper = ensure_player_wrapper("vlc").expect("Failed to create vlc wrapper");
        assert!(vlc_wrapper.file_name().unwrap().to_str().unwrap().contains("vlc"));
        let vlc_content = std::fs::read_to_string(&vlc_wrapper).expect("Failed to read vlc wrapper");
        assert!(vlc_content.contains("--play-and-exit"));
        assert!(vlc_content.contains("--no-video-title-show"));

        let iina_content = std::fs::read_to_string(&iina_wrapper).expect("Failed to read iina wrapper");
        assert!(iina_content.contains("--keep-running"));
        assert!(iina_content.contains("--no-stdin"));
        assert!(iina_content.contains("--mpv-user-agent"));
    }

    #[test]
    fn test_derive_master_playlist_url() {
        assert_eq!(
            derive_master_playlist_url("https://cdn.example.com/hls/v1/1080/index.m3u8"),
            Some("https://cdn.example.com/hls/v1/master.m3u8".to_string())
        );
        assert_eq!(
            derive_master_playlist_url("https://cdn.example.com/index.m3u8"),
            None
        );
        assert_eq!(
            derive_master_playlist_url("https://cdn.example.com/hls/master.m3u8"),
            None
        );
    }

    #[test]
    fn test_ensure_youtube_input_conf_contains_youtube_bindings() {
        let path = ensure_youtube_input_conf().expect("youtube-input.conf path");
        assert!(path.is_file());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("k cycle pause"));
        assert!(text.contains("j seek -10"));
        assert!(text.contains("l seek 10"));
        assert!(text.contains("m cycle mute"));
        assert!(text.contains("f cycle fullscreen"));
        assert!(text.contains("c cycle sub-visibility"));
        assert!(text.contains("5 seek 50 absolute-percent"));

        let lua_path = ensure_solus_audio_lua().expect("solus-audio.lua path");
        assert!(lua_path.is_file());
        let lua_text = std::fs::read_to_string(&lua_path).unwrap();
        assert!(lua_text.contains("__SOLUS_AUDIO_CYCLE__"));
        assert!(lua_text.contains("relative+exact"));
    }

    #[test]
    fn test_build_mpv_direct_args_wraps_loading_screen_and_stream_per_file() {
        let mut o = opts("Serial Experiments Lain", 1, "1080p", false);
        o.loading_image = Some("/tmp/solus_y2k_0.jpg".to_string());
        let yt_conf = PathBuf::from("/tmp/youtube-input.conf");
        let args = build_mpv_direct_args(
            &o,
            "https://cdn.example.com/hls/v1/1080/index.m3u8",
            Some(&yt_conf),
        );
        assert!(args.contains(&"--prefetch-playlist=yes".to_string()));
        assert!(args.contains(&"--image-display-duration=4".to_string()));
        assert!(args.contains(&"--input-conf=/tmp/youtube-input.conf".to_string()));
        assert!(args.contains(&"--audio-file=https://cdn.example.com/hls/v1/master.m3u8".to_string()));
        let first_group = args.iter().position(|a| a == "--{").unwrap();
        assert_eq!(args[first_group + 1], "/tmp/solus_y2k_0.jpg");
        assert_eq!(args[first_group + 2], "--}");
        assert_eq!(args[first_group + 3], "--{");
        assert_eq!(args.last().unwrap(), "--}");
    }

    #[test]
    fn test_no_subtitles_and_sub_delay_mpv_flags() {
        let mut o = opts("Cowboy Bebop", 1, "1080p", true);
        o.subtitles_disabled = true;
        o.sub_url = Some("https://cdn.example.com/eng.vtt".to_string());
        o.sub_delay_ms = -250;

        let args = build_mpv_direct_args(&o, "https://cdn.example.com/stream.m3u8", None);
        assert!(args.contains(&"--sid=no".to_string()));
        assert!(args.contains(&"--sub-visibility=no".to_string()));
        assert!(!args.iter().any(|a| a.starts_with("--sub-file=")));
        assert!(args.contains(&"--sub-delay=-0.250".to_string()));

        o.subtitles_disabled = false;
        o.sub_delay_ms = 250;
        let args_pos = build_mpv_direct_args(&o, "https://cdn.example.com/stream.m3u8", None);
        assert!(args_pos.contains(&"--sub-file=https://cdn.example.com/eng.vtt".to_string()));
        assert!(args_pos.contains(&"--sub-delay=0.250".to_string()));
    }

    #[test]
    fn test_mpv_network_args_and_quality_clamp() {
        let normal = mpv_network_args(crate::config::NetworkMode::Normal, false, false);
        assert!(normal.contains(&"--hls-bitrate=max".to_string()));
        assert!(normal.contains(&"--demuxer-readahead-secs=45".to_string()));
        assert!(normal.contains(&"--cache-pause-wait=2.5".to_string()));
        assert!(normal.contains(&"--network-timeout=15".to_string()));
        assert!(!normal.contains(&"--cache-on-disk=yes".to_string()));
        assert!(normal.iter().any(|a| a.starts_with("--stream-lavf-o=reconnect=1")));

        let wifi = mpv_network_args(crate::config::NetworkMode::CongestedWifi, true, false);
        assert!(wifi.contains(&"--hls-bitrate=1800000".to_string()));
        assert!(wifi.contains(&"--demuxer-readahead-secs=120".to_string()));
        assert!(wifi.contains(&"--cache-pause-wait=4.5".to_string()));
        assert!(wifi.contains(&"--network-timeout=12".to_string()));
        assert!(wifi.contains(&"--cache-on-disk=yes".to_string()));
        assert!(wifi.contains(&"--ytdl-raw-options-append=force-ipv4=".to_string()));

        let eco = mpv_network_args(crate::config::NetworkMode::UltraSaver, false, false);
        assert!(eco.contains(&"--hls-bitrate=900000".to_string()));
        assert!(eco.contains(&"--demuxer-readahead-secs=180".to_string()));
        assert!(eco.contains(&"--cache-pause-wait=6.0".to_string()));

        assert!(mpv_network_args(crate::config::NetworkMode::UltraSaver, true, true).is_empty());

        let mut o = opts("Frieren", 1, "1080p", false);
        o.network_mode = crate::config::NetworkMode::CongestedWifi;
        let cli_args = build_args(&o);
        let q_pos = cli_args.iter().position(|a| a == "-q").unwrap();
        assert_eq!(cli_args[q_pos + 1], "720p");
    }

    #[test]
    fn test_auto_fullscreen_mpv_flags_and_wrapper_script() {
        let mut o = opts("Frieren", 1, "1080p", false);
        let windowed = build_mpv_direct_args(&o, "https://cdn.example.com/stream.m3u8", None);
        assert!(!windowed.contains(&"--fullscreen=yes".to_string()));

        o.auto_fullscreen = true;
        let fullscreen = build_mpv_direct_args(&o, "https://cdn.example.com/stream.m3u8", None);
        assert!(fullscreen.contains(&"--fullscreen=yes".to_string()));

        let wrapper = ensure_player_wrapper("mpv").unwrap();
        let script = std::fs::read_to_string(wrapper).unwrap();
        assert!(script.contains("SOLUS_AUTO_FULLSCREEN"));
        assert!(script.contains("SOLUS_AUTO_PIP"));
        assert!(script.contains("--fullscreen=yes"));
        assert!(script.contains("--mpv-fullscreen=yes"));

        o.auto_fullscreen = false;
        o.auto_pip = true;
        let pip_args = build_mpv_direct_args(&o, "https://cdn.example.com/stream.m3u8", None);
        assert!(pip_args.contains(&"--fullscreen=no".to_string()));
        assert!(pip_args.contains(&"--ontop=yes".to_string()));
        assert!(pip_args.contains(&"--border=no".to_string()));
        assert!(pip_args.contains(&"--window-scale=0.30".to_string()));
        assert!(pip_args.contains(&"--geometry=30%x30%-24-24".to_string()));
        assert!(pip_args.contains(&"--script-opts=solus_auto_pip=1".to_string()));

        let yt_path = ensure_youtube_input_conf().unwrap();
        let yt_text = std::fs::read_to_string(yt_path).unwrap();
        assert!(yt_text.contains("p script-binding solus-toggle-pip"));
        assert!(yt_text.contains("P script-binding solus-cycle-pip-corner"));
        assert!(yt_text.contains("i script-binding solus-toggle-hud"));
        assert!(yt_text.contains("I script-binding solus-pin-hud"));
        assert!(yt_text.contains("F1 script-binding stats/display-stats-toggle"));

        let lua_path = ensure_solus_audio_lua().unwrap();
        let lua_text = std::fs::read_to_string(lua_path).unwrap();
        assert!(lua_text.contains("solus-toggle-pip"));
        assert!(lua_text.contains("solus-toggle-hud"));
        assert!(lua_text.contains("SOLUS INFO HUD"));
    }
}

# ❖ SOLUS

**The fast, featherlight anime theatre with zero ads, built-in AniList sync, TV casting, mobile remote, and no browser required.**

https://github.com/user-attachments/assets/a7c6c35f-ac28-49ef-bb0f-fbfc72bc4177

`Solus` is an anime theatre that lives directly inside your terminal. You get crisp cover art, real episode thumbnails, cinema-grade typography presets, local phone remote companion, direct Smart TV casting, and full AniList integration—all without opening a single browser tab, installing three ad-blockers, or fighting pop-up windows just to click Play.

---

## ✦ Why Solus?

Watching anime on the web usually means choosing between a bloated streaming site that eats 3 GB of RAM, or a free site where clicking the unpause button opens four new tabs for crypto scams and questionable browser games.

**Solus fixes all of that by skipping the browser entirely:**

- **No Ads. No Pop-ups. No Ad-Blocker Required.** — When you press `Enter` on an episode, it opens cleanly in your media player (`mpv`, `iina`, or `vlc`). No banners, no redirects, no nonsense.
- **Cinematic Bento Home & Balanced Command Dock** — Top shelf pills (`1`–`6`), a 3-zone Spotlight Hero stage with live telemetry HUD, a Smart Bento dock pairing *Continue Watching* beside *My Watchlist*, and a cleanly structured 2-row Command Dock with clickable action chips that never clip.
- **Seasonal Anime & Movie Release Calendar (`c`)** — Browse weekly episode airings and movie/OVA premieres across past, current, and upcoming weeks (`H` / `L` or `[` / `]`). Tracked shows pin cleanly to the top of each day with local airtime countdowns, `All` / `Tracked` / `Movies` scope filters (`f`), one-key Watchlist toggling (`+`), and instant streaming (`Enter`) the moment an episode airs.
- **Split-Deck Fuzzy Feature Command Hub (`s`)** — An omni-searchable control palette inspired by Raycast and Neovim Telescope. Search any feature, typography style, or setting in real time with Skim fuzzy matching, preview details on the live inspector card, and trigger actions with `Enter`—no need to memorize cryptic hotkeys.
- **Cinema-Grade Subtitles & Signage Collision Guard** — Four hand-tuned typography presets (`Studio Minimal`, `Netflix Clean`, `Anime Fansub`, `OLED Plate`) with custom stroke outlines, drop shadows, and pill backdrops. Dialogue that collides with translated Japanese signs is automatically lifted upward in real time so you never miss a beat.
- **Zero-Setup Local LAN Mobile Remote Companion (`m`)** — Scan an in-terminal ASCII QR code from your smartphone to launch an embedded OLED touch remote on your local Wi-Fi. Scrub through episodes, play/pause, adjust volume, skip intro/outro, and switch sub/dub right from your couch.
- **Native AirPlay & Smart TV Direct Streamer (`c` on Detail)** — Scan your local Wi-Fi for Apple TV, LG webOS, Samsung Tizen, and Sony Bravia TVs. Beam decrypted 1080p/4K master streams directly to television hardware decoders with a full-screen TV Command Deck.
- **Real-Time Anime4K Upscaling & Adaptive Ambilight** — Direct GPU GLSL shader upscaling reconstructs crisp line-art in real time, while cover-adaptive Ambilight extracts vibrant color histograms from anime posters to softly tint terminal borders.
- **Watch in Your Language** — Flip between `SUB` and `DUB` with one key (`d`), or press `o` to choose your video quality (`1080p`, `720p`, `480p`) and subtitle language—including **English, Arabic, Spanish, French, German, Russian, Portuguese, Italian**, and more.
- **Seamless AniList Compatibility** — Connect your AniList account by username or drop in your saved JSON backup file (`s` → AniList). Your **Continue Watching** and **Watchlist** stay in sync, and dropped shows stay gone.
- **Fluid Background Auto-Updater (`u`)** — Zero-lag startup update checks throttled with a 4-hour SQLite cache, atomic in-place binary swapping, and multi-architecture binary distribution.
- **Discovery That Actually Knows Your Taste (`w`)** — Take a fast 10-question taste quiz to unlock **30 tailored matches** scored across genres, pacing, and mood. Toggle the **Live Vibe Tuner (`f`)** to interactively reshape results in real time without answering another quiz, or anchor similar picks to any show you love (`m`).
- **Anime Wrapped & Viewing Telemetry (`a`)** — Press `a` to view your total watch time, completed episodes, favorite studio affinity radar, and seasonal pacing graphs rendered in high-density Braille charts.

---

## ❖ One-Command Installation

### Raspberry Pi / uConsole — Linux ARM64

This branch adds an optional native ARM64 installer, kept separate from the original
installers and Rust application so the changes can be reviewed and merged independently.
From this checkout, choose **Foot** (Wayland), **Kitty**, or keep your current terminal:

```bash
bash scripts/raspberry-pi/install.sh
# Or choose without prompting:
bash scripts/raspberry-pi/install.sh --terminal foot
bash scripts/raspberry-pi/install.sh --terminal kitty
```

Use a **64-bit OS** (`uname -m` must report `aarch64`). The first installation compiles
locally and can take considerable time and memory. Both Foot and Kitty displayed
cover art in testing on a Raspberry Pi Compute Module 5 uConsole running Debian 13.
See the [Raspberry Pi guide](docs/raspberry-pi.md) for USB downloads, build requirements,
checks, limitations, and Spanish instructions. Original installation instructions follow.

Paste **one command** for your platform—it automatically installs `solus`, `mpv`, `ani-cli`, `yt-dlp`, `ffmpeg`, and an image-capable terminal (`Kitty` on macOS/Linux, `WezTerm` on Windows) so everything works immediately.

### macOS & Linux
```bash
curl -fsSL https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.sh | bash
```
*Prefer to inspect the script first?*
```bash
curl -fsSLO https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.sh && less install.sh && bash install.sh
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.ps1 | iex
```
*Prefer to inspect the script first?*
```powershell
irm https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.ps1 -OutFile install.ps1; notepad install.ps1; .\install.ps1
```

Then open **Kitty** (macOS/Linux) or **WezTerm** (Windows) and run:
```bash
solus
```

> [!TIP]
> **Recommended Window Size**: Run your terminal **Maximized / Full Screen** (or at least **125 columns wide × 38 rows tall**) so HD anime covers and widescreen episode stills render without cramped text.

<details>
<summary><strong>Alternative Installs & Optional Players (Homebrew, IINA, VLC, Build from Source)</strong></summary>

#### Build from Source (Rust)
```bash
cargo install --git https://github.com/abdullah-binmadhi/Solus.git
```

#### Switching Media Players (`~/.config/solus/config.toml`)
Solus defaults to **`mpv`** (required for IPC progress tracking, Anime4K shaders, and mobile remote sync), and also supports **`iina`** (macOS) and **`vlc`**:

| Player | Platform | Install |
| :--- | :--- | :--- |
| **`mpv`** *(Default)* | macOS, Linux, Windows | Installed automatically by `install.sh` / `install.ps1` |
| **`iina`** | macOS | `brew install --cask iina` |
| **`vlc`** | macOS, Linux, Windows | `brew install --cask vlc` / `sudo apt install -y vlc` / `scoop install vlc` |

</details>

---

## ◈ Keyboard Controls

### Home Screen
| Key | Action |
| :--- | :--- |
| `j` / `k` or `↑` / `↓` | Move between category rows (*Continue Watching*, *Watchlist*, *Trending*, etc.) |
| `h` / `l` or `←` / `→` | Scroll anime cards left / right |
| `1` – `6` | Jump directly to a shelf matching the Top Bar shelf pills |
| `Enter` / `d` | Open the **Detail & Episode Inspector** screen for the highlighted anime |
| `r` | Immediately resume the next unwatched episode of the highlighted anime |
| `+` | Add or remove the highlighted anime from your **Watchlist** |
| `x` / `Del` | Remove highlighted anime from *Continue Watching* / *Watchlist* (or stop playback) |
| `c` / `C` | Open **Seasonal Anime & Movie Release Calendar** (weekly airings & countdowns) |
| `w` | Open **For You // Tailored** (top-30 recommendation engine & live vibe tuner) |
| `a` | Open **Anime Wrapped & Taste Analytics** (watch time, genres, telemetry) |
| `m` / `M` | Display **Mobile Touch Remote ASCII QR Code Modal** |
| `T` | Cycle color theme palette (`Solus Dark`, `Anime Violet`, etc.) |
| `L` | Open **AniList Login / Cloud Library Sync** |
| `s` | Open **Split-Deck Fuzzy Feature Command Hub** |
| `/` | Open instant fuzzy title search |
| `!` | Open Dependency Health Doctor (validates mpv, ani-cli, ffmpeg) |
| `u` | Check for Solus releases in the background |
| `U` | Trigger immediate auto-update and restart |
| `Shift+R` | Refresh Home rows from AniList |
| `?` | Open keyboard shortcuts help modal |
| `q` / `Esc` | Quit Solus |

### Seasonal & Movie Release Calendar (`c`)
| Key | Action |
| :--- | :--- |
| `h` / `l` or `←` / `→` | Move between `MON`–`SUN` (rolls seamlessly into previous/next week at edges) |
| `1` – `7` | Jump directly to a weekday (`Monday`–`Sunday`) |
| `H` / `L` or `[` / `]` | Jump backward or forward week-by-week (up to ±8 weeks) |
| `t` | Snap back to **Today** (*This Week*) |
| `j` / `k` or `↑` / `↓` | Browse chronological episodes and movie premieres for the selected day |
| `f` | Cycle scope filter (`All` → `Tracked` → `Movies`) |
| `Enter` | Stream aired episode/movie immediately, or view overview for upcoming releases |
| `+` | Toggle the focused anime in your **Watchlist** and pin it as Tracked |
| `d` | Open full **Detail & Episode Inspector** for the highlighted title |
| `r` | Force-refresh the currently displayed week from AniList |
| `Esc` / `q` / `c` | Close Calendar and return |

### Detail Screen & Split-Deck Episode Inspector
| Key | Action |
| :--- | :--- |
| `j` / `k` or `↑` / `↓` | Navigate across episode entries |
| `Enter` / `Space` | Play the selected episode |
| `Tab` / `p` | Toggle streaming provider between **ani-cli** and **AllAnime** |
| `c` / `C` | Open **AirPlay / DLNA TV Device Picker** to beam episode to Smart TV |
| `m` / `M` | Display **Mobile Touch Remote ASCII QR Code Modal** |
| `v` | Switch between **Split-Deck Inspector** (default) and **Horizontal List View** |
| `o` | Open **Stream & Subtitle Options** (`1080p`/`720p` + subtitle languages) |
| `w` | Mark the selected episode watched / unwatched |
| `+` | Add or remove the anime from your **Watchlist** |
| `n` | Play the next unwatched episode immediately |
| `d` | Switch audio between `SUB` and `DUB` |
| `s` | Open **Split-Deck Fuzzy Feature Command Hub** |
| `Esc` / `q` | Return to Home screen |

### Split-Deck Feature Command Hub (`s`)
| Key | Action |
| :--- | :--- |
| `Any typing` | Filter features, presets, and actions in real time using fuzzy scoring |
| `j` / `k` or `↑` / `↓` | Move selection pointer across filtered commands |
| `Ctrl+n` / `Ctrl+p` | Next / previous command item |
| `Enter` | Trigger the highlighted action (cycle preset, toggle option, launch modal, check updates) |
| `h` / `l` or `←` / `→` | Cycle option forward / backward on cycling rows |
| `Backspace` | Delete query characters |
| `Esc` | Clear query or exit Hub back to previous screen |

### Smart TV Command Deck (`c`)
| Key | Action |
| :--- | :--- |
| `Space` | Toggle TV playback (Play / Pause) |
| `h` / `l` or `←` / `→` | Seek ±10 seconds on the television |
| `q` / `Esc` | Stop casting and return to Detail screen |

### Tailored Discovery (`w`)
| Key | Action |
| :--- | :--- |
| `j` / `k` or `↑` / `↓` | Move through quiz choices or the **30 curated anime recommendations** |
| `Enter` | Confirm quiz answers / open the highlighted recommendation |
| `f` | Toggle the **Live Vibe Tuner** bar to interactively filter by mood, match mode, era, or tags |
| `m` | Search by an **Anchor Anime** ("More Like This") to pivot recommendations cleanly |
| `Space` | Toggle vibe chips directly inside the **Live Vibe Tuner** |
| `+` | Add or remove the focused recommendation from your **Watchlist** |
| `r` | Retake the taste quiz from the start |
| `Esc` | Close tuner / return to Home |

---

## ⬡ Configuration

Solus creates and synchronizes its configuration automatically at `~/.config/solus/config.toml` (with cross-platform fallbacks for macOS Application Support and Windows AppData):

```toml
# Solus Theatre Configuration (~/.config/solus/config.toml)
player            = "mpv"       # mpv | iina | vlc
quality           = "best"      # best | 1080p | 720p | 480p | 360p
audio_mode        = "sub"       # sub | dub
anilist_username  = ""          # Optional: AniList public username for auto-sync
anime4k_mode      = "off"       # off | balanced | ultra
ambilight_enabled = true        # Cover-adaptive RGB terminal border glow
auto_skip_intro   = true        # Automated AniSkip OP/ED jumping

[subtitle_style]
preset              = "studio_minimal" # studio_minimal | netflix_clean | anime_fansub | oled_plate | custom
collision_avoidance = true             # Real-time dialogue lifting over translated signage

[remote]
enabled = true                  # Background Tokio HTTP daemon on local LAN
port    = 8484                  # Touch client accessible at http://<LAN_IP>:8484

[cast]
default_protocol = "auto"       # auto | airplay | dlna

[update]
check_on_startup = true         # Background GitHub release checker
auto_install     = false        # Atomic binary replacement on new versions

[cache]
trending_ttl = 86400            # 24 hours
stable_ttl   = 604800           # 7 days
```

---

## ❖ Architecture & Ecosystem

```text
┌────────────────────────────────────────────────────────────────────────┐
│                          ❖ SOLUS RUNTIME CORE                          │
├───────────────────┬─────────────────────┬──────────────────────────────┤
│  Terminal View    │  Playback & Media   │  Network & Protocols         │
├───────────────────┼─────────────────────┼──────────────────────────────┤
│ Ratatui TUI Deck  │ mpv IPC Unix Socket │ AniList GraphQL Client       │
│ Sixels/Kitty Img  │ Direct HLS Streamer │ Local LAN Remote (Tokio 8484)│
│ Fuzzy Skim Hub    │ Subtitle Styler/ASS │ AirPlay & DLNA TV Sender     │
│ Cover Ambilight   │ Anime4K GLSL Engine │ GitHub Release Auto-Updater  │
└───────────────────┴─────────────────────┴──────────────────────────────┘
```

---

## ❖ License & Credits

- Licensed under the **[MIT License](LICENSE)**.
- Metadata powered by **[AniList](https://anilist.co)**, **[ani.zip](https://api.ani.zip)**, and **[Kitsu](https://kitsu.app)**.
- Playback engine powered by **[`ani-cli`](https://github.com/pystardust/ani-cli)** and **[`mpv`](https://mpv.io/)**.

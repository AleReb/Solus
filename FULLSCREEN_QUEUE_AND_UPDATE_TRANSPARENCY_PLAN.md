# SOLUS IMPLEMENTATION PLAN: Auto-Fullscreen on Launch, 90% Queue Completion Guard & Plain-English Update Transparency

> **Neovim Navigation Tip**: Use `/^## ` to jump between major sections or `]m` / `[m`.
> All ASCII sketches, tables, and diagrams are formatted to fit 88–100 column Neovim splits.

---

## 1. Executive Summary & Scope

This plan covers three user-requested upgrades for **Solus (`v1.0.7` -> next)**:

1. **Auto-Fullscreen on Player Launch (Toggled in Control Hub `[s]`)**:
   - Persistent `auto_fullscreen: bool` setting in `config.toml` (`Config`) and runtime `AppState`.
   - New toggle row **`Auto-Fullscreen on Launch`** inside **Control Hub v2 (`[s]` — `src/ui/settings.rs`)** under **`❖ VISUAL & PLAYBACK ENGINE`** with contextual inspector details.
   - Multi-player launch parity across **`mpv`** (`--fullscreen=yes`), **`iina` / `iina-cli`** (`--mpv-fullscreen=yes`), **`vlc`** (`--fullscreen`), and the **`ani-cli` wrapper script** (`SOLUS_AUTO_FULLSCREEN=1`).
   - Live mid-playback IPC sync (`{"command": ["set_property", "fullscreen", true/false]}`) so toggling the setting in Control Hub `[s]` while a video is open immediately switches the active `mpv`/`iina` window in addition to saving for future launches.

2. **Watch Queue Crash / Early-Close Protection & 90% Completion Removal Guard**:
   - **Root-Cause Diagnosis of User Bug Report**:
     1. *Premature pop on launch*: Previously, `play_queue_item` / `pop_next_for_advance` / `remove_at` removed an episode from `state.queue.items` the instant playback *started* and moved it into `state.queue.active_item` (marked `#[serde(skip)]`). If `mpv` crashed, the user closed the player early, or Solus exited, `PlaybackDone` set `state.queue.active_item = None`, permanently losing the unfinished episode from the queue.
     2. *Accidental jump to next episode on window close*: In `AppMessage::PlaybackDone` (`src/main.rs:624`), `natural_finish` was computed as `natural_eof || (exited_ok && !user_quit && elapsed_secs >= 3)`. When a user closed `mpv`, `IINA`, or `VLC` via the OS window close button (`X` / `Cmd+W` / `Alt+F4`) after watching for >=3 seconds, the player exited with code `0` (`exited_ok = true`, `user_quit = false`), tricking Solus into thinking the episode finished and **auto-advancing to the next episode by accident**.
     3. *Non-durable active item on disk*: Because `active_item` was `#[serde(skip)]`, a terminal or app crash while watching dropped the active episode from `<cache_dir>/active_queue_<profile>.json`.
   - **The Fix**:
     - Introduce `QUEUE_COMPLETION_THRESHOLD: f64 = 0.90` (**90% watched / within 5%–10% of completion**).
     - Track `from_queue: bool`, `progress_ratio: f64`, and `completion_reached: bool` on `ActiveNowItem` and **serialize `active_item` to disk** in `active_queue_<profile>.json` (removing `#[serde(skip)]`).
     - Update live progress via `WatchQueue::record_active_progress(anime_id, ep, pos, dur)` on every `AppMessage::PlaybackProgressUpdated`: once `pos / dur >= 0.90` (or `natural_eof` is reached), mark `completion_reached = true` and persist immediately.
     - Tighten `natural_finish` in `AppMessage::PlaybackDone` so IPC-monitored sessions (`mpv`/`iina`) **only** auto-advance when `natural_eof || active_completion_reached` (never on an early window close at `<90%`).
     - When playback ends early (`!natural_finish && !completion_reached`) for an item launched from the queue (`from_queue == true`), automatically call `state.queue.restore_unfinished_active_to_front()` so the unfinished episode is placed right back at **index `0` (`#1` in `UP NEXT`)** ready to be resumed at its saved timestamp with a single `[Enter]`.

3. **Orchestria-Style Plain-English Update Transparency Popup (During & After Updating)**:
   - Port Orchestria's (`/Users/abdullahbinmadhi/Projects/OmniRip/src/harvest/updater.rs` & `src/ui/update_modal.rs`) jargon-free release-notes engine and 4-stage transparency modal to Solus:
     - **Plain-English Translator (`simplify_release_notes`)**: Converts raw GitHub Release bodies, commit messages, or `<!-- solus-notes-start -->` blocks into up to **5 zero-jargon bullet points** tagged with color-coded `[NEW]`, `[FIXED]`, and `[IMPROVED]` badges. Strips commit hashes, file paths (`src/*.rs`), Rust module paths (`::`), PR numbers, and technical jargon (`m3u8`, `IPC`, `serde`, `sqlx`, `lavf`).
     - **Shown During the Update (`Available`, `Downloading`, and `RestartRequired` phases)**: Users see `"What's New & Fixed for You"` in plain English before clicking Update, **while the progress bar is downloading**, and when prompted to restart.
     - **Shown After Updating (`PostUpdateWelcome` on First Launch + `last_seen_build.json`)**: Even when **Silent Auto-Install** updates Solus quietly in the background, `record_pending_post_update_welcome(&info)` saves the plain-English summary to `<config_dir>/last_seen_build.json`. On the very next launch, Solus automatically presents the **`❖ Solus Just Got Better! (v...) ❖`** welcome popup explaining what changed and what was fixed in friendly, everyday language.
     - **On-Demand Review in Control Hub (`[s]`)**: Add **`What's New & Release Notes`** (`HubActionId::WhatsNewNotes`) under `❖ SYSTEM & MAINTENANCE` so users can re-open the plain-English summary anytime.

---

## 2. End-to-End Architecture Diagram

```text
+--------------------------------------------------------------------------------------------------+
| 1. AUTO-FULLSCREEN ON LAUNCH (src/config.rs + src/ui/settings.rs + src/api/player.rs + ipc.rs)   |
|                                                                                                  |
|  Control Hub [s] -> "Auto-Fullscreen on Launch" [✓] Enabled                                      |
|       |                                                                                          |
|       +--> Persists `auto_fullscreen = true` in `~/.config/solus/config.toml`                    |
|       +--> Live IPC (if mpv/iina active): `{"command":["set_property","fullscreen",true]}`       |
|       +--> Next Player Launch (`spawn_async` / `build_mpv_direct_args` / wrapper):               |
|              • mpv:      `--fullscreen=yes`                                                      |
|              • iina-cli: `--mpv-fullscreen=yes`                                                  |
|              • vlc:      `--fullscreen`                                                          |
|              • ani-cli:  `SOLUS_AUTO_FULLSCREEN=1` -> wrapper injects player fullscreen flag     |
+--------------------------------------------------------------------------------------------------+
                                                |
                                                v
+--------------------------------------------------------------------------------------------------+
| 2. QUEUE 90% COMPLETION GUARD & CRASH/EARLY-CLOSE RECOVERY (src/services/queue.rs + src/main.rs) |
|                                                                                                  |
|  Queue Item Launched (`from_queue = true`, `completion_reached = false`, `progress_ratio = 0.0`) |
|       |                                                                                          |
|       +--> `active_item` serialized in `<cache_dir>/active_queue_<profile>.json` (crash-safe!)   |
|       |                                                                                          |
|       +--> Live `PlaybackProgressUpdated(anime_id, ep, pos, dur)` from mpv IPC:                  |
|       |      • `progress_ratio = pos / dur`                                                      |
|       |      • Crossed `>= 0.90` (within 5-10% of end)?                                          |
|       |          YES -> `completion_reached = true` (marked finished; saved to disk)             |
|       |          NO  -> Protected (`completion_reached = false`)                                 |
|       |                                                                                          |
|       +--> `PlaybackDone { exited_ok, natural_eof, user_quit, elapsed_secs }`:                   |
|              • Case A: `natural_eof || completion_reached` (>=90% watched):                      |
|                  -> Episode is permanently consumed from queue (or rotated to tail if Loop ON)   |
|                  -> Auto-Advance pops & launches next queued episode `N+1` cleanly!              |
|              • Case B: Player closed early / crashed (`<90%` and `!natural_eof`):                |
|                  -> DO NOT auto-advance to `N+1`!                                                |
|                  -> `restore_unfinished_active_to_front()` pushes episode back to `items[0]`     |
|                  -> Toast: "◈ Player closed early (34%) — Ep 5 kept at front of queue to resume" |
|                  -> Next `[Enter]` in Queue resumes Ep 5 at exact saved timestamp!               |
+--------------------------------------------------------------------------------------------------+
                                                |
                                                v
+--------------------------------------------------------------------------------------------------+
| 3. PLAIN-ENGLISH UPDATE TRANSPARENCY (src/services/updater.rs + src/ui/update_modal.rs)          |
|                                                                                                  |
|  GitHub Release Body / Commit Log -> `simplify_release_notes(raw_body)`                          |
|       |  (Strips SHAs, file paths, Rust symbols, PR tags; translates to plain-English bullets)   |
|       v                                                                                          |
|  Up to 5 Bullets: `[NEW] ...` (Emerald) | `[FIXED] ...` (Cyan) | `[IMPROVED] ...` (Amber)        |
|       |                                                                                          |
|       +--> Phase 1: `Available`        -> Shows plain-English "What's New & Fixed for You"       |
|       +--> Phase 2: `Downloading`      -> Shows live progress gauge + plain-English notes below  |
|       +--> Phase 3: `RestartRequired`  -> Saves `<config_dir>/last_seen_build.json` + summary    |
|       +--> Phase 4: `PostUpdateWelcome`-> On first boot after update (incl. Silent Auto-Install),|
|                                           pops up "❖ Solus Just Got Better! (v1.0.8) ❖"          |
+--------------------------------------------------------------------------------------------------+
```

---

## 3. UI Sketches (Formatted for Neovim)

### Sketch 1 — Control Hub v2 (`[s]`) with `Auto-Fullscreen on Launch` & `What's New`

```text
╭─ ◈ SOLUS // CONTROL HUB v2  NET:FULL ────────────────────────────────────────────────────────────╮
│ ╭──────────────────────────────────────────────────────────────────────────────────────────────╮ │
│ │  [ NET:FULL ]  Uncapped · 45s buf  |  Auto-ABR: ON  |  IPv4: OFF  |  Remote: :8484 (1 cli)   │ │
│ ╰──────────────────────────────────────────────────────────────────────────────────────────────╯ │
│ ╭──────────────────────────────────────────────────────────────────────────────────────────────╮ │
│ │  0:All   1:Network   2:Playback   3:Remote   4:Subtitles   5:System  │ Filter: fullscreen_   │ │
│ ╰──────────────────────────────────────────────────────────────────────────────────────────────╯ │
│ ╭─ ACTIONS (22) ─────────────────────────────────────╮╭─ ◈ CONTEXTUAL INSPECTOR ───────────────╮ │
│ │  ❖ VISUAL & PLAYBACK ENGINE                        ││ ❖ VISUAL & PLAYBACK ENGINE · [✓] Enabled│ │
│ │  ▸ Auto-Fullscreen on Launch         [✓] Enabled   ││ Auto-Fullscreen on Launch              │ │
│ │    Anime4K Shader Engine                 [ off ▾ ] ││                                        │ │
│ │    Adaptive Ambilight Glow          [ ] Disabled   ││ Automatically enter fullscreen mode    │ │
│ │    Auto-Skip OP/ED (AniSkip)         [✓] Enabled   ││ immediately when launching the video   │ │
│ │  ◈ NETWORK & STREAMING                             ││ player, and toggle any active mpv      │ │
│ │    Adaptive Network Mode            [ NET:FULL ▾ ] ││ window live via IPC.                   │ │
│ │    Auto-ABR Downshift                [✓] Enabled   ││                                        │ │
│ │    Force IPv4 Transport                 [ ] Auto   ││ Player Launch Flags:                   │ │
│ │  ❖ SYSTEM & MAINTENANCE                            ││  • mpv  : --fullscreen=yes             │ │
│ │    Check for Solus Updates              [ v1.0.7 ] ││  • IINA : --mpv-fullscreen=yes         │ │
│ │    What's New & Release Notes       [ View Notes ] ││  • VLC  : --fullscreen                 │ │
│ │    Auto-Update Policy           [ Silent Install ] ││  • Key  : Press [f] in player anytime  │ │
│ │                                                    ││                                        │ │
│ │                                                    ││ Controls: [Enter / Space / ← / →]      │ │
│ ╰────────────────────────────────────────────────────╯╰────────────────────────────────────────╯ │
│    [↑↓] Navigate   [Tab / 0-5] Category   [←/→/Enter] Toggle   [N] Net Mode   [Esc] Close        │
╰──────────────────────────────────────────────────────────────────────────────────────────────────╯
```

---

### Sketch 2 — Session Queue (`[Q]`) 90% Completion Guard & Early-Close Retention

```text
╭─ ◈ SOLUS // WATCH QUEUE & PLAYLISTS · 󰀄 Default · Auto: ON · Loop: OFF ──────────────────────────╮
│ ╭─ ◆ SESSION QUEUE (3)  [ACTIVE] ────────────────────╮╭─ ❖ SAVED PLAYLISTS (2)  [Tab] ─────────╮ │
│ │ ╭────────────────────────────────────────────────╮ ││   ▶ ◈ Weekend Marathon     4 items     │ │
│ │ │ ▶ NOW  Sousou no Frieren — Episode 12          │ ││       Sousou no Frieren · Dandadan     │ │
│ │ │   1080p · SUB · 38% watched (Kept until 90%)   │ ││     ◈ Cyberpunk Rewatch    10 items    │ │
│ │ ╰────────────────────────────────────────────────╯ ││       Cyberpunk: Edgerunners           │ │
│ │  ── UP NEXT (Protected on early close / crash) ─── ││ ╭────────────────────────────────────╮ │ │
│ │  ▶ 1. Sousou no Frieren · Ep 13        [QUEUED ]   ││ │ ── SELECTED PLAYLIST ITEMS         │ │ │
│ │       1080p · SUB · English · MODERN · Waiting     ││ │   1. Sousou no Frieren · E12       │ │ │
│ │    2. Sousou no Frieren · Ep 14        [QUEUED ]   ││ │   2. Sousou no Frieren · E13       │ │ │
│ │       1080p · SUB · English · MODERN · Waiting     ││ │   3. Dandadan · E07                │ │ │
│ │    3. Dandadan · Ep 7                  [QUEUED ]   ││ │   4. Dandadan · E08                │ │ │
│ │       1080p · SUB · English · MODERN · Waiting     ││ ╰────────────────────────────────────╯ │ │
│ │ ────────────────────────────────────────────────── ││                                        │ │
│ │  Guard: Removes Ep 12 at >=90% or EOF (now 38%)    ││                                        │ │
│ │  If player closes early, Ep 12 stays at #1         ││                                        │ │
│ ╰────────────────────────────────────────────────────╯╰────────────────────────────────────────╯ │
│  [Enter] Play  [J/K] Move  [a] Audio  [l] Sub  [v] Qual  [c] Config  [d] Drop                    │
│  [s] Save Playlist  [L] Loop  [R] Shuffle  [Space] Auto  [C] Clear  [Tab] Playlists  [Esc] Close │
╰──────────────────────────────────────────────────────────────────────────────────────────────────╯

  ┌─ WHAT HAPPENS IF PLAYER CLOSES EARLY OR CRASHES AT 38% (10:15 / 24:00)? ─────────────────────┐
  │ 1. Auto-Advance is BLOCKED (38% < 90% threshold and !natural_eof) -> never jumps to Ep 13!  │
  │ 2. Ep 12 is automatically restored to #1 at the front of `SESSION QUEUE`:                    │
  │      ▶ 1. Sousou no Frieren · Ep 12  (Resume @ 10:15 · 38%)                     [ READY ]    │
  │        2. Sousou no Frieren · Ep 13                                             [QUEUED ]    │
  │ 3. Toast: "◈ Player closed early (38%) — Ep 12 kept at front of queue to resume"            │
  └──────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

### Sketch 3 — Plain-English Update Popup DURING an Update (`Available` & `Downloading`)

```text
╭────────────────────────────── ❖ Downloading Solus Update ❖ ──────────────────────────────╮
│                                                                                          │
│               Updating Solus:  v1.0.7 (8881520)   ──►   v1.0.8 (a4f9c12)                 │
│                                                                                          │
│  ╭─ Download Progress ────────────────────────────────────────────────────────────────╮  │
│  │█████████████████████████████████████████████░░░░░░░░░░░░░░░░░░░░░  68.4% (8.2 MB/s)│  │
│  ╰────────────────────────────────────────────────────────────────────────────────────╯  │
│                                                                                          │
│  What's New & Fixed in This Update (Plain English):                                      │
│                                                                                          │
│   • [NEW]      Added an Auto-Fullscreen setting in the Control Hub so your video         │
│                player can open in fullscreen automatically.                              │
│   • [FIXED]    Episodes now stay in your watch queue until you finish at least 90%       │
│                of the episode, so closing the player early never skips ahead.            │
│   • [NEW]      Added clear, jargon-free update summaries during and after updates        │
│                so you always know what changed and what was fixed.                       │
│   • [IMPROVED] Smoother video streaming that automatically adjusts quality on slow       │
│                Wi-Fi to prevent mid-episode buffering.                                   │
│                                                                                          │
│  Your anime progress, watch queue, playlists, and settings are safely preserved.         │
│                                                                                          │
│             [b / Esc] Finish Quietly in Background     (Auto-installs in ~2s)            │
╰──────────────────────────────────────────────────────────────────────────────────────────╯
```

---

### Sketch 4 — Post-Update Welcome Popup AFTER Updating (`PostUpdateWelcome` on First Boot)

```text
╭─────────────────────── ❖ Solus Just Got Better! (v1.0.8) ❖ ──────────────────────────────╮
│                                                                                          │
│  You are now running the latest version of Solus! Here is what changed in plain English: │
│                                                                                          │
│   • [NEW]      Added an Auto-Fullscreen setting in the Control Hub so your video         │
│                player can open in fullscreen automatically.                              │
│   • [FIXED]    Episodes now stay in your watch queue until you finish at least 90%       │
│                of the episode, so closing the player early never skips ahead.            │
│   • [NEW]      Added clear, jargon-free update summaries during and after updates        │
│                so you always know what changed and what was fixed.                       │
│   • [IMPROVED] Smoother video streaming that automatically adjusts quality on slow       │
│                Wi-Fi to prevent mid-episode buffering.                                   │
│                                                                                          │
│  All your watch history, AniList/MAL sync, saved playlists, and active queue are ready.  │
│  Tip: You can re-open these notes anytime from Control Hub [s] -> What's New.            │
│                                                                                          │
│                          [ Enter / Space / Esc ]  Start Watching                         │
╰──────────────────────────────────────────────────────────────────────────────────────────╯
```

---

## 4. Detailed Feature Specifications & File-by-File Blueprint

### Feature 1: Auto-Fullscreen on Launch + Control Hub Toggle

#### 1.1 `src/config.rs`
- Add `auto_fullscreen: bool` to `Config`:
  ```rust
  /// Automatically launch the external video player (mpv / IINA / VLC) in fullscreen mode
  #[serde(default)]
  pub auto_fullscreen: bool,
  ```
- In `Config::default()`, initialize `auto_fullscreen: false`.

#### 1.2 `src/state/mod.rs`
- Add `pub auto_fullscreen: bool` to `AppState` (initialized to `false` in `AppState::new()`, and synced from `cfg.auto_fullscreen` at startup in `src/main.rs`).

#### 1.3 `src/api/player.rs`
- Add `#[serde(default)] pub auto_fullscreen: bool` to `PlayOptions`.
- **`build_mpv_direct_args`**:
  - Right after `--input-terminal=no`, if `opts.auto_fullscreen` is `true`, push `"--fullscreen=yes".to_string()`.
  - Because `--fullscreen=yes` is placed at top-level before any `--{ ... --}` playlist entries, `mpv` opens in fullscreen immediately on the Y2K loading card and stays in fullscreen when transitioning to the stream (while still allowing the user to press `f` at any time to exit fullscreen).
- **`spawn_async` (direct stream branch)**:
  - `is_vlc`: if `opts.auto_fullscreen { cmd.arg("--fullscreen"); }`
  - `is_iina`: if `opts.auto_fullscreen { cmd.arg("--mpv-fullscreen=yes"); }`
  - `mpv`: covered by `build_mpv_direct_args`.
  - Pass `cmd.env("SOLUS_AUTO_FULLSCREEN", if opts.auto_fullscreen { "1" } else { "0" })`.
- **`ensure_player_wrapper` & `spawn_async` (fallback `ani-cli` branch)**:
  - In `ensure_player_wrapper` bash script:
    ```bash
    if [ "$SOLUS_AUTO_FULLSCREEN" = "1" ]; then
        if [ "$is_iina" -eq 1 ]; then
            args+=("--mpv-fullscreen=yes")
        elif [ "$is_vlc" -eq 1 ]; then
            args+=("--fullscreen")
        else
            args+=("--fullscreen=yes")
        fi
    fi
    ```
  - In `spawn_async` fallback branch: set `cmd.env("SOLUS_AUTO_FULLSCREEN", if opts.auto_fullscreen { "1" } else { "0" })` and push `--mpv-fullscreen=yes` / `--fullscreen` / `--fullscreen=yes` into `player_flags` when `opts.auto_fullscreen` is `true`.

#### 1.4 `src/services/ipc.rs`
- Add live IPC helper `build_set_fullscreen_commands(fullscreen: bool) -> Vec<Value>` and `send_set_fullscreen(fullscreen: bool) -> bool`:
  ```rust
  pub fn build_set_fullscreen_commands(fullscreen: bool) -> Vec<Value> {
      let label = if fullscreen {
          "◈ Solus: Auto-Fullscreen ON"
      } else {
          "◈ Solus: Auto-Fullscreen OFF"
      };
      vec![
          serde_json::json!({ "command": ["set_property", "fullscreen", fullscreen] }),
          serde_json::json!({ "command": ["show-text", label, 2500] }),
      ]
  }
  ```

#### 1.5 `src/ui/settings.rs` & `src/main.rs`
- Add `HubActionId::AutoFullscreen` to `HubActionId` in `src/ui/settings.rs`.
- Insert `HubItem` at the top of **`2. VISUAL & PLAYBACK ENGINE`** (`cat_idx: 2`):
  - `id: HubActionId::AutoFullscreen`
  - `label: "Auto-Fullscreen on Launch"`
  - `keywords: "fullscreen fs cinema launch window maximize player mpv iina vlc"`
  - `badge: if cfg.auto_fullscreen { "[✓] Enabled".into() } else { "[ ] Disabled".into() }`
  - `description: "Automatically enter fullscreen mode immediately when launching mpv, IINA, or VLC (and toggle any currently playing mpv window live via IPC)."`
  - `kind: HubActionKind::Toggle(cfg.auto_fullscreen)`
- In `render_inspector_card` (`src/ui/settings.rs`), add a rich inspector section for `HubActionId::AutoFullscreen` displaying the active player flag (`--fullscreen=yes` / `--mpv-fullscreen=yes` / `--fullscreen`) and reminder that `[f]` inside the player toggles fullscreen anytime.
- In `execute_hub_action` (`src/main.rs`), handle `HubActionId::AutoFullscreen`:
  - Toggle `cfg.auto_fullscreen = !cfg.auto_fullscreen;`
  - Update `state.auto_fullscreen = cfg.auto_fullscreen;`
  - Persist `let _ = cfg.save();`
  - Call `services::ipc::send_set_fullscreen(cfg.auto_fullscreen);`
  - Show toast: `"❖ Auto-Fullscreen on Launch: Enabled/Disabled"`.
- In `start_playback` (`src/main.rs`), enforce `opts.auto_fullscreen = cfg.auto_fullscreen;` so every playback path (Home, Detail, PlayOptions, Queue, Remote) honors the latest toggle state.

---

### Feature 2: Queue Crash / Early-Close Protection & 90% Completion Threshold

#### 2.1 `src/services/queue.rs`
- Define the completion threshold constant:
  ```rust
  /// Episode completion ratio (90% watched / 10% remaining) required before an active
  /// queued episode is considered finished and permanently removed from the queue.
  pub const QUEUE_COMPLETION_THRESHOLD: f64 = 0.90;
  ```
- Upgrade `ActiveNowItem` to be **serializable** (`Serialize, Deserialize`) and track queue origin and live progress:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct ActiveNowItem {
      pub item:               QueueItem,
      #[serde(default)]
      pub used_prefetched:    bool,
      #[serde(default)]
      pub retried_once:       bool,
      /// True when this playback session was launched from the Session Queue / Playlist.
      #[serde(default)]
      pub from_queue:         bool,
      /// True once playback reached >= 90% (`QUEUE_COMPLETION_THRESHOLD`) or natural EOF.
      #[serde(default)]
      pub completion_reached: bool,
      /// Latest observed `pos / duration` ratio (`0.0 ..= 1.0`).
      #[serde(default)]
      pub progress_ratio:     f64,
  }
  ```
- In `WatchQueue`:
  - Change `pub active_item: Option<ActiveNowItem>` from `#[serde(skip)]` to `#[serde(default)]` so `<cache_dir>/active_queue_<profile>.json` preserves the active item across app/terminal crashes!
  - Update `normalize_uids(&mut self)` (called by `load_active_queue` on startup):
    - If `self.active_item.take()` is `Some(active)` where `active.from_queue && !active.completion_reached`:
      - Automatically restore `active.item` to the front of `self.items` (deduplicating if `(anime_id, episode)` is already present) before re-numbering `uid`s!
      - This guarantees that even if Solus or the whole computer crashes mid-episode, re-opening Solus has that unfinished episode waiting at `#1` in the queue!
  - Add helper methods to `WatchQueue`:
    1. `pub fn record_active_progress(&mut self, anime_id: i64, episode: u32, pos: f64, duration: f64) -> bool`:
       - If `self.active_item` matches `(anime_id, episode)` and `duration >= 60.0`:
         - Computes `ratio = (pos / duration).clamp(0.0, 1.0)`.
         - Updates `active.progress_ratio = active.progress_ratio.max(ratio)`.
         - If `!active.completion_reached && active.progress_ratio >= QUEUE_COMPLETION_THRESHOLD`:
           - Sets `active.completion_reached = true`.
           - Returns `true` (signaling caller to persist `save_active_queue` immediately so the episode is now durably marked completed).
       - Returns `false` otherwise.
    2. `pub fn mark_active_completed(&mut self)`:
       - Sets `completion_reached = true` and `progress_ratio = 1.0` on `self.active_item`.
    3. `pub fn restore_unfinished_active_to_front(&mut self) -> Option<QueueItem>`:
       - Checks `self.active_item.take()`.
       - If `active.from_queue && !active.completion_reached`:
         - Removes any existing duplicate `(active.item.anime_id, active.item.episode)` in `self.items`.
         - Allocates a fresh `uid`, clears `resolving`, and pushes `active.item` to the **front** (`self.items.push_front(item)`).
         - Returns `Some(restored_item)` so the UI can reset `state.queue_cursor = 0` and notify the user.
       - Otherwise returns `None`.
    4. Update `pop_next_for_advance(&mut self) -> Option<QueueItem>`:
       - Only rotates `self.active_item.take()` to the tail in `loop_mode` when `finished.completion_reached` is `true` (or when `pop_next_for_advance` is called on a completed episode).

#### 2.2 `src/main.rs`
- **Track `from_queue: true` in `play_queue_item` (`src/main.rs:6944`)**:
  - When `play_queue_item` sets `state.queue.active_item`, set:
    ```rust
    state.queue.active_item = Some(services::queue::ActiveNowItem {
        item: item.clone(),
        used_prefetched: actually_prefetched,
        retried_once,
        from_queue: true,
        completion_reached: false,
        progress_ratio: 0.0,
    });
    ```
  - And in `start_playback` (`src/main.rs:9026`), when playback was started directly from Home/Detail (not via `play_queue_item`), if the user happened to play the exact same `(anime_id, episode)` that is currently at the front of `state.queue.items`, pop it and set `from_queue: true`; otherwise set `from_queue: false`.
  - Also align the timestamp auto-resume threshold in `start_playback` (`src/main.rs:9063`) from `pos / dur < 0.85` to `pos / dur < services::queue::QUEUE_COMPLETION_THRESHOLD` (`0.90`) so any episode that closed early at `<90%` resumes at its exact saved timestamp!
- **Update live progress in `AppMessage::PlaybackProgressUpdated(anime_id, ep, pos, dur)` (`src/main.rs:1202`)**:
  - Call:
    ```rust
    if state.queue.record_active_progress(anime_id, ep, pos, dur) {
        services::queue::save_active_queue(&state.active_profile_id, &state.queue);
        sync_remote_queue_snapshot(&state);
    }
    ```
- **Fix `AppMessage::PlaybackDone` (`src/main.rs:569..645`)**:
  - Replace the buggy `let natural_finish = natural_eof || (exited_ok && !user_quit && elapsed_secs >= 3);` with completion-aware logic:
    ```rust
    let active_completion_reached = active_snapshot
        .as_ref()
        .is_some_and(|a| a.completion_reached || a.progress_ratio >= services::queue::QUEUE_COMPLETION_THRESHOLD);
    let had_ipc_duration = state.selected_anime.as_ref()
        .and_then(|a| state.anime_progress.get(&(active_snapshot.as_ref().map(|x| x.item.episode).unwrap_or(0))))
        .is_some_and(|(_, dur)| *dur >= 60.0);

    // An episode only finishes naturally if:
    // 1. mpv reported true EOF (`natural_eof`), OR
    // 2. Playback crossed the 90% completion threshold (`active_completion_reached`) without an explicit quit, OR
    // 3. A non-IPC external player (e.g. VLC without IPC duration telemetry) exited cleanly after >= 15 minutes.
    let natural_finish = natural_eof
        || (active_completion_reached && !user_quit)
        || (!had_ipc_duration && exited_ok && !user_quit && elapsed_secs >= 900);
    ```
  - When `natural_finish` is `true`:
    - Call `state.queue.mark_active_completed();`
    - If `!state.queue.manual_stop && state.queue.auto_advance`:
      - Pop and launch `next_item` via `state.queue.pop_next_for_advance()`.
  - When `natural_finish` is `false` (player crashed, stream dropped, or user closed the player window early at `<90%`):
    - **Do NOT auto-advance** to the next episode!
    - Call `if let Some(restored) = state.queue.restore_unfinished_active_to_front()`:
      - Set `state.queue_cursor = 0;`
      - Persist `services::queue::save_active_queue(&state.active_profile_id, &state.queue);`
      - Sync remote queue snapshot `sync_remote_queue_snapshot(&state);`
      - Show toast:
        ```rust
        let pct = (active_snapshot.as_ref().map(|a| a.progress_ratio * 100.0).unwrap_or(0.0)).round() as u32;
        state.show_toast(
            format!(
                "◈ Player closed early ({}%) — {} Ep {} kept at #1 in queue to resume",
                pct, restored.title, restored.episode
            ),
            unix_now(),
        );
        ```

#### 2.3 `src/ui/queue.rs`
- In `render_session_queue_pane`, enrich the `▶ NOW PLAYING` card subline when `state.queue.active_item` has `from_queue == true`:
  - Show live completion protection status:
    - Before 90%: `"1080p · SUB · 38% watched (Kept in queue until 90%)"`
    - At/After 90%: `"1080p · SUB · 92% ✓ Completed (Up Next armed)"`

---

### Feature 3: Orchestria-Style Plain-English Update Transparency Popup

#### 3.1 `src/services/updater.rs`
- Add `pub simple_notes: Vec<String>` to `UpdateInfo`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  pub struct UpdateInfo {
      pub current_version: String,
      pub latest_version: String,
      pub download_url: String,
      pub asset_name: String,
      pub release_notes: Option<String>,
      pub simple_notes: Vec<String>,
      pub is_homebrew: bool,
  }
  ```
- Port and adapt `simplify_release_notes(raw_body: &str) -> Vec<String>` from Orchestria (`OmniRip/src/harvest/updater.rs`):
  - **Priority 1**: Parse explicit `<!-- solus-notes-start --> ... <!-- solus-notes-end -->` block if present in the GitHub release body.
  - **Priority 2**: Parse `Simple:` / `Note:` lines or `[NEW]` / `[FIXED]` / `[IMPROVED]` lines.
  - **Priority 3**: Filter out non-user-facing noise (`Merge pull request`, `Merge branch`, `chore`, `ci:`, `test:`, `docs:`, `style:`, `build:`, `bump`, `<!-- solus-... -->`, `Full Changelog`, `Automated build for commit` header wrapper while extracting the commit summary after `: `).
  - **Priority 4**: Apply Solus domain-specific plain-English rules (zero technical jargon):
    - *Fullscreen*: matches `"fullscreen"` / `"auto_fullscreen"` -> `"[NEW] Added an Auto-Fullscreen setting in the Control Hub so your video player can open in fullscreen automatically."`
    - *Queue 90% / early close / crash protection*: matches `("queue" || "que")` + `("90%" || "5%" || "10%" || "early" || "crash" || "restore" || "completion")` -> `"[FIXED] Episodes now stay in your watch queue until you finish at least 90% of the episode, so closing the player early or a crash never skips ahead by accident."`
    - *Update transparency / plain English / what's new*: matches `("update" || "release")` + `("plain" || "jargon" || "what's new" || "transparency" || "popup")` -> `"[NEW] Added clear, plain-English update summaries during and after updates so you always know what changed and what was fixed."`
    - *Network / Wi-Fi / ABR / buffering*: matches `"network_mode"` / `"auto-abr"` / `"abr"` / `"congested"` / `"readahead"` / `"rebuffer"` -> `"[IMPROVED] Smoother video streaming that automatically adjusts quality on slow or congested Wi-Fi to prevent buffering."`
    - *Mobile remote / QR / themes*: matches `"remote"` / `"qr"` / `"websocket"` -> `"[IMPROVED] Upgraded the phone remote control with smoother live playback sync and customizable themes."`
    - *Subtitles / sync / delay / collision*: matches `"subtitle"` / `"subsync"` / `"sub-delay"` / `"vtt"` -> `"[FIXED] Cleaner, better-timed subtitles that stay in sync with the voices and avoid overlapping on-screen text."`
    - *Audio / Sub vs Dub*: matches `"audio"` / `"dub"` / `"dual-audio"` -> `"[FIXED] Smoother switching between Japanese (Sub) and English (Dub) audio tracks."`
    - *Trackers / AniList / MAL / scrobble*: matches `"anilist"` / `"mal"` / `"tracker"` / `"scrobble"` -> `"[IMPROVED] More reliable episode progress saving and watchlist syncing with AniList and MyAnimeList."`
    - *Artwork / covers / thumbnails*: matches `"cover"` / `"thumbnail"` / `"portrait"` / `"banner"` -> `"[FIXED] Crisper anime cover art and episode thumbnails with smoother browsing."`
    - *Generic fallback (`humanize_free_text`)*: strips `src/`, `.rs`, `::`, backticks, hex hashes, PR numbers `(#123)`, and `by @user`, then prefixes with `[NEW]`, `[FIXED]`, or `[IMPROVED]`.
- Add `BUILTIN_CURRENT_RELEASE_NOTES: &[&str]` for the current release so even users who upgrade via `brew upgrade solus`, `install.sh`, or `cargo install` (or open **"What's New & Release Notes"** from Control Hub `[s]`) always see crisp plain-English release highlights.
- Port **`LastSeenBuild` persistence** (`<config_dir>/last_seen_build.json`):
  - `pub fn last_seen_build_path() -> Option<PathBuf>`
  - `pub fn record_pending_post_update_welcome(info: &UpdateInfo)` (called inside `download_and_install_update` right before `AppMessage::UpdateSuccess`, as well as on Homebrew update notice).
  - `pub fn check_or_take_post_update_welcome() -> Option<(String, Vec<String>)>`:
    - On startup, reads `last_seen_build.json`.
    - Case 1: `pending_whats_new` is non-empty -> consumes and clears `pending_whats_new`, updates `last_seen_version` & `last_seen_sha` to the running binary, writes `last_seen_build.json`, and returns `Some((version_label, notes))`.
    - Case 2: `last_seen_version` (or `last_seen_sha`) existed on disk and differs from the running binary's `CARGO_PKG_VERSION` / `current_build_sha()` (e.g. upgraded via Homebrew or `install.sh`) -> updates `last_seen_build.json` and returns `Some((current_version_label, default_current_release_notes()))`.
    - Case 3: First time creating `last_seen_build.json` -> writes current version/SHA and returns `None` so fresh installs aren't spammed on first run.

#### 3.2 `src/state/mod.rs` & `src/ui/update_modal.rs`
- Update `UpdateModalPhase` in `src/state/mod.rs`:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub enum UpdateModalPhase {
      /// An update is available; user can press `[Enter / u]` to install, `[b]` background, or `[Esc]` snooze.
      Available {
          info: crate::services::updater::UpdateInfo,
      },
      /// Binary is actively downloading; displays live progress bar AND plain-English "What's New & Fixed".
      Downloading {
          info: crate::services::updater::UpdateInfo,
          progress_pct: f32,
          speed: String,
      },
      /// Binary has been atomically replaced on disk; shows plain-English summary and prompts to restart.
      RestartRequired {
          new_version: String,
          simple_notes: Vec<String>,
      },
      /// First launch after an update completed (or opened via Control Hub "What's New"):
      /// celebrates the update and lists what changed & what was fixed without technical jargon.
      PostUpdateWelcome {
          version_label: String,
          simple_notes: Vec<String>,
      },
      /// Update failed; displays friendly error and leaves current install untouched.
      Failed {
          error: String,
      },
  }
  ```
- Add `AppState::open_post_update_welcome(&mut self, version_label: String, simple_notes: Vec<String>)`.
- Redesign `src/ui/update_modal.rs` (`centered_rect(72, 60, frame.area())`):
  - Add `format_plain_english_bullet(note: &str, max_width: usize) -> Vec<Line<'static>>` that renders wrapped multi-line bullets with color-coded badges:
    - `[NEW]` in bold Emerald (`Color::Rgb(80, 250, 123)`)
    - `[FIXED]` in bold Cyan (`Color::Rgb(0, 210, 255)`)
    - `[IMPROVED]` in bold Amber (`Color::Rgb(251, 191, 36)`)
  - **`render_available`**: Displays version diff (`Current ──► New`), `"What's New & Fixed for You (Plain English):"` with the color-coded bullets, safety reassurance line, and actions `[Enter / u] Update Now`, `[b] Update in Background`, `[Esc] Remind Me Later`.
  - **`render_downloading`**: Displays version header, live Ratatui `Gauge` + transfer speed (`MB/s`), **AND** the `"What's New & Fixed in This Update:"` plain-English bullets below the gauge so the user can read them during the download, plus `[b / Esc] Finish in Background`.
  - **`render_restart_required`**: Displays `"✓ Solus has been updated to <version>!"`, the `"What's Included:"` plain-English bullets, and `[Enter] Exit & Relaunch Now` / `[Esc] Continue Current Session`.
  - **`render_post_update_welcome`**: Displays title `" ❖ Solus Just Got Better! (v<version>) ❖ "` in Emerald border, `"You are now running the latest version of Solus! Here is what changed:"`, the color-coded plain-English `[NEW]` / `[FIXED]` / `[IMPROVED]` bullets, reassurance that all watchlists/queues/settings are intact, and `[Enter / Space / Esc] Start Watching`.

#### 3.3 `src/main.rs` & Control Hub Integration
- On startup in `src/main.rs` (right after `state` initialization):
  - Call `if let Some((ver_label, notes)) = services::updater::check_or_take_post_update_welcome() { state.open_post_update_welcome(ver_label, notes); }`.
- In `spawn_update_check` and `handle_update_modal`:
  - Pass `&info` into `download_and_install_update` (or call `services::updater::record_pending_post_update_welcome(&info)` before/on completion) so both **Silent Auto-Install** and **Interactive Update** persist the plain-English release notes for the post-update welcome popup!
  - In `handle_update_modal`:
    - In `Available`: support `[b]` / `[B]` to close the modal while starting the download in the background (`state.show_toast("❖ Downloading update in background...", unix_now())`).
    - In `Downloading`: keep `info` inside `UpdateModalPhase::Downloading { info, progress_pct, speed }` so the plain-English bullets remain visible throughout the download; pressing `[b]` or `[Esc]` hides the modal to finish in the background.
    - In `PostUpdateWelcome`: pressing `Enter`, `Esc`, `Space`, or `q` calls `state.close_update_modal()`.
- In `src/ui/settings.rs` (`get_hub_items`) & `src/main.rs` (`execute_hub_action`):
  - Add `HubActionId::WhatsNewNotes` under `5. SYSTEM & MAINTENANCE`:
    - `label: "What's New & Release Notes"`
    - `keywords: "whats new release notes changelog update fixes features plain english"`
    - `badge: "[ View Notes ]".into()`
    - `description: "Open a plain-English, jargon-free summary of what is new, improved, and fixed in this version of Solus."`
    - `kind: HubActionKind::Launch("Open What's New")`

---

## 5. Verification & Test Plan

1. **Unit Tests (`cargo test`)**:
   - `config::tests::test_auto_fullscreen_config_roundtrip`: Verifies `auto_fullscreen` defaults to `false` and serializes/deserializes cleanly in `config.toml`.
   - `api::player::tests::test_auto_fullscreen_flags_across_mpv_iina_vlc_and_wrapper`: Verifies `build_mpv_direct_args` includes `--fullscreen=yes` when `auto_fullscreen: true` and omits it when `false`, and checks `ipc::build_set_fullscreen_commands`.
   - `services::queue::tests::test_queue_retains_item_on_early_close_and_removes_at_90_pct`:
     - Launches item #1 (`from_queue = true`), simulates progress at `35%` (`500s / 1420s`), verifies `completion_reached == false`, simulates early player close, verifies `restore_unfinished_active_to_front()` restores item #1 to `queue.items[0]` and does NOT advance to item #2.
     - Simulates progress crossing `91%` (`1300s / 1420s`), verifies `record_active_progress` returns `true` and sets `completion_reached = true`, and verifies `PlaybackDone` advances to item #2 without re-inserting item #1.
     - Simulates crash recovery via JSON serialization/deserialization of `WatchQueue` with an unfinished `active_item`, verifying `normalize_uids()` restores the unfinished episode to `items[0]`.
   - `services::updater::tests::test_simplify_release_notes_strips_jargon_and_formats_badges`:
     - Feeds raw commit messages, GitHub PR trailers, file paths (`src/services/queue.rs`), and `<!-- solus-notes-start -->` blocks into `simplify_release_notes`, asserting output contains only clean `[NEW]`, `[FIXED]`, `[IMPROVED]` plain-English sentences with zero technical jargon.
   - `services::updater::tests::test_post_update_welcome_persistence_roundtrip`:
     - Verifies `record_pending_post_update_welcome_at` and `take_post_update_welcome_at` persist and consume the post-update welcome notes once.
   - `ui::update_modal::tests::test_render_all_update_modal_phases_with_plain_english_notes`:
     - Renders `Available`, `Downloading`, `RestartRequired`, and `PostUpdateWelcome` on a Ratatui `TestBackend`, verifying plain-English bullets render cleanly without overflow.
2. **Static Analysis**:
   - `cargo clippy --all-targets -- -D warnings` (0 warnings).

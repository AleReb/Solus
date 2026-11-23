# Solus — Unified Watchlist Status Labels & Bento List Picker Plan

> **Goal:**
> 1. **Pull Everything from AniList & MyAnimeList** (all **Watching**, **Completed**, **On-Hold**, **Dropped**, and **Plan to Watch** shows) into **My Watchlist** so zero shows go missing.
> 2. **Display Crisp Native Solus Status Labels** right next to the rating and show type (`★ 8.8 · TV ·  Watch`), using native Solus Nerd Font glyphs & RGB colors—zero emoji slop and zero word concatenation.
> 3. **Intuitive Bento Status & List Picker Popup (`[+]`)** to label, move, or remove any anime across AniList, MyAnimeList, Both, or Local in 1–2 keystrokes.

---

## 1. Plain-English Overview (Why Shows Were Missing & What Changes)

- **Why your 3 MAL shows + 14 AniList shows were hidden:**
  - Right now, Solus only places shows labeled **"Plan to Watch"** into **My Watchlist**, and removes shows marked **Watching**, **Completed**, **On-Hold**, or **Dropped** from that row.
  - Because your MAL account had `2 Watching` (`0/64`, `0/12`) + `1 Completed` (`28/28`) + `0 Plan to Watch`, all 3 MAL shows were filtered out—and 14 of your 18 AniList shows were hidden for the same reason.
- **What this upgrade fixes:**
  1. **Every list status syncs into My Watchlist** (`Watching`, `Plan to Watch`, `Completed`, `On-Hold`, `Dropped`) from **AniList**, **MyAnimeList**, and **Local**, merged with zero duplicates.
  2. Shows marked **Watching** (even at Episode `0`) also appear in **Continue Watching** ready at `E1 next`.
  3. Each Watchlist card shows its exact **Status Label** cleanly next to its rating and format (` 8.8 · TV ·  Watch`), separated by `" · "` so words never run together.
  4. Pressing **`+`** on any anime opens a clean **Status & List Picker** popup where you pick the label (`Watching`, `Plan to Watch`, `Completed`, `On-Hold`, `Dropped`, or `Remove`) **and** pick where to save it (`Both`, `AniList`, `MyAnimeList`, or `Local`)—all in one place.

---

## 2. Visual Sketch Designs (Solus Bento Style)

### Sketch A — Bento "Status & Tracker List Picker" Popup (`[+]` on Any Anime)

```text
╭─ ✦ UPDATE ANIME STATUS & TRACKER LIST ─────────────────────────── ◆ ACTIVE ─╮
│  Show:    Fullmetal Alchemist: Brotherhood  (TV · 64 EPS ·  9.1)           │
│  Current: [  WATCHING ] on ◆ MyAnimeList  ·  Progress: 0 / 64 EPS          │
│                                                                             │
│  Where to Save  ([Tab / ←→] Switch Target DB):                              │
│  ╭──────────────────────╮ ╭──────────────╮ ╭───────────────╮ ╭────────────╮ │
│  │ ◈ Both (AL + MAL) ✓  │ │ ✦ AniList    │ │ ◆ MyAnimeList │ │ • Local    │ │
│  ╰──────────────────────╯ ╰──────────────╯ ╰───────────────╯ ╰────────────╯ │
│  ─────────────────────────────────────────────────────────────────────────  │
│   ▶ [1]     Watching        Active rotation — tracks episodes as you watch │
│     [2]  󰃰   Plan to Watch   Saved in your backlog for later                │
│     [3]  󰄬   Completed       Finished all episodes (marks 64/64 watched)    │
│     [4]  󰏤   On-Hold         Paused temporarily — keeps your episode spot   │
│     [5]  󰅖   Dropped         Stopped watching — kept in your list history   │
│     [6]  󰆴   Remove Entry    Delete completely from selected database(s)    │
│                                                                             │
│  [1-6] Quick Apply   [↑↓/jk] Pick Label   [Tab/←→] Target DB   [Esc] Close  │
╰─────────────────────────────────────────────────────────────────────────────╯
```

### Sketch B — Enhanced "My Watchlist" Shelf With Native Solus Status Labels

```text
── 󰋜 My Watchlist (21) [✦AL:14 · ◆MAL:3 · ◈Both:3 · •Loc:1] ────────── ●○○○○ [ 1/21 ] ──
✦ AL  ╭  FOCUS ─── [AL]╮  ╭──────────── [AL]╮  ◆ MAL ╭─────────── [MAL]╮  ╭─────────── [MAL]╮
  │   │                 │  │                 │    │   │                 │  │                 │
  │   │   COVER ART     │  │   COVER ART     │    │   │   COVER ART     │  │   COVER ART     │
  │   │   (PORTRAIT)    │  │   (PORTRAIT)    │    │   │   (PORTRAIT)    │  │   (PORTRAIT)    │
  │   │                 │  │                 │    │   │                 │  │                 │
  ╵   ╰─────────────────╯  ╰─────────────────╯    ╵   ╰─────────────────╯  ╰─────────────────╯
      Black Clover Seas…   Saga of Tanya th…          Fullmetal Alchem…    Sousou no Friere…
       8.8 · TV ·  Watch  8.0 · TV · 󰃰 Plan         9.1 · TV ·  Watch  9.3 · TV · 󰄬 Done
```

### Sketch C — Native Solus Status Glyphs, Colors & Anti-Concatenation Sizing

| Status | On-Card Tag (`18–21` cols) | Popup & Hero Label | Solus Glyph | Theme Color (RGB) |
| :--- | :--- | :--- | :--- | :--- |
| **Watching** | ` Watch` | ` Watching` | `` (Play) | Emerald `Rgb(80, 250, 123)` |
| **Plan to Watch** | `󰃰 Plan` | `󰃰 Plan to Watch` | `󰃰` (Bookmark) | Soft Lavender `Rgb(189, 147, 249)` |
| **Completed** | `󰄬 Done` | `󰄬 Completed` | `󰄬` (Check) | Royal Cyan `Rgb(103, 232, 249)` |
| **On-Hold** | `󰏤 Hold` | `󰏤 On-Hold` | `󰏤` (Pause) | Amber Gold `Rgb(251, 191, 36)` |
| **Dropped** | `󰅖 Drop` | `󰅖 Dropped` | `󰅖` (Cross) | Crimson Rose `Rgb(255, 105, 135)` |

- **Zero Word Concatenation Guarantee**:
  - Card metadata line uses explicit `" · "` separators (`Span::styled(" · ", theme.divider)`):
    - Wide cards (`19+` chars): ` 8.8 · TV ·  Watch` (20 chars)
    - `MOVIE` / long formats on narrow `18`-col cards shorten the format chip to `MOV` / `SPC` (` 8.0 · MOV · 󰃰 Plan` -> fits `18`–`20` cols cleanly without touching adjacent words or truncating the status label).
  - Shows inside each provider section (`✦ AL`, `◆ MAL`, `◈ BOTH`, `• LOC`) are sorted cleanly by status priority: `Watching → Plan to Watch → On-Hold → Completed → Dropped`.

---

## 3. Step-by-Step User Navigation Guide (Zero Technical Jargon)

1. **See Every Show Automatically**:
   - Open Solus or press **`R`** on Home after connecting **AniList** and/or **MyAnimeList** in **`L`**.
   - **My Watchlist** now displays **all** of your shows across all 5 statuses (`Watch`, `Plan`, `Done`, `Hold`, `Drop`) right next to each anime's rating and show type.
2. **Label or Change Any Anime's Status in 1 Keypress**:
   - Highlight any anime on Home (or inside its Episode / Detail screen) and press **`+`**.
   - Press **`Tab`** (or **`←`** / **`→`**) at the top of the popup if you want to save to **Both**, **AniList**, **MyAnimeList**, or **Local**.
   - Press **`1`–`6`** (or use **`↑`/`↓`** + **`Enter`**) to choose:
     - **`[1]` Watching**
     - **`[2]` Plan to Watch**
     - **`[3]` Completed**
     - **`[4]` On-Hold**
     - **`[5]` Dropped**
     - **`[6]` Remove Entry**
   - Solus immediately updates the badge on the card and syncs the new label live to **AniList** and/or **MyAnimeList**.

---

## 4. File-by-File Implementation Plan

### Step 1: Canonical `WatchStatus` Model (`src/config.rs`)
- Add `WatchStatus` enum (`Watching`, `Planning`, `Completed`, `Paused`, `Dropped`):
  - `from_tracker_status(raw: &str) -> Self` (maps `"CURRENT" | "REPEATING" | "watching"` → `Watching`; `"COMPLETED"` → `Completed`; `"PAUSED" | "on_hold"` → `Paused`; `"DROPPED"` → `Dropped`; `"PLANNING" | "plan_to_watch"` → `Planning`).
  - `to_tracker_status(self) -> &'static str` (`"CURRENT"`, `"PLANNING"`, `"COMPLETED"`, `"PAUSED"`, `"DROPPED"`).
  - `short_card_tag(self) -> &'static str` (`" Watch"`, `"󰃰 Plan"`, `"󰄬 Done"`, `"󰏤 Hold"`, `"󰅖 Drop"`).
  - `full_label(self) -> &'static str` (`"Watching"`, `"Plan to Watch"`, `"Completed"`, `"On-Hold"`, `"Dropped"`).
  - `order_rank(self) -> u8` (`0..=4`).

### Step 2: SQLite Status Column & Unfiltered Source Queries (`src/db/mod.rs`, `src/db/user.rs`)
- In `src/db/mod.rs`: safely ensure `watchlist` has column `status TEXT NOT NULL DEFAULT 'PLANNING'` (via `ALTER TABLE watchlist ADD COLUMN status ...` ignoring duplicate-column error on existing DBs).
- In `src/db/user.rs`:
  - Add `set_watchlist_status(pool, anime_id, status: WatchStatus, now)` to upsert/update an entry's status in `watchlist`.
  - Update `get_anime_watchlist_source` and `get_watchlist_with_sources` (`src/db/user.rs:1362–1426`) to inspect `tracker_entries` across **all 5 statuses** (remove `AND te.status = 'PLANNING'`), returning `(anime_id, WatchlistSource, WatchStatus)` ordered by provider group → `WatchStatus::order_rank()` → `added_at DESC`.

### Step 3: Full 5-Status Sync Across AniList & MAL (`src/services/tracker_merge.rs`)
- Update `merge_and_reconcile` (`src/services/tracker_merge.rs:467–562`):
  - All 5 resolved statuses (`"CURRENT"`, `"PLANNING"`, `"COMPLETED"`, `"PAUSED"`, `"DROPPED"`) are written into `watchlist (profile_id, anime_id, added_at, status)` unless explicitly dismissed in `dismissed_wl`.
  - For `"CURRENT"` shows, also upsert into `continue_watching` even when `resolved.progress == 0`, so 0-watched shows in `Watching` (`Fullmetal Alchemist: Brotherhood`, `NieR:Automata`) stay visible and resume cleanly at Episode 1 (`E1 next`).
  - Update eviction loop (`prev_replaced_entries`) so if a show is deleted remotely on a tracker, stale entries are removed only when no other tracker or Local holds them.

### Step 4: HomeData & Anti-Concatenation Card Line (`src/services/sync.rs`, `src/ui/home.rs`)
- In `src/services/sync.rs`:
  - Extend `load_watchlist_with_sources` to return `(Vec<Anime>, HashMap<i64, WatchlistSource>, HashMap<i64, WatchStatus>)` (and update callers in `src/main.rs`).
- In `src/ui/home.rs`:
  - Add `pub watchlist_statuses: HashMap<i64, WatchStatus>` to `HomeData`.
  - Pass `watchlist_statuses.get(&anime.id).copied()` into `render_card_with_source` → `build_card_meta_line`.
  - Inside `build_card_meta_line`, pass the available line width (`area.width`), compact long formats (`"MOVIE"` → `"MOV"`, `"SPECIAL"` → `"SPC"` when needed so status never wraps/truncates), and append `Span::styled(" · ", theme.divider)` + colorized `WatchStatus::short_card_tag()`.

### Step 5: Redesigned Bento Status & List Picker Popup (`src/state/mod.rs`, `src/services/scrobble.rs`, `src/ui/home.rs`, `src/main.rs`)
- Upgrade `WatchlistTargetPrompt` in `src/state/mod.rs` to include:
  - `current_status: Option<WatchStatus>`
  - `status_cursor: usize` (`0..=5`: Watching, Plan to Watch, Completed, On-Hold, Dropped, Remove)
  - `selected_target: WatchlistTarget` + helper `cycle_target(dir: isize)`
  - `available_targets()` / smart default target (`Both` if both AL+MAL have write tokens, `AniList` if only AL, `Mal` if only MAL, else `Local`)
- In `src/services/scrobble.rs`:
  - Add `set_anime_status_on_target(pool, anime, status: WatchStatus, target: WatchlistTarget, auth, trackers, now)` which saves the status locally and pushes `status.to_tracker_status()` live to AniList (`save_list_entry`, setting progress = `anime.episodes` if `Completed` and known) and/or MyAnimeList (`update_list_status`).
- In `src/ui/home.rs`:
  - Render the Bento modal (`watchlist_modal_pct` expanded slightly to `(68, 60)` on desktop so all 4 target pills + 6 labeled rows breathe cleanly) matching **Sketch A**.
- In `src/main.rs`:
  - Pressing `+` (on Home, Detail, Search, Tailored Wizard) opens the **Status & List Picker** popup immediately—even if the show is already in Watchlist—pre-selecting its current `WatchStatus` and appropriate `WatchlistTarget`.
  - Support `↑`/`↓`/`j`/`k`, quick keys `1`–`6`, `Tab`/`←`/`→`/`h`/`l` (for target DB), `Enter`, and `Esc`/`q`.

### Step 6: Unit & Integration Tests
- **`src/services/tracker_merge.rs`**: Verify `"CURRENT"`, `"PLANNING"`, `"COMPLETED"`, `"PAUSED"`, and `"DROPPED"` from both AniList and MAL all appear in `watchlist` with accurate resolved statuses and `0` duplicates.
- **`src/ui/home.rs`**: Verify `build_card_meta_line` renders `★ 8.8 · TV ·  Watch` (with zero concatenated words on 18–21 char cards) and `render_watchlist_target_modal` renders all 6 status options + 4 target pills without panic across desktop and compact viewports.

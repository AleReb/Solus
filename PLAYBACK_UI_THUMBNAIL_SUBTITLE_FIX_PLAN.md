# Solus Playback UI, Thumbnail, and Subtitle Reliability Plan

## 1. Reported Problems

The following issues must be fixed as one playback-state reliability project:

1. **Episode options modal misaligns after a few seconds.**
   The modal initially renders, then moves or leaves columns off to the side after asynchronous quality/subtitle/provider data arrives.

2. **Episode thumbnails repeatedly resize.**
   A thumbnail appears in its untrimmed size and is then repeatedly cropped or resized, causing visible flicker.

3. **Wrong episode thumbnails.**
   Some episodes can receive artwork from another episode, including Hunter x Hunter episode 13 receiving metadata or artwork associated with later episodes.

4. **No persistent “No subtitles” option for dubs.**
   Dub playback currently defaults to English subtitles, and turning them off may not persist into the next episode.

5. **Subtitle delay adjustment is unreliable.**
   Millisecond delay adjustments do not reliably affect the player or reset/reapply correctly when episodes change.

---

## 2. Engineering Goals

- Every asynchronous result is tied to the exact anime, episode, provider, dub/sub mode, and request generation that caused it.
- Stale responses can never update the active modal or thumbnail state.
- Modal geometry is reserved before data loads and never changes between loading and loaded states.
- Thumbnail image frames have fixed dimensions and never show an untrimmed intermediate replacement.
- Episode metadata is canonical, source-priority-aware, and key-validated by episode number.
- Subtitle selection is explicit: language or `No subtitles`.
- Subtitle selection and delay are persisted per profile and applied consistently to every episode/player.

---

## 3. Data Model Changes

### 3.1 Playback request generation

Add a process-local request generation counter to `AppState`:

```rust
pub playback_request_generation: u64
```

Increment whenever:

- An episode changes.
- Dub/sub mode changes.
- Provider changes.
- The playback options modal is reopened.
- Playback is cancelled or another anime/episode is selected.

Attach the generation and canonical keys to all stream-info messages:

```rust
StreamInfoDetected {
    request_generation: u64,
    anime_id: i64,
    episode: u32,
    is_dub: bool,
    provider_key: Option<String>,
    info: StreamInfo,
}
```

Ignore the response unless all current identity fields match.

### 3.2 Stable subtitle selection

Add an explicit subtitle selection enum:

```rust
pub enum SubtitleSelection {
    None,
    Track { code: String, name: String, url: Option<String> },
}
```

Do not represent `No subtitles` as an empty English track or as the first URL-less entry.

Add it to `SubtitleOption` or construct a synthetic first item:

```text
Code: "none"
Name: "No subtitles"
Stream label: "Off"
URL: empty
```

The synthetic item must be handled before auto-sync and URL lookup.

### 3.3 Persistent playback preferences

Persist the following per profile and optional per anime:

- `dub: bool`
- `subtitle_selection: None | track code`
- `subtitle_delay_ms: i32`
- `quality`
- `preferred_player`

Use profile-scoped SQLite or an existing settings table. The first release may persist globally per profile; later it can add per-anime overrides.

Do not store only the remote UI state. It must be a shared source of truth for the TUI, queue, remote, and player launch path.

### 3.4 Canonical episode thumbnail metadata

Add a thumbnail metadata structure:

```rust
pub struct EpisodeMediaMetadata {
    pub anime_id: i64,
    pub episode: u32,
    pub title: String,
    pub synopsis: Option<String>,
    pub thumbnail_url: Option<String>,
    pub thumbnail_source: ThumbnailSource,
}
```

Each thumbnail download message must include:

- `anime_id`
- `episode`
- `source_url`
- `metadata_generation`

This prevents an old download for episode 13 from replacing episode 13 after the anime changed.

---

## 4. Fix Episode Options Misalignment

### 4.1 Reserve modal geometry

In `src/ui/play_options.rs`:

- Do not clear or replace the entire list geometry during loading.
- Keep four fixed columns for the lifetime of the modal.
- Keep a fixed header height and fixed footer height.
- Define loading rows inside each existing column instead of removing a border/block or changing constraints.
- Clamp every title and item label to the final column width.
- Render only after `frame.area()` has been split once.

### 4.2 Replace mouse coordinate assumptions

The current mouse path is based on fixed modal coordinates and a few hardcoded X ranges.

Replace it with rectangles returned by the layout render function:

```rust
PlaybackOptionsLayout {
    player_rect,
    provider_rect,
    quality_rect,
    subtitle_rect,
    footer_rect,
}
```

Mouse handling must use `Rect::contains` rather than `rel_x < 28`, `rel_x < 55`, and hardcoded header offsets.

### 4.3 Transactional stream-info updates

When `StreamInfoDetected` arrives:

1. Verify anime, episode, generation, and dub mode.
2. Build the complete replacement option state locally.
3. Preserve the user's existing quality/subtitle/provider cursors only if an exact match exists.
4. Commit qualities, subtitles, providers, master URL, and loading state in one block.
5. Never clear a list and then add entries in separate phases.

If a list is loading, keep placeholder rows or fixed-height empty rows instead of shrinking the layout.

### 4.4 Regression tests

Add snapshot/layout tests for:

- Loading state at 84x28, 120x36, and 90x28.
- Loaded state with 0/1/many providers.
- 0/1/many subtitle tracks.
- Dub-only provider.
- Stale stream response from a previous episode.
- Stale stream response from sub mode arriving after dub mode selected.

The tests must assert:

- Border rectangles never move.
- Body columns remain identical widths.
- Footer remains at the same location.
- Stale responses do not mutate state.

---

## 5. Fix Thumbnail Flicker

### 5.1 Reserve a stable frame

Episode thumbnails must use a fixed protocol/image size for the lifetime of a detail/episode card.

- Prepare images once to the exact target cell dimensions.
- Reuse the same stateful protocol object while downloading.
- Do not remove an existing protocol and insert a fallback protocol repeatedly.
- Do not recreate stateful protocols on ordinary render frames.

Fallback protocol, real thumbnail protocol, and empty protocol should each be stable instances keyed by `(anime_id, episode)`.

### 5.2 Atomic thumbnail replacement

Add a thumbnail state machine:

```rust
enum EpisodeThumbnailState {
    Empty,
    Fallback,
    Loading,
    Ready,
}
```

Rendering rules:

- `Empty` -> reserved placeholder, no content-dependent resize.
- `Loading` -> fallback/placeholder, not an alternate-sized thumbnail.
- `Ready` -> replace once with the prepared image and never resize again unless terminal font size or terminal protocol geometry changes.

The real thumbnail must replace the fallback only once, not every render/download event.

### 5.3 Terminal resize handling

Prepare/insert thumbnails again only when the terminal font size or protocol geometry changes.

Track:

```rust
pub thumbnail_geometry_key: Option<(u16, u16, u16, u16)>
```

On geometry change:

1. Rebuild protocols for loaded images.
2. Preserve URLs and metadata.
3. Do not restart downloads for unchanged URLs.

### 5.4 Remove repeated fallback clearing

Remove unconditional fallback protocol clearing from banner or cover update paths.

Clear episode fallback state only when:

- Anime changes.
- Episode metadata generation changes.
- Font/protocol geometry changes.

### 5.5 Thumbnail regression tests

Test:

- Thumbnail remains the same Rect across loading/ready transitions.
- The same image URL produces no duplicate download.
- A stale episode download cannot replace another episode.
- Terminal resize rebuilds protocols once, not once per frame.
- No untrimmed image is inserted before final 16:9 preparation.

---

## 6. Fix Wrong Episode Thumbnails

### 6.1 Canonical episode mapping

Episode metadata fetch must produce one canonical entry per anime episode number.

Source priority:

1. AniZip exact AniList-ID episode mapping.
2. AniList streaming episode entries only if title parsing produces the same canonical episode number.
3. Kitsu fallback only for missing title/synopsis/thumbnail fields.

Never let an AniList entry at array index 13 automatically overwrite episode 13 if its title says Episode 73 or a special.

The title parser must support:

- `Episode 13 - Title`
- `Ep 13: Title`
- `E13 Title`
- Absolute/relative date/OVA/special formats
- Roman numeral or special entry formats

The parser must return an ambiguity flag when it cannot confidently map an episode.

### 6.2 Confidence-based merges

For each episode number:

- AniZip exact ep number gets high priority.
- AniList title parser with an explicit episode number gets medium/high priority.
- Array-index fallback gets low priority and may fill missing data only.
- Kitsu mapping gets low priority unless the episode is missing.

Do not replace a high-confidence thumbnail with a later low-confidence URL.

### 6.3 Cache validation

Before using `{}_{}.jpg`, validate that the cache entry is for:

- The exact anime ID.
- The exact episode number.
- The exact source URL.

Cache filenames should include a short hash of the URL:

```text
{anime_id}_{episode}_{source_hash}.jpg
```

A different URL for the same episode must generate a new cache entry.

### 6.4 Metadata source telemetry

In debug/error state, record:

- Source used.
- Parsed episode number.
- Confidence.
- Source URL.

This makes future incorrect episodes diagnosable.

### 6.5 Hunter x Hunter regression coverage

Add fixture tests representing a long-running series:

- Episode 13 thumbnail remains episode 13.
- A source array incorrectly places a later episode in the 13th position.
- Title parsing identifies the later episode and rejects array-index fallback.
- AniZip exact map wins over AniList/Kitsu.

Use a saved fixture with malformed or mismatched titles to ensure no network dependency.

---

## 7. Add Explicit No-Subtitle Option

### 7.1 Subtitle list

Add “No subtitles” as the first non-user-track item in every subtitle list.

For dub playback, recommend:

- “No subtitles” selected by default.
- Optional English or other subtitle tracks explicitly selectable.

For sub playback, default to the user profile’s preferred subtitle language rather than silently defaulting English.

### 7.2 Launch behavior

Introduce an explicit boolean in launch intent:

```rust
pub subtitle_enabled: bool
pub subtitle_track: Option<SubtitleOption>
```

Rules:

- `None` -> set `sub-url`/external subtitle path to none and disable any existing subtitle in mpv/iina/VLC.
- Track -> load that URL or track and enable it.
- Provider/native HLS subtitle behavior must not re-enable subtitles after launch.

### 7.3 Player commands

At launch and episode change:

- mpv: set `sid=no` for no subtitles, or set the external/file track.
- iina: pass the matching subtitle behavior through its mpv options.
- VLC: add only the supported subtitle disable/enable arguments for its launch path.

The implementation must not assume every player accepts mpv-only flags.

### 7.4 Persist selection

Store no-subtitle state in the shared playback preference object.

At next episode:

- The no-subtitle choice must remain active.
- Reopening playback options must display “No subtitles” selected.
- Remote state must receive `sub_active=false` and an explicit label.

### 7.6 No-subtitle tests

Test:

- Dub + no subtitles produces `sub_url=None` and player sid disabled.
- Dub + English subtitles produces English URL and enabled subtitle state.
- Next episode in the same profile retains no-subtitles.
- Changing back to sub mode does not accidentally leave subtitles disabled.
- Queue-prefetched episodes preserve the user’s selection.

---

## 8. Fix Subtitle Delay

### 8.1 Shared delay state

Use one delay value across:

- TUI playback options
- mpv IPC monitor
- mobile remote
- queue/active playback state
- next-episode launch

Represent delay in milliseconds internally:

```rust
pub subtitle_delay_ms: i32
```

Convert to seconds only at the player boundary.

### 8.2 Apply immediately

When the user changes delay:

- mpv: set `sub-delay=<seconds>`.
- iina: forward the same property to the embedded mpv instance.
- VLC: apply supported equivalent delay if available; otherwise show an accurate unsupported notice rather than claiming success.

Remote actions must verify the command succeeded before updating UI state.

### 8.3 Apply across episodes

Before launching each episode:

1. Apply the selected subtitle track.
2. Apply the persisted delay milliseconds.
3. Apply sync timing only after the external subtitle is downloaded and ready.

For auto-synced subtitle files:

- Calculate sync once.
- Preserve the user delay as an offset after sync.
- Do not reset the offset when changing episodes unless no preference exists.

### 8.4 Delay reset behavior

A reset action must:

- Set the delay to zero milliseconds.
- Send the player command immediately.
- Persist zero so future episodes start at zero.

### 8.5 Delay tests

Test:

- `-250 ms` becomes `-0.25 seconds` for mpv.
- `+250 ms` becomes `0.25 seconds`.
- Reset sends zero and persists zero.
- Delay survives episode launch and queue prefetch.
- Failed IPC delivery does not update the shared UI value.
- Switching no-subtitle mode preserves delay for later re-enable.

---

## 9. Playback Launch Transaction

Create one launch intent assembled in one place:

```rust
pub struct PlaybackIntent {
    pub anime_id: i64,
    pub episode: u32,
    pub dub: bool,
    pub quality: String,
    pub provider_key: String,
    pub player: String,
    pub subtitles: SubtitleSelection,
    pub subtitle_delay_ms: i32,
}
```

Before launch:

1. Verify provider supports dub/sub mode.
2. Verify quality exists.
3. Verify subtitle track exists or explicitly `None`.
4. Resolve stream URL.
5. Download/sync subtitle if needed.
6. Build one `PlayOptions` value.
7. Launch player.
8. Broadcast the exact remote state only after success.

This eliminates partially synchronized launch state.

---

## 10. Implementation Phases

### Phase A — State and request identity

- Add playback request generations.
- Add transactional stream-info handling.
- Add stable subtitle selection and delay models.
- Add source-aware thumbnail messages.

### Phase B — Modal layout

- Replace fixed mouse ranges with layout rectangles.
- Reserve loading/loaded geometry.
- Prevent stale stream responses.
- Add modal snapshot tests.

### Phase C — Thumbnail pipeline

- Add stable protocol/state keys.
- Add URL/hash cache validation.
- Remove repeated fallback clearing.
- Fix episode metadata merge and confidence.
- Add thumbnail/mapping fixtures.

### Phase D — Subtitle behavior

- Add No subtitles.
- Persist subtitle selection and delay.
- Apply delay and track selection at launch.
- Update remote state and queue launch behavior.
- Add player-specific tests.

### Phase E — End-to-end verification

- Unit tests for parsing and state models.
- UI snapshot tests for sizes and loading states.
- Async stale-response tests.
- Cache/mapping fixture tests.
- Manual multi-episode mpv test.
- Manual remote test.

---

## 11. Acceptance Criteria

The implementation is complete only when:

- The playback options modal geometry is identical before and after stream detection.
- Stale stream responses cannot change an active anime or episode.
- Episode thumbnails remain fixed-size without flicker or repeated crop changes.
- Episode 13 cannot receive metadata/thumbnails for episode 73 or another later episode.
- “No subtitles” is visible and selectable for dub playback.
- No subtitles remains off for subsequent episodes until explicitly changed.
- Subtitle delay changes immediately affect active playback.
- Subtitle delay persists across episodes and queue launches.
- Subtitle reset persists as zero.
- The remote and TUI show the same subtitle and delay state.
- 294 or more existing tests continue passing.

---

## 12. Manual Verification Checklist

Test at least:

- 90x28, 120x36, and larger terminal sizes.
- Hunter x Hunter episode 13 and a late-series episode.
- Slow/failed stream metadata response followed by a provider change.
- Switch sub -> dub -> sub while the options modal is loading.
- Dub stream with no subtitles enabled.
- Dub stream with English subtitles enabled.
- Subtitle delay +250 ms and -250 ms.
- Reset delay to zero.
- Advance to next episode automatically and manually.
- Stop and relaunch Solus.
- Reload an episode with cached thumbnails.
- Toggle terminal font-size/protocol if supported.

A real-device pass should cover both keyboard and remote controls before release.
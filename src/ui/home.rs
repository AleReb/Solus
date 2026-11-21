//! Home screen — Cinematic Bento & Dynamic Shelf layout.
//! Renders: Top Glass Navigation Bar + 3-Zone Hero Stage + Smart Bento Shelf Grid + Footer.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;

use crate::{
    config::WatchlistTarget,
    db::{cache::Anime, user::WatchlistSource},
    services::tailored::clean_synopsis_single_line,
    state::{AppState, CategoryRow, Screen, WatchlistPromptMode},
    ui::components::{
        cover::HalfblockCover,
        dock::{build_dock_row_plan, DockChip, DockRowPlan},
    },
};

/// Width of each anime card in the row (chars)
pub const CARD_WIDTH: u16 = 18;
/// Height of the rounded thumbnail frame (rows, enclosing 16x11 portrait art)
pub const CARD_FRAME_HEIGHT: u16 = 13;
/// Total height of each anime card (13-row portrait frame + 2 caption rows below)
pub const CARD_HEIGHT: u16 = 15;
/// Gap between cards
pub const CARD_GAP: u16 = 2;
/// Height of the top glass navigation bar
pub const TOP_BAR_HEIGHT: u16 = 1;
/// Height of the widescreen hero stage
pub const HERO_HEIGHT: u16 = 10;
/// Default height of each shelf tier
#[allow(dead_code)]
pub const ROW_HEIGHT: u16 = 17;
/// Height of the 2-row bottom command dock (1 top divider + 2 balanced rows)
pub const FOOTER_HEIGHT: u16 = 3;

/// Interactive actions exposed by clicking pills in the Home Bottom Command Dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeFooterAction {
    Search,
    Queue,
    Calendar,
    Wizard,
    Analytics,
    Remote,
    Profile,
    Theme,
    Login,
    StopPlayback,
    UpdateNow,
    OpenDetail,
    Resume,
    Watchlist,
    Remove,
    Refresh,
    Settings,
    Help,
    Quit,
}

/// A single vertical tier on the Home screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfTier {
    /// Side-by-side Bento dock combining Continue Watching (1-2 items) & My Watchlist.
    BentoSplit,
    /// Standard full-width horizontal shelf.
    Single(CategoryRow),
}

/// Returns true when Continue Watching (1-2 items) and Watchlist can dock side-by-side.
pub fn is_bento_split(data: &HomeData, width: u16) -> bool {
    !data.continue_watching.is_empty()
        && data.continue_watching.len() <= 2
        && !data.watchlist.is_empty()
        && width >= 96
}

/// Width of the left Bento container for 1-2 Continue Watching cards.
pub fn bento_left_width(cw_count: usize) -> u16 {
    let count = cw_count.clamp(1, 2) as u16;
    count * CARD_WIDTH + count.saturating_sub(1) * CARD_GAP + 4
}

/// Compute centered modal bounds matching overlays so underlying Home artwork doesn't bleed through.
fn active_modal_rect(frame_area: Rect, state: &AppState) -> Option<Rect> {
    if state.exit_confirm_open {
        return Some(exit_confirm_modal_rect(frame_area));
    }
    let (pct_x, pct_y) = if state.watchlist_target_prompt.is_some() {
        watchlist_modal_pct(frame_area)
    } else {
        match state.screen {
            Screen::UpdateModal => (64, 45),
            Screen::RemoteModal => (64, 76),
            Screen::CastModal => (68, 55),
            Screen::ProfileModal => crate::ui::profile::modal_pct(frame_area),
            Screen::QueueModal => crate::ui::queue::modal_pct(frame_area),
            Screen::Help => crate::ui::help::modal_pct(frame_area),
            Screen::Settings => crate::ui::settings::modal_pct(frame_area),
            Screen::Setup => crate::ui::setup::modal_pct(frame_area),
            Screen::AniListLogin => crate::ui::login::modal_pct(frame_area),
            Screen::Calendar => {
                if frame_area.width >= 115 && frame_area.height >= 34 {
                    (94, 90)
                } else {
                    (98, 94)
                }
            }
            _ => return None,
        }
    };
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(frame_area);
    Some(
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - pct_x) / 2),
                Constraint::Percentage(pct_x),
                Constraint::Percentage((100 - pct_x) / 2),
            ])
            .split(vert[1])[1],
    )
}

/// Build the ordered list of visible vertical tiers for the current terminal width.
pub fn build_shelf_tiers(data: &HomeData, width: u16) -> Vec<ShelfTier> {
    let mut tiers = Vec::new();
    let split = is_bento_split(data, width);

    if split {
        tiers.push(ShelfTier::BentoSplit);
    } else {
        if !data.continue_watching.is_empty() {
            tiers.push(ShelfTier::Single(CategoryRow::ContinueWatching));
        }
        if !data.watchlist.is_empty() {
            tiers.push(ShelfTier::Single(CategoryRow::Watchlist));
        }
    }

    if !data.trending.is_empty() {
        tiers.push(ShelfTier::Single(CategoryRow::Trending));
    }
    if !data.popular.is_empty() {
        tiers.push(ShelfTier::Single(CategoryRow::Popular));
    }
    if !data.top_rated.is_empty() {
        tiers.push(ShelfTier::Single(CategoryRow::TopRated));
    }
    if !data.seasonal.is_empty() {
        tiers.push(ShelfTier::Single(CategoryRow::Seasonal));
    }

    tiers
}

/// Compute `(visible_cards, card_width)` for a horizontal strip of `usable_w` columns
/// so cards expand/pack cleanly edge-to-edge without leaving ~15-20 col voids on the right.
pub fn compute_row_card_metrics(usable_w: u16) -> (usize, u16) {
    let min_card_w: u16 = CARD_WIDTH;
    let max_card_w: u16 = 21;
    let gap = CARD_GAP;
    let vis_n = ((usable_w + gap) / (min_card_w + gap)).max(1) as usize;
    let gaps_total = (vis_n.saturating_sub(1)) as u16 * gap;
    let card_w = (usable_w.saturating_sub(gaps_total) / vis_n as u16).clamp(min_card_w, max_card_w);
    (vis_n, card_w)
}

/// Compute `(visible_cards, card_width)` for a specific shelf row.
/// `ContinueWatching` always uses fixed `CARD_WIDTH` (`22`) so all thumbnails match standard Trending card proportions.
pub fn row_card_metrics_for(row: &CategoryRow, usable_w: u16) -> (usize, u16) {
    if *row == CategoryRow::ContinueWatching {
        let vis_n = ((usable_w + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).max(1) as usize;
        (vis_n, CARD_WIDTH.min(usable_w))
    } else {
        compute_row_card_metrics(usable_w)
    }
}

/// Compute `(start_idx, row_heights)` so visible tiers pack cleanly into `avail_h`
/// without dropping an extra shelf to small 1–3 row boundary differences.
pub fn compute_shelf_layout(
    tiers: &[ShelfTier],
    avail_h: u16,
    active_tier_idx: usize,
) -> (usize, Vec<u16>) {
    if tiers.is_empty() || avail_h == 0 {
        return (0, Vec::new());
    }

    let min_tier_h: u16 = CARD_HEIGHT + 1; // 1 header + 15-row portrait card
    let max_fit = (avail_h / min_tier_h).max(1) as usize;
    let visible_rows = tiers.len().min(max_fit);
    let start_idx = active_tier_idx.saturating_sub(visible_rows.saturating_sub(1));

    let visible_slice: Vec<&ShelfTier> = tiers.iter().skip(start_idx).take(visible_rows).collect();
    if visible_slice.len() == 1 {
        let cap = match visible_slice[0] {
            ShelfTier::BentoSplit => CARD_HEIGHT + 2,
            ShelfTier::Single(_) => CARD_HEIGHT + 2,
        };
        return (start_idx, vec![avail_h.min(cap)]);
    }

    let mut heights = vec![min_tier_h; visible_slice.len()];
    let mut rem = avail_h.saturating_sub(min_tier_h * visible_slice.len() as u16);

    // 1. Ensure BentoSplit gets +1 row (17 total: 2 borders + 15-row card) first
    for (idx, tier) in visible_slice.iter().enumerate() {
        if matches!(tier, ShelfTier::BentoSplit) {
            let add = rem.min((CARD_HEIGHT + 2).saturating_sub(heights[idx]));
            heights[idx] += add;
            rem -= add;
        }
    }

    // 2. Distribute any remaining breathing rows (up to CARD_HEIGHT + 2 per tier) evenly
    for h in &mut heights {
        if rem == 0 {
            break;
        }
        if *h < CARD_HEIGHT + 2 {
            *h += 1;
            rem -= 1;
        }
    }

    (start_idx, heights)
}

/// Compute the starting index for a row viewport so scrolling to the end never leaves blank slots.
pub fn row_view_start(offset: usize, total: usize, visible_n: usize) -> usize {
    if visible_n == 0 {
        return 0;
    }
    offset.min(total.saturating_sub(visible_n))
}

/// Canonical metadata for a shelf row.
fn row_meta(row: &CategoryRow) -> (&'static str, &'static str, &'static str, &'static str) {
    match row {
        CategoryRow::ContinueWatching => ("continue_watching", " Resume", " Continue Watching", "RESUME"),
        CategoryRow::Watchlist => ("watchlist", "󰋜 Watchlist", "󰋜 My Watchlist", "WATCHLIST"),
        CategoryRow::Trending => ("trending", " Trending", " Trending Now", "TRENDING"),
        CategoryRow::Popular => ("popular", " Popular", " Popular Picks", "POPULAR"),
        CategoryRow::TopRated => ("top_rated", " Top Rated", " Top Rated", "TOP RATED"),
        CategoryRow::Seasonal => ("seasonal", "󰃭 Seasonal", "󰃭 Seasonal", "SEASONAL"),
    }
}

fn row_items<'a>(row: &CategoryRow, data: &'a HomeData) -> &'a [Anime] {
    match row {
        CategoryRow::ContinueWatching => &data.continue_watching,
        CategoryRow::Watchlist => &data.watchlist,
        CategoryRow::Trending => &data.trending,
        CategoryRow::Popular => &data.popular,
        CategoryRow::TopRated => &data.top_rated,
        CategoryRow::Seasonal => &data.seasonal,
    }
}

/// Non-empty category rows in canonical display order with their 1-based shortcut number.
pub fn top_bar_shelves(data: &HomeData) -> Vec<(usize, CategoryRow, &'static str)> {
    let all = [
        (1, CategoryRow::ContinueWatching, " Resume", !data.continue_watching.is_empty()),
        (2, CategoryRow::Watchlist, "󰋜 Watchlist", !data.watchlist.is_empty()),
        (3, CategoryRow::Trending, " Trending", !data.trending.is_empty()),
        (4, CategoryRow::Popular, " Popular", !data.popular.is_empty()),
        (5, CategoryRow::TopRated, " Top Rated", !data.top_rated.is_empty()),
        (6, CategoryRow::Seasonal, "󰃭 Seasonal", !data.seasonal.is_empty()),
    ];
    all.into_iter()
        .filter(|(_, _, _, non_empty)| *non_empty)
        .map(|(num, row, label, _)| (num, row, label))
        .collect()
}

/// Hit-test a click on the top navigation bar (row 0) to jump directly to a shelf.
pub fn top_bar_shelf_at(data: &HomeData, width: u16, mx: u16) -> Option<CategoryRow> {
    let mut x: u16 = 11; // "  SOLUS " (9) + "  " (2)
    for (num, row, label) in top_bar_shelves(data) {
        let pill_text = format!(" {}:{} ", num, label);
        let pill_w = pill_text.chars().count() as u16;
        if x + pill_w + 18 >= width {
            break;
        }
        if mx >= x && mx < x + pill_w {
            return Some(row);
        }
        x += pill_w + 1;
    }
    None
}

/// Render the full home screen.
pub fn render(frame: &mut Frame, state: &mut AppState, categories: &HomeData) {
    let area = frame.area();
    let banner_anime = active_banner_anime(state, categories).or(categories.featured.as_ref());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(TOP_BAR_HEIGHT), // Glass navigation & shelf bar
            Constraint::Length(HERO_HEIGHT),    // 3-Zone Widescreen Hero Stage
            Constraint::Min(0),                 // Smart Bento Shelf Grid
            Constraint::Length(FOOTER_HEIGHT),  // 2-Row Balanced Command Dock
        ])
        .split(area);

    render_top_bar(frame, chunks[0], state, categories);
    render_featured(frame, chunks[1], state, banner_anime, categories);
    render_rows(frame, chunks[2], state, categories);
    render_footer(frame, chunks[3], state);
}

/// Top 1-line Glass Navigation Bar with brand badge, interactive shelf pills, and telemetry.
fn render_top_bar(frame: &mut Frame, area: Rect, state: &AppState, data: &HomeData) {
    let theme = state.theme();
    let mut spans = vec![
        Span::styled(
            "  SOLUS ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ];

    let mut used_width: u16 = 11;

    // Reserve space for right-side status badges
    let user_badge = match (state.auth.has_anilist(), state.auth.has_mal()) {
        (true, true) => {
            let al = state.auth.anilist.username.as_deref().unwrap_or("AL");
            let mal = state.auth.mal.username.as_deref().unwrap_or("MAL");
            format!("󰄬 AL:{al} · MAL:{mal}")
        }
        (true, false) => format!(
            "󰄬 AL:{}",
            state.auth.anilist.username.as_deref().unwrap_or("AniList")
        ),
        (false, true) => format!(
            "󰄬 MAL:{}",
            state.auth.mal.username.as_deref().unwrap_or("MAL")
        ),
        (false, false) => "󰍂 Guest".to_string(),
    };
    let queue_badge = if state.queue.is_empty() {
        String::new()
    } else {
        format!("◆ Queue: {}  ·  ", state.queue.len())
    };
    let net_badge = if state.network_mode.is_congested() {
        format!("[ {} ]  ·  ", state.network_mode.short_tag())
    } else {
        String::new()
    };
    let profile_badge = format!(
        "{} {} [P]",
        state.active_profile_icon, state.active_profile_name
    );
    let right_text = format!(
        "{}{}{}{}  ·  {}  ·  ◈ {} ",
        queue_badge,
        net_badge,
        profile_badge,
        "",
        user_badge,
        theme.name
    );
    let right_width = right_text.chars().count() as u16 + 2;

    for (num, row, label) in top_bar_shelves(data) {
        let is_active = state.active_row == row;
        let pill_text = format!(" {}:{} ", num, label);
        let pill_w = pill_text.chars().count() as u16;
        if used_width + pill_w + right_width >= area.width {
            break;
        }
        let style = if is_active {
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.text_dim)
                .bg(theme.border_idle)
        };
        spans.push(Span::styled(pill_text, style));
        spans.push(Span::raw(" "));
        used_width += pill_w + 1;
    }

    let remaining = area.width.saturating_sub(used_width + right_width) as usize;
    if remaining > 0 {
        spans.push(Span::raw(" ".repeat(remaining)));
    }

    if area.width >= used_width + right_width {
        let auth_color = if state.auth.is_authenticated() {
            Color::Rgb(80, 250, 123)
        } else {
            theme.text_dim
        };
        if !queue_badge.is_empty() {
            spans.push(Span::styled(
                format!("◆ Queue: {}", state.queue.len()),
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("  ·  ", Style::default().fg(theme.divider)));
        }
        if state.network_mode.is_congested() {
            let net_fg = match state.network_mode {
                crate::config::NetworkMode::CongestedWifi => Color::Rgb(245, 185, 66),
                crate::config::NetworkMode::UltraSaver => Color::Rgb(80, 220, 145),
                crate::config::NetworkMode::Normal => theme.text_dim,
            };
            spans.push(Span::styled(
                format!("[ {} ]", state.network_mode.short_tag()),
                Style::default().fg(net_fg).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("  ·  ", Style::default().fg(theme.divider)));
        }
        spans.push(Span::styled(
            profile_badge,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("  ·  ", Style::default().fg(theme.divider)));
        spans.push(Span::styled(
            user_badge,
            Style::default().fg(auth_color).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("  ·  ", Style::default().fg(theme.divider)));
        spans.push(Span::styled(
            format!("◈ {} ", theme.name),
            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn build_footer_dock_rows(state: &AppState, width: usize) -> (DockRowPlan<HomeFooterAction>, DockRowPlan<HomeFooterAction>) {
    let theme = state.theme();
    let emerald = Color::Rgb(80, 250, 123);

    // ── Row 1: Discovery & System Features + Account / Theme / Live Status ─────
    let mut explore_left = vec![
        DockChip {
            icon: "",
            key: "[/]",
            label: "Search".to_string(),
            key_color: theme.primary,
            label_color: theme.text,
            priority: 0,
            bg: None,
            action: Some(HomeFooterAction::Search),
        },
        DockChip {
            icon: "◆",
            key: "[Q]",
            label: if state.queue.is_empty() {
                "Queue".to_string()
            } else {
                format!("Queue ({})", state.queue.len())
            },
            key_color: theme.accent,
            label_color: theme.text,
            priority: 2,
            bg: None,
            action: Some(HomeFooterAction::Queue),
        },
        DockChip {
            icon: "󰃭",
            key: "[c]",
            label: "Calendar".to_string(),
            key_color: emerald,
            label_color: theme.text,
            priority: 1,
            bg: None,
            action: Some(HomeFooterAction::Calendar),
        },
        DockChip {
            icon: "󰋽",
            key: "[w]",
            label: "Wizard".to_string(),
            key_color: theme.accent,
            label_color: theme.text,
            priority: 2,
            bg: None,
            action: Some(HomeFooterAction::Wizard),
        },
        DockChip {
            icon: "",
            key: "[a]",
            label: if width >= 156 { "Analytics" } else { "Stats" }.to_string(),
            key_color: theme.secondary,
            label_color: theme.text,
            priority: 4,
            bg: None,
            action: Some(HomeFooterAction::Analytics),
        },
        DockChip {
            icon: "◎",
            key: "[m]",
            label: "Remote".to_string(),
            key_color: theme.secondary,
            label_color: theme.text,
            priority: 5,
            bg: None,
            action: Some(HomeFooterAction::Remote),
        },
    ];

    // If room permits cleanly, include Update chip inside Row 1 status or Remote right cluster
    let mut explore_right: Vec<DockChip<HomeFooterAction>> = Vec::new();
    if let Some(ref track) = state.now_playing {
        let short_track: String = track.chars().take(18).collect();
        explore_right.push(DockChip {
            icon: "󰐊",
            key: "[X]",
            label: format!("Stop ({})", short_track),
            key_color: Color::Black,
            label_color: Color::Black,
            priority: 1,
            bg: Some(theme.accent),
            action: Some(HomeFooterAction::StopPlayback),
        });
    } else if let Some((_, ep, pct, ref _spd)) = state.active_download {
        explore_right.push(DockChip {
            icon: "󰇚",
            key: "",
            label: format!("Ep {}: {:.0}%", ep, pct),
            key_color: Color::Rgb(139, 233, 253),
            label_color: Color::Rgb(139, 233, 253),
            priority: 1,
            bg: None,
            action: None,
        });
    } else if let Some((pct, ref _spd)) = state.update_progress {
        explore_right.push(DockChip {
            icon: "󰚰",
            key: "",
            label: format!("Updating {:.0}%", pct),
            key_color: emerald,
            label_color: emerald,
            priority: 1,
            bg: None,
            action: None,
        });
    } else if let Some(ref update) = state.available_update {
        explore_right.push(DockChip {
            icon: "󰚰",
            key: "[U]",
            label: format!("Update ({})", update.latest_version),
            key_color: Color::Black,
            label_color: Color::Black,
            priority: 1,
            bg: Some(emerald),
            action: Some(HomeFooterAction::UpdateNow),
        });
    }

    let profile_chip_label: String = state.active_profile_name.chars().take(10).collect();
    explore_right.push(DockChip {
        icon: "◈",
        key: "[P]",
        label: if profile_chip_label.is_empty() {
            "Profile".to_string()
        } else {
            profile_chip_label
        },
        key_color: theme.accent,
        label_color: theme.text,
        priority: 4,
        bg: None,
        action: Some(HomeFooterAction::Profile),
    });
    explore_right.push(DockChip {
        icon: "◈",
        key: "[T]",
        label: theme.name.to_string(),
        key_color: theme.secondary,
        label_color: theme.text,
        priority: 3,
        bg: None,
        action: Some(HomeFooterAction::Theme),
    });
    explore_right.push(DockChip {
        icon: "󰄬",
        key: "[L]",
        label: match (state.auth.has_anilist(), state.auth.has_mal()) {
            (true, true) => "AL+MAL".to_string(),
            (true, false) => state
                .auth
                .anilist
                .username
                .as_deref()
                .unwrap_or("AniList")
                .to_string(),
            (false, true) => format!(
                "MAL:{}",
                state.auth.mal.username.as_deref().unwrap_or("MAL")
            ),
            (false, false) => "Login".to_string(),
        },
        key_color: if state.auth.is_authenticated() { emerald } else { theme.primary },
        label_color: if state.auth.is_authenticated() { emerald } else { theme.text },
        priority: 2,
        bg: None,
        action: Some(HomeFooterAction::Login),
    });

    // If nothing dynamic is playing/downloading/updating and width is narrow, shrink Priority 6 quietly
    let _ = &mut explore_left;

    let row1 = build_dock_row_plan(
        " EXPLORE ",
        theme.primary,
        theme.border_idle,
        explore_left,
        explore_right,
        width,
    );

    // ── Row 2: Card & Navigation Controls + Settings / Help / Quit ─────────────
    let mut actions_left = vec![
        DockChip {
            icon: "▸",
            key: "[hjkl]",
            label: "Move".to_string(),
            key_color: theme.secondary,
            label_color: theme.text_dim,
            priority: 3,
            bg: None,
            action: None,
        },
        DockChip {
            icon: "",
            key: "[1-6]",
            label: "Shelf".to_string(),
            key_color: theme.secondary,
            label_color: theme.text_dim,
            priority: 6,
            bg: None,
            action: None,
        },
        DockChip {
            icon: "󰘳",
            key: "[Enter]",
            label: "Watch".to_string(),
            key_color: Color::White,
            label_color: theme.text,
            priority: 0,
            bg: None,
            action: Some(HomeFooterAction::OpenDetail),
        },
        DockChip {
            icon: "",
            key: "[r]",
            label: "Resume".to_string(),
            key_color: theme.primary,
            label_color: theme.text,
            priority: 1,
            bg: None,
            action: Some(HomeFooterAction::Resume),
        },
        DockChip {
            icon: "󰋜",
            key: "[+]",
            label: "Watchlist".to_string(),
            key_color: theme.accent,
            label_color: theme.text,
            priority: 2,
            bg: None,
            action: Some(HomeFooterAction::Watchlist),
        },
    ];

    if state.active_row == CategoryRow::ContinueWatching || state.active_row == CategoryRow::Watchlist {
        actions_left.push(DockChip {
            icon: "󰅖",
            key: "[x]",
            label: "Remove".to_string(),
            key_color: Color::Rgb(255, 85, 85),
            label_color: Color::Rgb(255, 130, 130),
            priority: 4,
            bg: None,
            action: Some(HomeFooterAction::Remove),
        });
    }

    actions_left.push(DockChip {
        icon: "↺",
        key: "[R]",
        label: "Refresh".to_string(),
        key_color: theme.text_dim,
        label_color: theme.text_dim,
        priority: 5,
        bg: None,
        action: Some(HomeFooterAction::Refresh),
    });

    let actions_right = vec![
        DockChip {
            icon: "󰒓",
            key: "[s]",
            label: "Settings".to_string(),
            key_color: theme.primary,
            label_color: theme.text,
            priority: 2,
            bg: None,
            action: Some(HomeFooterAction::Settings),
        },
        DockChip {
            icon: "",
            key: "[?]",
            label: "Help".to_string(),
            key_color: theme.accent,
            label_color: theme.text,
            priority: 0,
            bg: None,
            action: Some(HomeFooterAction::Help),
        },
        DockChip {
            icon: "󰈆",
            key: "[q]",
            label: "Quit".to_string(),
            key_color: theme.text_dim,
            label_color: theme.text_dim,
            priority: 1,
            bg: None,
            action: Some(HomeFooterAction::Quit),
        },
    ];

    let row2 = build_dock_row_plan(
        " ACTIONS ",
        theme.secondary,
        theme.border_idle,
        actions_left,
        actions_right,
        width,
    );

    (row1, row2)
}

/// Hit-test a left-click inside the 2-row Home Command Dock (`FOOTER_HEIGHT = 3`).
pub fn footer_action_at(state: &AppState, width: u16, height: u16, mx: u16, my: u16) -> Option<HomeFooterAction> {
    let dock_top = height.saturating_sub(FOOTER_HEIGHT);
    if my <= dock_top || my >= height {
        return None;
    }
    let (row1, row2) = build_footer_dock_rows(state, width as usize);
    let target = if my == dock_top + 1 { &row1 } else { &row2 };
    target
        .hitboxes
        .iter()
        .find(|(x0, x1, _)| mx >= *x0 && mx < *x1)
        .map(|(_, _, act)| *act)
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) {
    let theme = state.theme();
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme.divider));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (row1, row2) = build_footer_dock_rows(state, inner.width as usize);
    frame.render_widget(Paragraph::new(vec![row1.line, row2.line]), inner);
}

/// 3-Zone Widescreen Hero Stage at the top: Cover Frame + Editorial Info + Telemetry HUD.
fn render_featured(frame: &mut Frame, area: Rect, state: &mut AppState, anime: Option<&Anime>, data: &HomeData) {
    let theme = state.theme();

    let banner = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .title(Span::styled(
            "  FEATURED SPOTLIGHT ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = banner.inner(area);
    frame.render_widget(banner, area);

    let Some(anime) = anime else {
        let msg = vec![
            Line::from(Span::styled(
                " 󰑮 Syncing Anime Library...",
                Style::default().fg(theme.primary).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Fetching trending anime and episode updates from AniList...",
                Style::default().fg(theme.text_dim),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [/] Live Search", Style::default().fg(theme.secondary)),
                Span::styled("   ·   ", Style::default().fg(theme.divider)),
                Span::styled(format!("󰃭 [T] Theme: {}", theme.name), Style::default().fg(theme.text_dim)),
            ]),
        ];
        frame.render_widget(Paragraph::new(msg).wrap(ratatui::widgets::Wrap { trim: true }), inner);
        return;
    };

    let show_hud = inner.width >= 104;
    let cols = if show_hud {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(11), // Glove-fit 9x6 Portrait Cover Frame
                Constraint::Length(2),  // Breathing spacer
                Constraint::Min(0),     // Editorial Metadata & CTA Deck
                Constraint::Length(30), // Right Telemetry HUD (29 inner cols, zero clipping)
            ])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(11), // Glove-fit 9x6 Portrait Cover Frame
                Constraint::Length(2),  // Breathing spacer
                Constraint::Min(0),     // Editorial Metadata & CTA Deck
            ])
            .split(inner)
    };

    // ── Zone 1: Portrait Cover Frame ─────────────────────────────────────────
    let cover_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_idle));
    let cover_inner = cover_block.inner(cols[0]);
    frame.render_widget(cover_block, cols[0]);

    let cover_occluded = active_modal_rect(frame.area(), state)
        .is_some_and(|m| m.intersects(cover_inner));
    if state.has_image_support() && !cover_occluded {
        if let Some(cover) = state
            .anime_banners
            .get_mut(&anime.id)
            .or_else(|| state.banner_cache.get_mut(&anime.id))
        {
            frame.render_stateful_widget(
                crate::ui::components::cover::hd_image_widget(),
                cover_inner,
                cover,
            );
        } else {
            let label = if state.cover_failed_anime_id == Some(anime.id) {
                if cover_inner.width < 18 { "N/A" } else { "Cover unavailable" }
            } else if cover_inner.width < 18 {
                "󰑮 ..."
            } else {
                "󰑮 Loading cover..."
            };
            let loading = Paragraph::new(label)
                .style(Style::default().fg(theme.text_dim))
                .alignment(Alignment::Center);
            frame.render_widget(loading, cover_inner);
        }
    } else {
        frame.render_widget(
            HalfblockCover {
                anime_id: anime.id,
                title: anime.display_title(),
            },
            cover_inner,
        );
    }

    // ── Shared Metadata Preparation ──────────────────────────────────────────
    let (_, _, _, badge_word) = row_meta(&state.active_row);
    let active_idx = {
        let (key, _, _, _) = row_meta(&state.active_row);
        state.row_offset(key) + 1
    };
    let focus_chip = match state.active_row {
        CategoryRow::Trending | CategoryRow::TopRated => format!(" {} #{} ", badge_word, active_idx),
        _ => format!(" {} ", badge_word),
    };

    let fmt = anime.format.as_deref().unwrap_or("TV");
    let year = anime.season_year.map(|v| v.to_string()).unwrap_or_else(|| "?".to_string());
    let eps = anime.episodes.map(|e| format!("{} EPS", e)).unwrap_or_else(|| "? EPS".to_string());
    let status = anime.status.as_deref().unwrap_or("UNKNOWN");
    let title = anime.display_title();
    let native = anime.title_native.as_deref().unwrap_or("");

    let watched = state
        .banner_progress
        .filter(|(anime_id, _)| *anime_id == anime.id)
        .map(|(_, watched)| watched)
        .unwrap_or(0);
    let in_watchlist = data.watchlist.iter().any(|item| item.id == anime.id);

    let play_label = if let Some(next_ep) = data.resume_next.get(&anime.id) {
        format!("  Continue E{} ", next_ep)
    } else {
        "  Start E1 ".to_string()
    };
    let watchlist_label = if in_watchlist {
        " 󰄬 In Watchlist "
    } else {
        " + Watchlist "
    };

    // ── Zone 2: Editorial Metadata (Strictly 8 lines — zero clipping) ─────────
    let center_w = cols[2].width as usize;

    // Row 0: Focus Pill + Specs
    let mut row0_spans = vec![
        Span::styled(
            focus_chip,
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(format!("{} {}", fmt, year), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        Span::styled("  ·  ", Style::default().fg(theme.divider)),
        Span::styled(eps, Style::default().fg(theme.text)),
        Span::styled("  ·  ", Style::default().fg(theme.divider)),
        Span::styled(status, Style::default().fg(theme.text_dim)),
    ];
    if !show_hud {
        if let Some(s) = anime.score {
            row0_spans.push(Span::styled("  ·  ", Style::default().fg(theme.divider)));
            row0_spans.push(Span::styled(
                format!(" {:.1}", s as f32 / 10.0),
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Row 1: Title + Native Subtitle (single line)
    let title_budget = center_w.saturating_sub(2);
    let mut title_spans = Vec::new();
    if !native.is_empty() && title.chars().count() + native.chars().count() + 5 <= title_budget {
        title_spans.push(Span::styled(
            title,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
        title_spans.push(Span::styled("  ·  ", Style::default().fg(theme.divider)));
        title_spans.push(Span::styled(
            native,
            Style::default().fg(theme.text_dim).add_modifier(Modifier::ITALIC),
        ));
    } else {
        title_spans.push(Span::styled(
            truncate_chars(title, title_budget),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    }

    // Row 2: Curated Genre Pills (max 4, guaranteed single line)
    let mut genre_spans = Vec::new();
    let mut g_used = 0usize;
    for genre in anime.genre_list().into_iter().take(4) {
        let chip = format!(" {} ", genre);
        let chip_len = chip.chars().count() + 1;
        if g_used + chip_len > center_w {
            break;
        }
        genre_spans.push(Span::styled(
            chip,
            Style::default().fg(theme.accent).bg(theme.border_idle),
        ));
        genre_spans.push(Span::raw(" "));
        g_used += chip_len;
    }
    if genre_spans.is_empty() {
        genre_spans.push(Span::styled(" Anime ", Style::default().fg(theme.text_dim).bg(theme.border_idle)));
    }

    // Rows 4-5: Strictly 2-sentence clean summary (full synopsis lives on Detail screen)
    let raw_desc = anime
        .description
        .as_deref()
        .unwrap_or("No description available.");
    let cleaned_desc = clean_synopsis_single_line(raw_desc);
    let line_budget = center_w.saturating_sub(1);
    let summary_desc = summarize_two_sentences(&cleaned_desc, line_budget.saturating_mul(2));
    let (syn_line1, syn_line2) = wrap_two_lines(&summary_desc, line_budget);

    // Row 6: Optional external player launch status OR breathing spacer
    let launch_status = state
        .last_played
        .as_deref()
        .filter(|_| state.last_played_anime_id == Some(anime.id));
    let row6 = if let Some(status_msg) = launch_status {
        Line::from(vec![
            Span::styled(
                " 󰐊 Player Active ",
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                truncate_chars(status_msg, center_w.saturating_sub(20)),
                Style::default().fg(theme.text_dim),
            ),
        ])
    } else {
        Line::from("")
    };

    // Row 7: Tactile Action Deck (Always Visible)
    let row7 = Line::from(vec![
        Span::styled(
            play_label,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            watchlist_label,
            Style::default()
                .fg(Color::Black)
                .bg(if in_watchlist { theme.secondary } else { theme.accent })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            " 󰘳 [d] Details ",
            Style::default().fg(theme.text).bg(theme.divider),
        ),
        Span::raw("  "),
        Span::styled(
            "  [r] Resume ",
            Style::default().fg(Color::Black).bg(theme.primary).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            " 󰋽 [w] Wizard ",
            Style::default().fg(theme.accent).bg(theme.border_idle),
        ),
    ]);

    let center_lines = vec![
        Line::from(row0_spans),
        Line::from(title_spans),
        Line::from(genre_spans),
        Line::from(""),
        Line::from(Span::styled(syn_line1, Style::default().fg(theme.text_dim))),
        Line::from(Span::styled(syn_line2, Style::default().fg(theme.text_dim))),
        row6,
        row7,
    ];
    frame.render_widget(Paragraph::new(center_lines), cols[2]);

    // ── Zone 3: Right Telemetry HUD ──────────────────────────────────────────
    if show_hud {
        let hud_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(theme.divider));
        let hud_inner = hud_block.inner(cols[3]);
        frame.render_widget(hud_block, cols[3]);

        let score_val = anime.score.map(|s| s as f32 / 10.0);
        let score_text = score_val
            .map(|v| format!(" {:.1} / 10", v))
            .unwrap_or_else(|| " N/A".to_string());
        let score_fill = score_val.map(|v| ((v / 10.0) * 10.0).round() as usize).unwrap_or(0).min(10);
        let score_tier = match anime.score.unwrap_or(0) {
            85..=100 => "Acclaimed",
            75..=84 => "Top Pick",
            60..=74 => "Solid",
            1..=59 => "Mixed",
            _ => "Unrated",
        };

        let total_eps = anime.episodes.unwrap_or(0).max(0) as usize;
        let prog_text = if total_eps > 0 {
            format!("{} / {} EPS", watched, total_eps)
        } else {
            format!("{} WATCHED", watched)
        };
        let (prog_fill, prog_pct_label) = if total_eps > 0 {
            let pct = ((watched as f32 / total_eps as f32).clamp(0.0, 1.0) * 100.0).round() as usize;
            let fill = ((watched as f32 / total_eps as f32).clamp(0.0, 1.0) * 10.0).round() as usize;
            (fill.min(10), format!("{}%", pct))
        } else if watched > 0 {
            (10, "Active".to_string())
        } else {
            (0, "Unwatched".to_string())
        };

        let audio_tag = if anime.has_dub() { "SUB / DUB" } else { "SUB ONLY" };
        let shader_tag = match state.anime4k_mode {
            crate::config::Anime4KMode::Off => None,
            crate::config::Anime4KMode::Balanced => Some("◈ A4K 2x"),
            crate::config::Anime4KMode::Ultra => Some("◈ A4K 4K"),
        };

        let mut audio_line = vec![
            Span::styled("  AUDIO  ", Style::default().fg(theme.text_dim)),
            Span::styled(
                format!(" {} ", audio_tag),
                Style::default().fg(theme.text).bg(theme.border_idle).add_modifier(Modifier::BOLD),
            ),
        ];
        if let Some(sh) = shader_tag {
            audio_line.push(Span::raw(" "));
            audio_line.push(Span::styled(sh, Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)));
        }

        let hud_lines = vec![
            Line::from(Span::styled(
                "  TELEMETRY",
                Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("  SCORE   ", Style::default().fg(theme.text_dim)),
                Span::styled(score_text, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("█".repeat(score_fill), Style::default().fg(theme.accent)),
                Span::styled("░".repeat(10usize.saturating_sub(score_fill)), Style::default().fg(theme.divider)),
                Span::styled(format!("  {}", score_tier), Style::default().fg(theme.text_dim)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  WATCH   ", Style::default().fg(theme.text_dim)),
                Span::styled(prog_text, Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("█".repeat(prog_fill), Style::default().fg(theme.primary)),
                Span::styled("░".repeat(10usize.saturating_sub(prog_fill)), Style::default().fg(theme.divider)),
                Span::styled(format!("  {}", prog_pct_label), Style::default().fg(theme.text_dim)),
            ]),
            Line::from(""),
            Line::from(audio_line),
        ];
        frame.render_widget(Paragraph::new(hud_lines), hud_inner);
    }
}

fn active_banner_anime<'a>(state: &AppState, data: &'a HomeData) -> Option<&'a Anime> {
    let (row_key, _, _, _) = row_meta(&state.active_row);
    let items = row_items(&state.active_row, data);
    items.get(state.row_offset(row_key))
}

/// Summarize a cleaned synopsis into at most two complete sentences fitting within `max_chars`.
/// Avoids mid-word/mid-sentence cutoffs in the Featured Spotlight while leaving full synopses for Detail.
pub fn summarize_two_sentences(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || max_chars == 0 {
        return String::new();
    }

    const ABBREVIATIONS: &[&str] = &[
        "mr.", "mrs.", "ms.", "dr.", "prof.", "vs.", "no.", "st.", "vol.", "ep.", "etc.", "e.g.", "i.e.",
    ];

    let chars: Vec<(usize, char)> = trimmed.char_indices().collect();
    let mut sentence_ends: Vec<usize> = Vec::new();

    for (idx, &(byte_pos, ch)) in chars.iter().enumerate() {
        if !matches!(ch, '.' | '!' | '?') {
            continue;
        }
        let next_ch = chars.get(idx + 1).map(|&(_, c)| c);
        // Must be followed by end-of-string or whitespace (skips decimals like "8.1")
        if let Some(nc) = next_ch {
            if !nc.is_whitespace() {
                continue;
            }
        }
        let end_byte = byte_pos + ch.len_utf8();
        if ch == '.' {
            let prefix_lower = trimmed[..end_byte].to_lowercase();
            let last_token = prefix_lower.split_whitespace().last().unwrap_or("");
            if ABBREVIATIONS.contains(&last_token) {
                continue;
            }
        }
        sentence_ends.push(end_byte);
        if sentence_ends.len() >= 2 {
            break;
        }
    }

    // Prefer 2 complete sentences if they fit, otherwise 1 complete sentence if it fits
    for &end_byte in sentence_ends.iter().rev() {
        let candidate = trimmed[..end_byte].trim();
        if candidate.chars().count() <= max_chars {
            return candidate.to_string();
        }
    }

    // Fallback when even the first sentence exceeds max_chars
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let limit = max_chars.saturating_sub(1);
    let mut cut_byte = trimmed.len();
    for (count, (b_idx, _)) in trimmed.char_indices().enumerate() {
        if count >= limit {
            cut_byte = b_idx;
            break;
        }
    }
    let slice = &trimmed[..cut_byte];
    let word_cut = slice.rfind(' ').filter(|&pos| pos > slice.len() / 2).unwrap_or(slice.len());
    format!("{}…", slice[..word_cut].trim_end())
}

/// Render all visible category tiers (with Smart Bento Split for Continue Watching + Watchlist).
fn render_rows(frame: &mut Frame, area: Rect, state: &mut AppState, data: &HomeData) {
    let tiers = build_shelf_tiers(data, area.width);
    if tiers.is_empty() {
        return;
    }

    let active_tier_idx = tiers
        .iter()
        .position(|tier| match tier {
            ShelfTier::BentoSplit => {
                matches!(state.active_row, CategoryRow::ContinueWatching | CategoryRow::Watchlist)
            }
            ShelfTier::Single(row) => *row == state.active_row,
        })
        .unwrap_or(0);

    let (start_idx, heights) = compute_shelf_layout(&tiers, area.height, active_tier_idx);
    let constraints: Vec<Constraint> = heights.iter().map(|&h| Constraint::Length(h)).collect();

    let tier_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, tier) in tiers.iter().skip(start_idx).take(heights.len()).enumerate() {
        if i >= tier_areas.len() {
            break;
        }
        match tier {
            ShelfTier::BentoSplit => render_bento_split(frame, tier_areas[i], state, data),
            ShelfTier::Single(row) => render_single_row(frame, tier_areas[i], state, row, data),
        }
    }
}

/// Render Continue Watching (1-2 cards) docked side-by-side with My Watchlist in one Bento tier.
fn render_bento_split(frame: &mut Frame, area: Rect, state: &mut AppState, data: &HomeData) {
    let theme = state.theme();
    let left_w = bento_left_width(data.continue_watching.len());

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_w),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    // ── Left Bento Module: Continue Watching ─────────────────────────────────
    let cw_active = state.active_row == CategoryRow::ContinueWatching;
    let cw_border = if cw_active { theme.primary } else { theme.border_idle };
    let cw_title_style = if cw_active {
        Style::default().fg(Color::Black).bg(theme.primary).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD)
    };
    let cw_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(cw_border))
        .title(Span::styled(
            format!("  Continue Watching ({}) ", data.continue_watching.len()),
            cw_title_style,
        ));
    let cw_inner = cw_block.inner(cols[0]);
    frame.render_widget(cw_block, cols[0]);

    let cw_offset = state.row_offset("continue_watching");
    let cw_vis_n = data.continue_watching.len().min(2);
    let cw_start = row_view_start(cw_offset, data.continue_watching.len(), cw_vis_n);
    for (i, anime) in data.continue_watching.iter().skip(cw_start).take(cw_vis_n).enumerate() {
        let x = cw_inner.x + 1 + i as u16 * (CARD_WIDTH + CARD_GAP);
        let rect = Rect {
            x,
            y: cw_inner.y,
            width: CARD_WIDTH,
            height: CARD_HEIGHT.min(cw_inner.height),
        };
        if rect.x + rect.width <= cw_inner.x + cw_inner.width {
            let progress = data.progress_labels.get(&anime.id).map(String::as_str);
            let selected = cw_active && (cw_start + i == cw_offset);
            render_card(frame, rect, anime, selected, None, progress, state);
        }
    }

    // ── Right Bento Module: My Watchlist ─────────────────────────────────────
    let wl_active = state.active_row == CategoryRow::Watchlist;
    let wl_border = if wl_active { theme.primary } else { theme.border_idle };
    let wl_title_style = if wl_active {
        Style::default().fg(Color::Black).bg(theme.primary).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD)
    };
    let wl_offset = state.row_offset("watchlist");
    let wl_dots = format_pagination_dots(wl_offset, data.watchlist.len());
    let mut wl_title_spans = vec![Span::styled(
        format!(" 󰋜 My Watchlist ({}) ", data.watchlist.len()),
        wl_title_style,
    )];
    wl_title_spans.extend(watchlist_source_summary_spans(data, &theme));
    let wl_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(wl_border))
        .title(Line::from(wl_title_spans))
        .title(
            Line::from(vec![
                Span::styled(wl_dots, Style::default().fg(if wl_active { theme.primary } else { theme.text_dim })),
                Span::styled(
                    format!(" [ {}/{} ] ", (wl_offset + 1).min(data.watchlist.len()), data.watchlist.len()),
                    Style::default().fg(theme.text_dim),
                ),
            ])
            .alignment(Alignment::Right),
        );
    let wl_inner = wl_block.inner(cols[2]);
    frame.render_widget(wl_block, cols[2]);

    let wl_usable_w = wl_inner.width.saturating_sub(2);
    let slots = compute_watchlist_slots(
        wl_inner.x + 1,
        wl_inner.y,
        wl_usable_w,
        wl_inner.height,
        wl_offset,
        data,
    );
    render_watchlist_slots_widgets(frame, &slots, wl_inner.height, wl_active, wl_offset, state, data);
}

/// A layout slot inside a Watchlist panel (either a source divider pillar or an anime card).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchlistRenderSlot {
    Divider {
        source: WatchlistSource,
        x: u16,
        width: u16,
    },
    Card {
        anime_idx: usize,
        source: WatchlistSource,
        rect: Rect,
    },
}

const WATCHLIST_DIVIDER_W: u16 = 5;
const WATCHLIST_DIVIDER_STEP: u16 = 6; // 5-col pillar + 1-col gap

fn watchlist_source_for(anime: &Anime, data: &HomeData) -> WatchlistSource {
    data.watchlist_sources
        .get(&anime.id)
        .copied()
        .unwrap_or(WatchlistSource::Local)
}

fn watchlist_source_counts(data: &HomeData) -> (usize, usize, usize, usize) {
    let mut al = 0usize;
    let mut mal = 0usize;
    let mut both = 0usize;
    let mut local = 0usize;
    for anime in &data.watchlist {
        match watchlist_source_for(anime, data) {
            WatchlistSource::AniList => al += 1,
            WatchlistSource::Mal => mal += 1,
            WatchlistSource::Both => both += 1,
            WatchlistSource::Local => local += 1,
        }
    }
    (al, mal, both, local)
}

fn source_accent_color(source: WatchlistSource, theme: &crate::ui::theme::ThemePalette) -> Color {
    match source {
        WatchlistSource::AniList => theme.primary,
        WatchlistSource::Mal => theme.secondary,
        WatchlistSource::Both => theme.accent,
        WatchlistSource::Local => theme.text_dim,
    }
}

fn watchlist_source_summary_spans(
    data: &HomeData,
    theme: &crate::ui::theme::ThemePalette,
) -> Vec<Span<'static>> {
    if data.watchlist_sources.is_empty() {
        return Vec::new();
    }
    let (al, mal, both, local) = watchlist_source_counts(data);
    let mut entries: Vec<(String, Color)> = Vec::new();
    if al > 0 {
        entries.push((format!("✦AL:{al}"), source_accent_color(WatchlistSource::AniList, theme)));
    }
    if mal > 0 {
        entries.push((format!("◆MAL:{mal}"), source_accent_color(WatchlistSource::Mal, theme)));
    }
    if both > 0 {
        entries.push((format!("◈Both:{both}"), source_accent_color(WatchlistSource::Both, theme)));
    }
    if local > 0 {
        entries.push((format!("•Loc:{local}"), source_accent_color(WatchlistSource::Local, theme)));
    }
    if entries.is_empty() {
        return Vec::new();
    }
    let dim_style = Style::default().fg(theme.text_dim);
    let mut spans = vec![Span::styled(" [", dim_style)];
    for (idx, (text, color)) in entries.into_iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled(" · ", dim_style));
        }
        spans.push(Span::styled(
            text,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled("] ", dim_style));
    spans
}

/// Count how many group divider pillars appear in `start..start + count`.
fn count_watchlist_dividers(start: usize, count: usize, show_dividers: bool, data: &HomeData) -> usize {
    if !show_dividers || count == 0 {
        return 0;
    }
    let end = (start + count).min(data.watchlist.len());
    let mut divs = 0usize;
    let mut prev: Option<WatchlistSource> = None;
    for anime in &data.watchlist[start..end] {
        let src = watchlist_source_for(anime, data);
        if prev != Some(src) {
            divs += 1;
            prev = Some(src);
        }
    }
    divs
}

/// Solve `(card_count, card_width)` starting at `start` after subtracting exact divider widths
/// so cards expand edge-to-edge without leaving an empty right-side void.
fn fit_watchlist_slice(
    start: usize,
    usable_w: u16,
    show_dividers: bool,
    data: &HomeData,
) -> (usize, u16) {
    let min_card_w: u16 = CARD_WIDTH;
    let max_card_w: u16 = 21;
    let avail = data.watchlist.len().saturating_sub(start);
    if avail == 0 || usable_w < min_card_w {
        return (1.min(avail), usable_w.min(max_card_w));
    }

    let mut best_n = 1usize;
    for n in 1..=avail {
        let divs = count_watchlist_dividers(start, n, show_dividers, data) as u16;
        let div_w = divs * WATCHLIST_DIVIDER_STEP;
        let gaps = n.saturating_sub(1) as u16 * CARD_GAP;
        if div_w + (n as u16) * min_card_w + gaps <= usable_w {
            best_n = n;
        } else {
            break;
        }
    }

    let divs = count_watchlist_dividers(start, best_n, show_dividers, data) as u16;
    let div_w = divs * WATCHLIST_DIVIDER_STEP;
    let gaps = best_n.saturating_sub(1) as u16 * CARD_GAP;
    let budget = usable_w.saturating_sub(div_w + gaps);
    let card_w = (budget / best_n as u16).clamp(min_card_w, max_card_w);
    (best_n, card_w)
}

/// Deterministically lay out Watchlist source dividers + cards so focused item is always visible.
pub fn compute_watchlist_slots(
    x_start: u16,
    y: u16,
    usable_w: u16,
    height: u16,
    offset: usize,
    data: &HomeData,
) -> Vec<WatchlistRenderSlot> {
    if data.watchlist.is_empty() || usable_w < 10 {
        return Vec::new();
    }

    let clamped_offset = offset.min(data.watchlist.len().saturating_sub(1));
    let show_dividers = !data.watchlist_sources.is_empty() && usable_w >= 30;

    let (first_fit, _) = fit_watchlist_slice(0, usable_w, show_dividers, data);
    let mut start = if clamped_offset < first_fit {
        0
    } else {
        row_view_start(clamped_offset, data.watchlist.len(), first_fit.max(1)).min(clamped_offset)
    };

    while start < clamped_offset {
        let (fit_n, _) = fit_watchlist_slice(start, usable_w, show_dividers, data);
        if clamped_offset < start + fit_n.max(1) {
            break;
        }
        start += 1;
    }

    let (fit_n, card_w) = fit_watchlist_slice(start, usable_w, show_dividers, data);

    let mut slots = Vec::new();
    let mut x = x_start;
    let mut prev_src: Option<WatchlistSource> = None;

    for idx in start..(start + fit_n).min(data.watchlist.len()) {
        let src = watchlist_source_for(&data.watchlist[idx], data);
        let need_div = show_dividers && prev_src != Some(src);
        let div_step = if need_div { WATCHLIST_DIVIDER_STEP } else { 0 };

        if x + div_step + card_w > x_start + usable_w {
            break;
        }
        if need_div {
            slots.push(WatchlistRenderSlot::Divider {
                source: src,
                x,
                width: WATCHLIST_DIVIDER_W,
            });
            x += WATCHLIST_DIVIDER_STEP;
        }
        slots.push(WatchlistRenderSlot::Card {
            anime_idx: idx,
            source: src,
            rect: Rect {
                x,
                y,
                width: card_w,
                height: CARD_HEIGHT.min(height),
            },
        });
        x += card_w + CARD_GAP;
        prev_src = Some(src);
    }

    slots
}

/// Hit-test mouse clicks inside a rendered Watchlist shelf container across divider pillars.
pub fn watchlist_card_index_at(
    area: Rect,
    is_bento: bool,
    state: &AppState,
    data: &HomeData,
    mx: u16,
    my: u16,
) -> Option<usize> {
    let (x_start, y_start, usable_w, h) = if is_bento {
        let left_w = bento_left_width(data.continue_watching.len());
        let wl_x = area.x + left_w + 1;
        let wl_w = area.width.saturating_sub(left_w + 1);
        let inner = Rect {
            x: wl_x + 1,
            y: area.y + 1,
            width: wl_w.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        (inner.x + 1, inner.y, inner.width.saturating_sub(2), inner.height)
    } else {
        (area.x, area.y + 1, area.width, area.height.saturating_sub(1))
    };

    let offset = state.row_offset("watchlist");
    for slot in compute_watchlist_slots(x_start, y_start, usable_w, h, offset, data) {
        if let WatchlistRenderSlot::Card { anime_idx, rect, .. } = slot {
            if mx >= rect.x && mx < rect.x + rect.width && my >= rect.y && my < rect.y + rect.height
            {
                return Some(anime_idx);
            }
        }
    }
    None
}

fn render_watchlist_divider(
    frame: &mut Frame,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    source: WatchlistSource,
    theme: &crate::ui::theme::ThemePalette,
) {
    if height < 2 || width < 4 {
        return;
    }
    let accent = source_accent_color(source, theme);
    let badge = match source {
        WatchlistSource::AniList => "✦ AL ",
        WatchlistSource::Mal => "◆ MAL",
        WatchlistSource::Both => "◈BOTH",
        WatchlistSource::Local => "• LOC",
    };

    let badge_rect = Rect {
        x,
        y,
        width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            badge,
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD),
        )),
        badge_rect,
    );

    let line_h = height.saturating_sub(1);
    let mut rail_lines = Vec::with_capacity(line_h as usize);
    for r in 0..line_h {
        let ch = if r + 1 == line_h { "  ╵  " } else { "  │  " };
        rail_lines.push(Line::from(Span::styled(ch, Style::default().fg(theme.divider))));
    }
    frame.render_widget(
        Paragraph::new(rail_lines),
        Rect {
            x,
            y: y + 1,
            width,
            height: line_h,
        },
    );
}

fn render_watchlist_slots_widgets(
    frame: &mut Frame,
    slots: &[WatchlistRenderSlot],
    height: u16,
    wl_active: bool,
    wl_offset: usize,
    state: &mut AppState,
    data: &HomeData,
) {
    let theme = state.theme();
    for slot in slots {
        match slot {
            WatchlistRenderSlot::Divider { source, x, width } => {
                let y = slots
                    .iter()
                    .find_map(|s| match s {
                        WatchlistRenderSlot::Card { rect, .. } => Some(rect.y),
                        _ => None,
                    })
                    .unwrap_or(0);
                render_watchlist_divider(frame, *x, y, *width, CARD_HEIGHT.min(height), *source, &theme);
            }
            WatchlistRenderSlot::Card {
                anime_idx,
                source,
                rect,
            } => {
                if let Some(anime) = data.watchlist.get(*anime_idx) {
                    let progress = data.progress_labels.get(&anime.id).map(String::as_str);
                    let selected = wl_active && (*anime_idx == wl_offset);
                    let tag = if data.watchlist_sources.is_empty() {
                        None
                    } else {
                        Some(*source)
                    };
                    render_card_with_source(frame, *rect, anime, selected, None, progress, tag, state);
                }
            }
        }
    }
}

/// Render a full-width horizontal category shelf with active section rail, pagination dots, and cards.
fn render_single_row(frame: &mut Frame, area: Rect, state: &mut AppState, row: &CategoryRow, data: &HomeData) {
    let theme = state.theme();
    let (key, _, label, _) = row_meta(row);
    let items = row_items(row, data);
    let is_active = state.active_row == *row;
    let ranked = matches!(row, CategoryRow::Trending | CategoryRow::TopRated);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let offset = state.row_offset(key);
    let dots = format_pagination_dots(offset, items.len());

    let rail_color = if is_active { theme.primary } else { theme.divider };
    let label_style = if is_active {
        Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD)
    };

    let base_prefix = format!("── {} ({})", label, items.len());
    let summary_spans = if *row == CategoryRow::Watchlist {
        watchlist_source_summary_spans(data, &theme)
    } else {
        Vec::new()
    };
    let summary_chars: usize = summary_spans.iter().map(|s| s.content.chars().count()).sum();
    let prefix_chars = base_prefix.chars().count() + summary_chars + 1;
    let title_suffix = format!(" {} [ {}/{} ] ──", dots, (offset + 1).min(items.len()), items.len());
    let sep_len = (area.width as usize)
        .saturating_sub(prefix_chars + title_suffix.chars().count() + 1);

    let mut header_spans = vec![Span::styled(base_prefix, label_style)];
    header_spans.extend(summary_spans);
    header_spans.push(Span::raw(" "));
    header_spans.push(Span::styled("─".repeat(sep_len), Style::default().fg(rail_color)));
    header_spans.push(Span::styled(
        format!(" {} ", dots),
        Style::default().fg(if is_active { theme.primary } else { theme.divider }),
    ));
    header_spans.push(Span::styled(
        format!("[ {}/{} ] ──", (offset + 1).min(items.len()), items.len()),
        Style::default().fg(theme.text_dim),
    ));
    frame.render_widget(Paragraph::new(Line::from(header_spans)), chunks[0]);

    let card_area = chunks[1];
    if *row == CategoryRow::Watchlist {
        let slots = compute_watchlist_slots(
            card_area.x,
            card_area.y,
            card_area.width,
            card_area.height,
            offset,
            data,
        );
        render_watchlist_slots_widgets(frame, &slots, card_area.height, is_active, offset, state, data);
        return;
    }

    let (visible_n, card_w) = row_card_metrics_for(row, card_area.width);
    let view_start = row_view_start(offset, items.len(), visible_n);

    for (i, anime) in items.iter().skip(view_start).take(visible_n).enumerate() {
        let x = card_area.x + i as u16 * (card_w + CARD_GAP);
        let rect = Rect {
            x,
            y: card_area.y,
            width: card_w,
            height: CARD_HEIGHT.min(card_area.height),
        };
        if rect.x + rect.width <= card_area.x + card_area.width {
            let progress = data.progress_labels.get(&anime.id).map(String::as_str);
            let rank = if ranked { Some(view_start + i + 1) } else { None };
            let selected = is_active && (view_start + i == offset);
            render_card(frame, rect, anime, selected, rank, progress, state);
        }
    }
}

/// Render a single anime card (cover + title + rich metadata/progress bar).
fn render_card(
    frame: &mut Frame,
    area: Rect,
    anime: &Anime,
    selected: bool,
    rank: Option<usize>,
    progress: Option<&str>,
    state: &mut AppState,
) {
    render_card_with_source(frame, area, anime, selected, rank, progress, None, state);
}

fn render_card_with_source(
    frame: &mut Frame,
    area: Rect,
    anime: &Anime,
    selected: bool,
    rank: Option<usize>,
    progress: Option<&str>,
    source_tag: Option<WatchlistSource>,
    state: &mut AppState,
) {
    if area.height < 4 {
        return;
    }

    let theme = state.theme();

    let border_color = if selected {
        theme.border_focus
    } else {
        theme.border_idle
    };

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    if selected {
        let badge = match rank {
            Some(r) => format!("  #{} · FOCUS ", r),
            None => "  FOCUS ".to_string(),
        };
        block = block.title(Span::styled(
            badge,
            Style::default()
                .fg(Color::Black)
                .bg(theme.border_focus)
                .add_modifier(Modifier::BOLD),
        ));
    } else if let Some(r) = rank {
        block = block.title(Span::styled(
            format!(" #{} ", r),
            Style::default()
                .fg(if r <= 3 { theme.accent } else { theme.text_dim })
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(src) = source_tag {
        let src_color = source_accent_color(src, &theme);
        block = block.title(
            Line::from(vec![Span::styled(
                format!(" [{}] ", src.pill_tag()),
                Style::default()
                    .fg(src_color)
                    .add_modifier(Modifier::BOLD),
            )])
            .alignment(Alignment::Right),
        );
    }

    let frame_h = area.height.saturating_sub(2).min(CARD_FRAME_HEIGHT);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(frame_h), // rounded poster frame (portrait art only)
            Constraint::Length(1),       // external title line
            Constraint::Length(1),       // external score / progress line
        ])
        .split(area);

    let cover_inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    let cover_occluded = active_modal_rect(frame.area(), state)
        .is_some_and(|m| m.intersects(cover_inner));
    if state.has_image_support() && !cover_occluded {
        let default_inner_w = CARD_WIDTH.saturating_sub(2);
        let default_inner_h = CARD_FRAME_HEIGHT.saturating_sub(2);
        let fitted = if cover_inner.width == default_inner_w && cover_inner.height == default_inner_h {
            state.cover_cache.get_mut(&anime.id)
        } else {
            state.resolve_cover_protocol(anime.id, cover_inner.width, cover_inner.height, 0.20, true)
        };
        if let Some(cover) = fitted {
            frame.render_stateful_widget(
                crate::ui::components::cover::hd_image_widget(),
                cover_inner,
                cover,
            );
        } else {
            frame.render_widget(
                HalfblockCover {
                    anime_id: anime.id,
                    title: anime.display_title(),
                },
                cover_inner,
            );
        }
    } else {
        frame.render_widget(
            HalfblockCover {
                anime_id: anime.id,
                title: anime.display_title(),
            },
            cover_inner,
        );
    }

    // External Title & Metadata below the frame (clamped to width - 1 to guarantee zero concatenation)
    let max_title_chars = chunks[1].width.saturating_sub(1) as usize;
    let title_str = anime.short_title();
    let short_title = truncate_chars(&title_str, max_title_chars);
    let title = Paragraph::new(Span::styled(
        short_title,
        Style::default()
            .fg(if selected { Color::White } else { theme.text })
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(title, chunks[1]);

    // Metadata / Colorized Progress Line
    let meta_line = build_card_meta_line(anime, progress, selected, &theme);
    frame.render_widget(Paragraph::new(meta_line), chunks[2]);
}

/// Build the bottom line of a card with colorized progress bars or score/format chips.
fn build_card_meta_line(
    anime: &Anime,
    progress: Option<&str>,
    selected: bool,
    theme: &crate::ui::theme::ThemePalette,
) -> Line<'static> {
    if let Some(prog) = progress {
        // Parse mini bar format like "E2 19:07 [━━────]" into dual-tone unicode bar
        if let Some(bracket_idx) = prog.find('[') {
            let prefix = prog[..bracket_idx].trim().to_string();
            let bar_raw = prog[bracket_idx..].trim_matches(|c| c == '[' || c == ']');
            let filled: String = bar_raw.chars().filter(|&c| c == '━').collect();
            let unfilled: String = bar_raw.chars().filter(|&c| c == '─').collect();
            let prefix_color = if selected { theme.accent } else { theme.secondary };
            let fill_color = if selected { theme.accent } else { theme.primary };
            return Line::from(vec![
                Span::styled(format!("{} ", prefix), Style::default().fg(prefix_color).add_modifier(Modifier::BOLD)),
                Span::styled(filled, Style::default().fg(fill_color)),
                Span::styled(unfilled, Style::default().fg(theme.divider)),
            ]);
        }

        return Line::from(Span::styled(
            truncate_chars(prog, 20),
            Style::default()
                .fg(if selected { theme.accent } else { theme.secondary })
                .add_modifier(Modifier::BOLD),
        ));
    }

    let score_str = anime
        .score
        .map(|s| format!(" {:.1}", s as f32 / 10.0))
        .unwrap_or_else(|| " N/A".to_string());
    let fmt_str = anime.format.as_deref().unwrap_or("TV").to_string();
    let is_airing = anime.status.as_deref() == Some("RELEASING");

    let mut spans = vec![
        Span::styled(
            score_str,
            Style::default()
                .fg(if selected { theme.accent } else { theme.text })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(theme.divider)),
        Span::styled(fmt_str, Style::default().fg(theme.text_dim)),
    ];
    if is_airing {
        spans.push(Span::styled(" · ", Style::default().fg(theme.divider)));
        spans.push(Span::styled("●", Style::default().fg(Color::Rgb(80, 250, 123))));
    }
    Line::from(spans)
}

fn format_pagination_dots(offset: usize, total: usize) -> String {
    if total <= 1 {
        return "●".to_string();
    }
    let slots = 5usize.min(total);
    let active_slot = if total <= slots {
        offset.min(slots - 1)
    } else {
        (offset * (slots - 1)) / (total - 1)
    };
    (0..slots)
        .map(|i| if i == active_slot { '●' } else { '○' })
        .collect()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= max_chars {
        text.to_string()
    } else {
        let t: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", t)
    }
}

/// Word-wrap `text` into at most 2 lines of `width` chars, clamping any overflow with `…`.
fn wrap_two_lines(text: &str, width: usize) -> (String, String) {
    if width < 4 || text.is_empty() {
        return (String::new(), String::new());
    }
    let mut lines: [String; 2] = [String::new(), String::new()];
    let mut line_idx = 0usize;
    let mut overflowed = false;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        let cur_len = lines[line_idx].chars().count();
        let needed = if cur_len == 0 { word_len } else { cur_len + 1 + word_len };

        if needed <= width {
            if cur_len > 0 {
                lines[line_idx].push(' ');
            }
            lines[line_idx].push_str(word);
        } else if line_idx == 0 {
            line_idx = 1;
            if word_len <= width {
                lines[1].push_str(word);
            } else {
                lines[1] = truncate_chars(word, width);
                overflowed = true;
                break;
            }
        } else {
            overflowed = true;
            break;
        }
    }

    if overflowed {
        let cur_len = lines[1].chars().count();
        if cur_len >= width {
            let trimmed: String = lines[1].chars().take(width.saturating_sub(1)).collect();
            lines[1] = format!("{}…", trimmed);
        } else if !lines[1].ends_with('…') {
            lines[1].push('…');
        }
    }

    (lines[0].clone(), lines[1].clone())
}

/// All data needed to render the home screen.
pub struct HomeData {
    pub featured: Option<Anime>,
    pub continue_watching: Vec<Anime>,
    pub watchlist: Vec<Anime>,
    pub watchlist_sources: HashMap<i64, WatchlistSource>,
    pub progress_labels: HashMap<i64, String>,
    pub resume_next: HashMap<i64, u32>,
    pub trending: Vec<Anime>,
    pub popular: Vec<Anime>,
    pub top_rated: Vec<Anime>,
    pub seasonal: Vec<Anime>,
}

impl HomeData {
    pub fn empty() -> Self {
        Self {
            featured: None,
            continue_watching: Vec::new(),
            watchlist: Vec::new(),
            watchlist_sources: HashMap::new(),
            progress_labels: HashMap::new(),
            resume_next: HashMap::new(),
            trending: Vec::new(),
            popular: Vec::new(),
            top_rated: Vec::new(),
            seasonal: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.continue_watching.is_empty()
            && self.watchlist.is_empty()
            && self.trending.is_empty()
            && self.popular.is_empty()
            && self.top_rated.is_empty()
            && self.seasonal.is_empty()
    }
}

/// Modal size percentages for the Watchlist Add/Remove database target picker popup.
pub fn watchlist_modal_pct(area: Rect) -> (u16, u16) {
    if area.width >= 100 && area.height >= 26 {
        (60, 52)
    } else {
        (86, 72)
    }
}

/// Render the centered opaque Watchlist Add/Remove Database Target Picker modal.
pub fn render_watchlist_target_modal(frame: &mut Frame, state: &AppState) {
    let Some(ref prompt) = state.watchlist_target_prompt else {
        return;
    };

    let area = frame.area();
    let theme = state.theme();
    let (pct_x, pct_y) = watchlist_modal_pct(area);

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    let modal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(vert[1])[1];

    let bg = Color::Rgb(15, 16, 25);
    let card_bg = Color::Rgb(22, 24, 38);
    let emerald = Color::Rgb(80, 250, 123);
    let cyan = Color::Rgb(103, 232, 249);
    let mal_blue = Color::Rgb(108, 160, 255);
    let amber = Color::Rgb(251, 191, 36);
    let muted = Color::Rgb(150, 156, 185);

    frame.render_widget(Clear, modal);
    frame.render_widget(Block::default().style(Style::default().bg(bg)), modal);

    let title_str = match prompt.mode {
        WatchlistPromptMode::Add => " + ADD TO WATCHLIST DATABASES ",
        WatchlistPromptMode::RemoveShared => " 󰅖 REMOVE FROM WATCHLIST DATABASES ",
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            title_str,
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = outer.inner(modal);
    frame.render_widget(outer, modal);

    if inner.width < 20 || inner.height < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Anime title + tracker write readiness
            Constraint::Min(6),    // 4 selectable targets
            Constraint::Length(2), // Key hints footer
        ])
        .split(inner);

    let al_cap = if state.auth.anilist.has_write_token() {
        ("AniList ✓ Live", emerald)
    } else if state.auth.has_anilist() {
        ("AniList Read-Only", amber)
    } else {
        ("AniList Offline", muted)
    };
    let mal_cap = if state.auth.mal.has_write_token() {
        ("MAL ✓ Live", emerald)
    } else if state.auth.has_mal() {
        ("MAL Read-Only", amber)
    } else {
        ("MAL Offline", muted)
    };

    let header_lines = vec![
        Line::from(vec![
            Span::styled(" Show: ", Style::default().fg(muted)),
            Span::styled(
                truncate_chars(&prompt.anime.display_title(), inner.width.saturating_sub(10) as usize),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Sync: ", Style::default().fg(muted)),
            Span::styled(al_cap.0, Style::default().fg(al_cap.1).add_modifier(Modifier::BOLD)),
            Span::styled("  ·  ", Style::default().fg(theme.divider)),
            Span::styled(mal_cap.0, Style::default().fg(mal_cap.1).add_modifier(Modifier::BOLD)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(header_lines)
            .style(Style::default().bg(bg))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let options = prompt.options_for_mode();
    let mut rows = Vec::new();
    for (idx, target) in options.iter().enumerate() {
        let sel = idx == prompt.cursor;
        let num = idx + 1;
        let (icon, label, desc, accent) = match (prompt.mode, target) {
            (WatchlistPromptMode::Add, WatchlistTarget::Both) => (
                "✦+◆",
                "Both Databases (AniList + MyAnimeList + Solus)",
                "Writes Planning live to both websites and shows under ◈ BOTH",
                amber,
            ),
            (WatchlistPromptMode::Add, WatchlistTarget::AniList) => (
                "✦  ",
                "AniList Database Only (+ Solus)",
                "Writes Planning live to anilist.co and shows under ✦ ANILIST",
                cyan,
            ),
            (WatchlistPromptMode::Add, WatchlistTarget::Mal) => (
                "◆  ",
                "MyAnimeList Database Only (+ Solus)",
                "Writes Plan to Watch live to myanimelist.net and shows under ◆ MAL",
                mal_blue,
            ),
            (WatchlistPromptMode::Add, WatchlistTarget::Local) => (
                "•  ",
                "Local Solus Only",
                "Saves locally inside Solus under • LOCAL without website writes",
                muted,
            ),
            (WatchlistPromptMode::RemoveShared, WatchlistTarget::Both) => (
                "✦+◆",
                "Remove from BOTH Websites + Solus",
                "Deletes live from AniList + MyAnimeList databases and Solus",
                Color::Rgb(255, 105, 135),
            ),
            (WatchlistPromptMode::RemoveShared, WatchlistTarget::Mal) => (
                "◆  ",
                "Remove from MyAnimeList Only",
                "Deletes live from myanimelist.net -> keeps card under ✦ ANILIST",
                mal_blue,
            ),
            (WatchlistPromptMode::RemoveShared, WatchlistTarget::AniList) => (
                "✦  ",
                "Remove from AniList Only",
                "Deletes live from anilist.co -> keeps card under ◆ MAL",
                cyan,
            ),
            (WatchlistPromptMode::RemoveShared, WatchlistTarget::Local) => (
                "•  ",
                "Hide Locally on Solus Only",
                "Removes from Solus Watchlist without touching remote websites",
                muted,
            ),
        };

        let row_bg = if sel { card_bg } else { bg };
        rows.push(Line::from(vec![
            Span::styled(
                if sel { " ▶ " } else { "   " },
                Style::default()
                    .fg(theme.primary)
                    .bg(row_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("[{num}] {icon} "),
                Style::default()
                    .fg(accent)
                    .bg(row_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                label,
                Style::default()
                    .fg(if sel { Color::White } else { Color::Rgb(215, 220, 238) })
                    .bg(row_bg)
                    .add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]));
        rows.push(Line::from(vec![
            Span::styled("       ", Style::default().bg(row_bg)),
            Span::styled(desc, Style::default().fg(muted).bg(row_bg)),
        ]));
    }

    frame.render_widget(Paragraph::new(rows).wrap(Wrap { trim: true }), chunks[1]);

    let footer = Line::from(vec![
        Span::styled(" [1-4]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
        Span::styled(" Quick Pick   ", Style::default().fg(muted)),
        Span::styled("[↑↓]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
        Span::styled(" Move   ", Style::default().fg(muted)),
        Span::styled("[Enter]", Style::default().fg(emerald).add_modifier(Modifier::BOLD)),
        Span::styled(" Confirm & Update DB   ", Style::default().fg(muted)),
        Span::styled("[Esc]", Style::default().fg(Color::Rgb(255, 105, 135)).add_modifier(Modifier::BOLD)),
        Span::styled(" Cancel", Style::default().fg(muted)),
    ]);
    frame.render_widget(Paragraph::new(footer).style(Style::default().bg(bg)), chunks[2]);
}

/// Compute the centered modal rectangle for the Exit Confirmation popup.
pub fn exit_confirm_modal_rect(area: Rect) -> Rect {
    let w = area.width.min(60).max(34.min(area.width));
    let h = area.height.min(11).max(7.min(area.height));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

/// Render the centered Exit Confirmation popup when `state.exit_confirm_open` is true.
pub fn render_exit_confirm_modal(frame: &mut Frame, state: &AppState) {
    if !state.exit_confirm_open {
        return;
    }

    let modal = exit_confirm_modal_rect(frame.area());
    let theme = state.theme();
    let bg = Color::Rgb(15, 16, 25);
    let card_bg = Color::Rgb(24, 27, 42);
    let emerald = Color::Rgb(80, 250, 123);
    let crimson = Color::Rgb(230, 57, 70);
    let amber = Color::Rgb(251, 191, 36);
    let muted = Color::Rgb(150, 156, 185);

    frame.render_widget(Clear, modal);
    frame.render_widget(Block::default().style(Style::default().bg(bg)), modal);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            " ◈ EXIT SOLUS? ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = outer.inner(modal);
    frame.render_widget(outer, modal);

    if inner.width < 16 || inner.height < 5 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Prompt message & active-session warning
            Constraint::Length(2), // Action buttons
            Constraint::Length(1), // Key hints
        ])
        .split(inner);

    let mut prompt_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Are you sure you want to close Solus?",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    if state.now_playing.is_some() {
        prompt_lines.push(Line::from(Span::styled(
            "✦ Playback is active — exiting will stop the player.",
            Style::default().fg(amber),
        )));
    } else if state.active_download.is_some() {
        prompt_lines.push(Line::from(Span::styled(
            "✦ Offline download is in progress — exiting will cancel it.",
            Style::default().fg(amber),
        )));
    }

    frame.render_widget(
        Paragraph::new(prompt_lines)
            .alignment(Alignment::Center)
            .style(Style::default().bg(bg))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let cancel_selected = state.exit_confirm_cursor == 0;
    let exit_selected = state.exit_confirm_cursor == 1;

    let cancel_style = if cancel_selected {
        Style::default()
            .fg(Color::Black)
            .bg(emerald)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(205, 210, 228)).bg(card_bg)
    };
    let exit_style = if exit_selected {
        Style::default()
            .fg(Color::White)
            .bg(crimson)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(205, 210, 228)).bg(card_bg)
    };

    let buttons = Line::from(vec![
        Span::styled("  [ Cancel · Stay ]  ", cancel_style),
        Span::styled("   ", Style::default().bg(bg)),
        Span::styled("  [ Exit Solus ]  ", exit_style),
    ]);
    frame.render_widget(
        Paragraph::new(buttons)
            .alignment(Alignment::Center)
            .style(Style::default().bg(bg)),
        chunks[1],
    );

    let hints = Line::from(vec![
        Span::styled("[←/→]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
        Span::styled(" Select  ", Style::default().fg(muted)),
        Span::styled("[Enter]", Style::default().fg(emerald).add_modifier(Modifier::BOLD)),
        Span::styled(" Confirm  ", Style::default().fg(muted)),
        Span::styled("[Y]", Style::default().fg(crimson).add_modifier(Modifier::BOLD)),
        Span::styled(" Exit  ", Style::default().fg(muted)),
        Span::styled("[N/Esc]", Style::default().fg(emerald).add_modifier(Modifier::BOLD)),
        Span::styled(" Cancel", Style::default().fg(muted)),
    ]);
    frame.render_widget(
        Paragraph::new(hints)
            .alignment(Alignment::Center)
            .style(Style::default().bg(bg)),
        chunks[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_two_lines_clamps_long_text() {
        let text = "Several hundred years ago, humans were nearly exterminated by giants called Titans who devour people without reason.";
        let (l1, l2) = wrap_two_lines(text, 40);
        assert!(l1.chars().count() <= 40);
        assert!(l2.chars().count() <= 40);
        assert!(l2.ends_with('…'));
    }

    #[test]
    fn test_bento_split_activates_for_small_continue_watching() {
        let mut data = HomeData::empty();
        data.continue_watching.push(Anime::default());
        data.watchlist.push(Anime::default());
        assert!(is_bento_split(&data, 120));
        assert!(!is_bento_split(&data, 80));

        data.continue_watching.push(Anime::default());
        assert!(is_bento_split(&data, 120));

        data.continue_watching.push(Anime::default());
        assert!(!is_bento_split(&data, 120));
    }

    #[test]
    fn test_row_view_start_prevents_trailing_blanks() {
        assert_eq!(row_view_start(0, 8, 4), 0);
        assert_eq!(row_view_start(5, 8, 4), 4);
        assert_eq!(row_view_start(7, 8, 4), 4);
        assert_eq!(row_view_start(1, 2, 2), 0);
    }

    #[test]
    fn test_home_footer_dock_never_truncates_or_clips_help() {
        use ratatui::{backend::TestBackend, Terminal};
        for width in [148u16, 126u16, 96u16] {
            let mut state = AppState::new();
            state.auth.anilist.username = Some("abdullah3".to_string());
            let backend = TestBackend::new(width, 38);
            let mut terminal = Terminal::new(backend).unwrap();
            let area = Rect::new(0, 35, width, FOOTER_HEIGHT);
            terminal
                .draw(|f| render_footer(f, area, &state))
                .expect("footer dock should render");

            let buf = terminal.backend().buffer();
            let mut rendered = String::new();
            for y in area.y..area.y + area.height {
                for x in 0..width {
                    rendered.push_str(buf[(x, y)].symbol());
                }
                rendered.push('\n');
            }
            assert!(rendered.contains("EXPLORE"), "missing EXPLORE badge at w={}", width);
            assert!(rendered.contains("ACTIONS"), "missing ACTIONS badge at w={}", width);
            assert!(rendered.contains("Search"), "missing Search at w={}", width);
            assert!(rendered.contains("Calendar"), "missing Calendar at w={}", width);
            assert!(rendered.contains("Help"), "missing full Help label at w={}", width);
            assert!(rendered.contains("Quit"), "missing full Quit label at w={}", width);
            assert!(rendered.contains("Settings"), "missing Settings at w={}", width);
        }
    }

    #[test]
    fn test_two_enlarged_portrait_shelves_fit_on_48_row_terminal() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut data = HomeData::empty();
        data.continue_watching.push(Anime::default());
        data.watchlist.push(Anime::default());
        data.trending.push(Anime::default());
        data.popular.push(Anime::default());
        data.top_rated.push(Anime::default());
        data.featured = Some(Anime::default());

        let mut state = AppState::new();
        let backend = TestBackend::new(148, 48);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, &mut state, &data))
            .expect("home screen should render enlarged portrait tiers at 148x48");

        let buf = terminal.backend().buffer();
        let mut rendered = String::new();
        for y in 0..48 {
            for x in 0..148 {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        assert!(rendered.contains("Continue Watching"), "missing row 1");
        assert!(rendered.contains("Trending Now"), "missing row 2");
        assert!(rendered.contains("ACTIONS"), "missing bottom command dock");
    }

    #[test]
    fn test_bento_watchlist_fills_width_without_dead_right_void() {
        // Matches Screenshot (168 cols wide -> left_w 26, spacer 1, wl outer 141, inner 139, usable 137)
        let (vis_n, card_w) = compute_row_card_metrics(137);
        assert_eq!(vis_n, 6);
        assert_eq!(card_w, 21);
        let used = vis_n as u16 * card_w + (vis_n as u16 - 1) * CARD_GAP;
        assert!(137 - used <= 2, "leftover margin {} too wide", 137 - used);

        // Also check 160-col terminal (wl usable = 129)
        let (vis_n2, card_w2) = compute_row_card_metrics(129);
        let used2 = vis_n2 as u16 * card_w2 + (vis_n2 as u16 - 1) * CARD_GAP;
        assert!(129 - used2 <= 5, "leftover margin {} too wide at 129 usable", 129 - used2);
    }

    #[test]
    fn test_watchlist_renders_dividers_and_target_modal_cleanly() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut data = HomeData::empty();
        data.continue_watching.push(Anime {
            id: 10,
            title_romaji: "Solo Leveling".into(),
            ..Anime::default()
        });
        let al_show = Anime {
            id: 1,
            title_romaji: "Attack on Titan".into(),
            ..Anime::default()
        };
        let mal_show = Anime {
            id: 2,
            title_romaji: "Vinland Saga".into(),
            ..Anime::default()
        };
        let both_show = Anime {
            id: 3,
            title_romaji: "Frieren".into(),
            ..Anime::default()
        };
        data.watchlist = vec![al_show, mal_show, both_show.clone()];
        data.watchlist_sources.insert(1, WatchlistSource::AniList);
        data.watchlist_sources.insert(2, WatchlistSource::Mal);
        data.watchlist_sources.insert(3, WatchlistSource::Both);
        data.featured = Some(both_show.clone());

        let mut state = AppState::new();
        state.auth.anilist.username = Some("al_me".into());
        state.auth.mal.username = Some("mal_me".into());

        let backend = TestBackend::new(150, 46);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, &mut state, &data))
            .expect("home watchlist dividers should render");

        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..46 {
            for x in 0..150 {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        assert!(out.contains("✦ AL"), "missing AniList divider badge");
        assert!(out.contains("◆ MAL"), "missing MAL divider badge");
        assert!(out.contains("◈BOTH"), "missing Both divider badge");
        assert!(out.contains("[AL]"), "missing [AL] card badge");
        assert!(out.contains("[MAL]"), "missing [MAL] card badge");

        // Now open interactive Watchlist Target Picker modal and verify render
        state.open_watchlist_prompt(both_show, WatchlistPromptMode::Add, false);
        terminal
            .draw(|f| {
                render(f, &mut state, &data);
                render_watchlist_target_modal(f, &state);
            })
            .expect("target modal should render cleanly");
        let buf2 = terminal.backend().buffer();
        let mut modal_out = String::new();
        for y in 0..46 {
            for x in 0..150 {
                modal_out.push_str(buf2[(x, y)].symbol());
            }
            modal_out.push('\n');
        }
        assert!(modal_out.contains("ADD TO WATCHLIST DATABASES"));
        assert!(modal_out.contains("Both Databases (AniList + MyAnimeList + Solus)"));
    }

    #[test]
    fn test_watchlist_slots_eliminate_right_void_with_dividers() {
        let mut data = HomeData::empty();
        for id in 1..=6 {
            data.watchlist.push(Anime {
                id,
                title_romaji: format!("AL Show {id}"),
                ..Anime::default()
            });
            data.watchlist_sources.insert(id, WatchlistSource::AniList);
        }
        data.watchlist.push(Anime {
            id: 7,
            title_romaji: "Local Show".into(),
            ..Anime::default()
        });
        data.watchlist_sources.insert(7, WatchlistSource::Local);

        let usable_w = 137u16;
        let slots = compute_watchlist_slots(10, 5, usable_w, 10, 0, &data);
        let cards: Vec<_> = slots
            .iter()
            .filter_map(|s| match s {
                WatchlistRenderSlot::Card { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect();
        assert_eq!(cards.len(), 6, "all 6 AL cards should pack cleanly alongside the divider");
        let right_edge = cards.last().unwrap().x + cards.last().unwrap().width;
        let trailing_slack = (10 + usable_w).saturating_sub(right_edge);
        assert!(
            trailing_slack <= 4,
            "expected minimal right slack with divider present, got {trailing_slack} cols"
        );
    }

    #[test]
    fn test_top_bar_renders_profile_badge_without_concatenation() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut data = HomeData::empty();
        data.trending.push(Anime::default());
        data.popular.push(Anime::default());

        let mut state = AppState::new();
        state.active_profile_name = "Brother".to_string();
        state.active_profile_icon = "✦".to_string();

        let backend = TestBackend::new(140, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 140, 1);
        terminal
            .draw(|f| render_top_bar(f, area, &state, &data))
            .unwrap();

        let buf = terminal.backend().buffer();
        let mut row0 = String::new();
        for x in 0..140 {
            row0.push_str(buf[(x, 0)].symbol());
        }
        assert!(row0.contains("✦ Brother [P]  ·  "));
        assert!(!row0.contains("[P]Guest"));
        assert!(!row0.contains("SOLUSTrending"));

        // Non-empty queue renders Queue pill with explicit divider spacing
        state.queue.enqueue_back(crate::services::queue::QueueItem::new(
            1,
            None,
            Some(2023),
            "Frieren".to_string(),
            "Frieren".to_string(),
            5,
            "1080p".to_string(),
            false,
            "English".to_string(),
            "mpv".to_string(),
        ));
        terminal
            .draw(|f| render_top_bar(f, area, &state, &data))
            .unwrap();
        let buf2 = terminal.backend().buffer();
        let mut row0_q = String::new();
        for x in 0..140 {
            row0_q.push_str(buf2[(x, 0)].symbol());
        }
        assert!(row0_q.contains("◆ Queue: 1  ·  ✦ Brother [P]"));
    }

    #[test]
    fn test_exit_confirm_modal_renders_cleanly_with_and_without_playback() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut state = AppState::new();
        state.open_exit_confirm();
        state.now_playing = Some("Frieren — Episode 1".to_string());

        for (w, h) in [(100u16, 30u16), (60u16, 20u16)] {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| render_exit_confirm_modal(f, &state))
                .unwrap();

            let buf = terminal.backend().buffer();
            let mut text = String::new();
            for y in 0..h {
                for x in 0..w {
                    text.push_str(buf[(x, y)].symbol());
                }
            }
            assert!(text.contains("EXIT SOLUS?"));
            assert!(text.contains("Cancel · Stay"));
            assert!(text.contains("Exit Solus"));
            assert!(text.contains("Playback is active"));
            assert!(text.contains("[N/Esc] Cancel"), "footer hint truncated in {}x{}: {text}", w, h);
        }

        let (_, cw_width) = row_card_metrics_for(&CategoryRow::ContinueWatching, 140);
        assert_eq!(cw_width, CARD_WIDTH);
    }

    #[test]
    fn test_summarize_two_sentences_picks_complete_sentences() {
        let desc = "Aoi Ashito's game has earned him a spot on the A team. Surrounded by elite players like Dr. Akutsu, practices are merciless. Even more trials await him in the third sentence.";
        let two = summarize_two_sentences(desc, 180);
        assert_eq!(
            two,
            "Aoi Ashito's game has earned him a spot on the A team. Surrounded by elite players like Dr. Akutsu, practices are merciless."
        );
        // Tighter budget falls back cleanly to 1 complete sentence rather than mid-word ellipsis
        let one = summarize_two_sentences(desc, 70);
        assert_eq!(one, "Aoi Ashito's game has earned him a spot on the A team.");
    }

    #[test]
    fn test_hero_hud_does_not_clip_anime4k_tag() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut data = HomeData::empty();
        data.trending.push(Anime {
            id: 7,
            title_romaji: "Aoashi Season 2".to_string(),
            description: Some(
                "Aoi Ashito earns a spot on the A team. Practices with elite teammates push him to his limits. A third sentence should not appear in Spotlight."
                    .to_string(),
            ),
            score: Some(81),
            episodes: Some(24),
            ..Anime::default()
        });

        let mut state = AppState::new();
        state.active_row = CategoryRow::Trending;
        state.anime4k_mode = crate::config::Anime4KMode::Balanced;

        let backend = TestBackend::new(148, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let featured = data.trending[0].clone();
        terminal
            .draw(|f| render_featured(f, Rect::new(0, 0, 148, 10), &mut state, Some(&featured), &data))
            .unwrap();

        let buf = terminal.backend().buffer();
        let mut rendered = String::new();
        for y in 0..10 {
            for x in 0..148 {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        assert!(rendered.contains("◈ A4K 2x"), "Anime4K badge was clipped: {rendered}");
        assert!(rendered.contains("push him to his limits."));
        assert!(!rendered.contains("third sentence"));
    }
}

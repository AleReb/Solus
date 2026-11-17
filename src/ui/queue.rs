//! Watch Session Queue & Profile-Scoped Saved Playlists Bento Overlay (`[Q]` / `[Ctrl+Q]`)
//! and Detail Batch Quick-Queue Mini-Modal (`[E]`).

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    config::Config,
    services::{
        queue::{parse_batch_queue_input, PrefetchStatus},
        shaders::AnimeEra,
    },
    state::AppState,
    ui::theme::ThemePalette,
};

const BG_MODAL: Color = Color::Rgb(15, 16, 25);
const BG_CARD: Color = Color::Rgb(21, 23, 36);
const BG_CARD_ACTIVE: Color = Color::Rgb(30, 32, 52);
const FG_MUTED: Color = Color::Rgb(148, 154, 180);
const EMERALD: Color = Color::Rgb(80, 250, 123);
const AMBER: Color = Color::Rgb(251, 191, 36);
const CYAN: Color = Color::Rgb(125, 207, 255);

/// Responsive modal `(percent_x, percent_y)` matching Home backdrop clearing.
pub fn modal_pct(area: Rect) -> (u16, u16) {
    if area.width >= 110 && area.height >= 32 {
        (88, 84)
    } else {
        (96, 92)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}

/// Strictly clamp `text` to `max_chars` (appending `…` at a clean word boundary when truncated)
/// so adjacent badges/columns always keep clean whitespace separation with zero concatenation.
pub fn clamp_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let budget = max_chars - 1;
    let slice: String = text.chars().take(budget).collect();
    if let Some(last_space) = slice.rfind(char::is_whitespace) {
        if last_space >= budget / 2 {
            let clean = slice[..last_space].trim_end();
            if !clean.is_empty() {
                return format!("{}…", clean);
            }
        }
    }
    format!("{}…", slice.trim_end())
}

/// Map a subtitle language name/keyword to a crisp uppercase 3-letter badge (never mid-word clipped).
fn short_lang_code(lang: &str) -> String {
    let lower = lang.trim().to_lowercase();
    if lower.starts_with("eng") || lower == "en" {
        "ENG".to_string()
    } else if lower.starts_with("ara") || lower == "ar" {
        "ARA".to_string()
    } else if lower.starts_with("spa") || lower == "es" {
        "SPA".to_string()
    } else if lower.starts_with("fre") || lower.starts_with("fra") || lower == "fr" {
        "FRE".to_string()
    } else if lower.starts_with("ger") || lower.starts_with("deu") || lower == "de" {
        "GER".to_string()
    } else if lower.starts_with("por") || lower == "pt" {
        "POR".to_string()
    } else if lower.starts_with("ind") || lower == "id" {
        "IND".to_string()
    } else if lower.starts_with("rus") || lower == "ru" {
        "RUS".to_string()
    } else if lower.starts_with("ita") || lower == "it" {
        "ITA".to_string()
    } else if lower.starts_with("tur") || lower == "tr" {
        "TUR".to_string()
    } else {
        lang.trim().chars().take(3).collect::<String>().to_uppercase()
    }
}

fn current_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Render the centered Split-Bento Watch Session Queue & Saved Playlists Hub (`[Q]`).
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    let frame_area = frame.area();
    let (pct_x, pct_y) = modal_pct(frame_area);
    let area = centered_rect(pct_x, pct_y, frame_area);
    frame.render_widget(Clear, area);

    let theme = state.theme();
    let now = current_unix();
    let auto_badge = if state.queue.auto_advance { "Auto: ON" } else { "Auto: OFF" };
    let loop_badge = if state.queue.loop_mode { "Loop: ON" } else { "Loop: OFF" };

    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " ◈ SOLUS // WATCH QUEUE & PLAYLISTS ",
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · ", Style::default().fg(theme.divider)),
            Span::styled(
                format!("{} {} ", state.active_profile_icon, state.active_profile_name),
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · ", Style::default().fg(theme.divider)),
            Span::styled(
                format!("{} ", auto_badge),
                Style::default()
                    .fg(if state.queue.auto_advance { EMERALD } else { FG_MUTED })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · ", Style::default().fg(theme.divider)),
            Span::styled(
                format!("{} ", loop_badge),
                Style::default()
                    .fg(if state.queue.loop_mode { CYAN } else { FG_MUTED })
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(BG_MODAL));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if inner.width < 24 || inner.height < 10 {
        return;
    }

    let save_h = if state.queue_save_mode { 3 } else { 0 };
    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(save_h),
            Constraint::Length(2),
        ])
        .split(inner.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }));

    // Split main stage cleanly into balanced Bento columns (54% Queue / 46% Playlists)
    let split_horizontal = v_chunks[0].width >= 68;
    if split_horizontal {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(54),
                Constraint::Length(1),
                Constraint::Percentage(46),
            ])
            .split(v_chunks[0]);

        render_session_queue_pane(frame, cols[0], state, &theme, now);
        render_saved_playlists_pane(frame, cols[2], state, &theme);
    } else if state.queue_focus_pane == 0 {
        render_session_queue_pane(frame, v_chunks[0], state, &theme, now);
    } else {
        render_saved_playlists_pane(frame, v_chunks[0], state, &theme);
    }

    if state.queue_save_mode {
        render_save_playlist_prompt(frame, v_chunks[1], state, &theme);
    }

    render_queue_footer(frame, v_chunks[2], state, &theme);
}

fn render_session_queue_pane(
    frame: &mut Frame,
    area:  Rect,
    state: &AppState,
    theme: &ThemePalette,
    now:   u64,
) {
    let focused = state.queue_focus_pane == 0;
    let pane_border = if focused { theme.primary } else { theme.border_idle };

    let pane = Block::default()
        .title(Line::from(vec![
            Span::styled(
                format!(" ◆ SESSION QUEUE ({}) ", state.queue.len()),
                Style::default()
                    .fg(if focused { theme.primary } else { theme.text_dim })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if focused { " [ACTIVE] " } else { " [Tab] " },
                Style::default().fg(if focused { Color::Black } else { FG_MUTED }).bg(
                    if focused { theme.primary } else { BG_CARD },
                ),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(pane_border))
        .style(Style::default().bg(BG_MODAL));

    let inner = pane.inner(area);
    frame.render_widget(pane, area);

    if inner.width < 12 || inner.height < 6 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // NOW PLAYING card
            Constraint::Min(3),    // UP NEXT list
            Constraint::Length(3), // Prefetch / session stats bar
        ])
        .split(inner);

    // 1. NOW PLAYING card (zero empty margin waste)
    let now_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if state.now_playing.is_some() {
            EMERALD
        } else {
            theme.border_idle
        }))
        .style(Style::default().bg(BG_CARD));
    let now_inner = now_block.inner(chunks[0]);
    frame.render_widget(now_block, chunks[0]);

    let now_w = now_inner.width.saturating_sub(2) as usize;
    let now_lines = if let Some(ref np) = state.now_playing {
        let opts_summary = state
            .last_play_options
            .as_ref()
            .map(|o| {
                let mode = if o.dub { "DUB" } else { "SUB" };
                format!(
                    "{} · {} · Sub: {}",
                    o.quality,
                    mode,
                    o.sub_lang.as_deref().unwrap_or("Eng")
                )
            })
            .unwrap_or_else(|| "External player active".to_string());
        vec![
            Line::from(vec![
                Span::styled(" ▶ NOW  ", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
                Span::styled(
                    clamp_text(np, now_w.saturating_sub(8)),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                format!("   {}", clamp_text(&opts_summary, now_w.saturating_sub(3))),
                Style::default().fg(FG_MUTED),
            )),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled(" ◎ IDLE  ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(
                    clamp_text(
                        if state.queue.is_empty() {
                            "Enqueue any episode with [e] or [E]"
                        } else {
                            "Press [Enter] to launch item #1"
                        },
                        now_w.saturating_sub(9),
                    ),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(Span::styled(
                clamp_text(
                    "   Auto-Advance starts N+1 seamlessly on episode finish",
                    now_w,
                ),
                Style::default().fg(FG_MUTED),
            )),
        ]
    };
    frame.render_widget(Paragraph::new(now_lines).style(Style::default().bg(BG_CARD)), now_inner);

    // 2. UP NEXT list
    let list_w = chunks[1].width as usize;
    let list_h = chunks[1].height as usize;
    if state.queue.is_empty() {
        let empty_lines = vec![
            Line::from(Span::styled(
                format!(
                    " ── UP NEXT {}",
                    "─".repeat(list_w.saturating_sub(13))
                ),
                Style::default().fg(theme.divider),
            )),
            Line::from(Span::styled(
                clamp_text(" Queue is empty. Quick ways to fill it:", list_w),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                clamp_text("  • Press [e] on Home or Detail to enqueue next ep", list_w),
                Style::default().fg(FG_MUTED),
            )),
            Line::from(Span::styled(
                clamp_text("  • Press [E] on Detail for Next 3 / Rest / Range", list_w),
                Style::default().fg(FG_MUTED),
            )),
            Line::from(Span::styled(
                clamp_text("  • Press [Tab] -> [Enter] to load a Saved Playlist", list_w),
                Style::default().fg(CYAN),
            )),
            Line::from(Span::styled(
                clamp_text("  • Press [Ctrl+Q] or [Q] anytime to toggle this deck", list_w),
                Style::default().fg(AMBER),
            )),
        ];
        frame.render_widget(Paragraph::new(empty_lines), chunks[1]);
    } else {
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(list_h);
        lines.push(Line::from(Span::styled(
            format!(
                " ── UP NEXT {}",
                "─".repeat(list_w.saturating_sub(13))
            ),
            Style::default().fg(theme.divider),
        )));

        // Each entry takes 2 rows (title + badge, then metadata subline)
        let rows_per_item = 2usize;
        let max_items_visible = list_h.saturating_sub(1) / rows_per_item;
        let start = if max_items_visible == 0 {
            0
        } else {
            state
                .queue_cursor
                .saturating_sub(max_items_visible.saturating_sub(1))
        };

        for (rel_idx, item) in state
            .queue
            .items
            .iter()
            .enumerate()
            .skip(start)
            .take(max_items_visible.max(1))
        {
            let selected = focused && rel_idx == state.queue_cursor;
            let status = item.status(now);
            let badge = status.badge();
            let badge_w = badge.chars().count();
            let pointer = if selected { " ▶ " } else { "   " };
            let head_num = format!("{}{}. ", pointer, rel_idx + 1);
            let ep_tag = if list_w < 42 {
                format!(" E{}", item.episode)
            } else {
                format!(" · Ep {}", item.episode)
            };
            let min_gap = 2usize;
            let fixed_w = head_num.chars().count() + ep_tag.chars().count() + badge_w + min_gap + 1;
            let title_budget = list_w.saturating_sub(fixed_w).max(4);
            let clamped_title = clamp_text(&item.title, title_budget);
            let left_str = format!("{}{}{}", head_num, clamped_title, ep_tag);
            let spacer_len = list_w
                .saturating_sub(left_str.chars().count() + badge_w + 1)
                .max(1);

            let badge_color = match status {
                PrefetchStatus::Ready => EMERALD,
                PrefetchStatus::Resolving => AMBER,
                PrefetchStatus::Queued => CYAN,
                PrefetchStatus::Stale => Color::Rgb(255, 140, 80),
            };
            let row_bg = if selected { BG_CARD_ACTIVE } else { BG_MODAL };

            lines.push(Line::from(vec![
                Span::styled(
                    head_num,
                    Style::default()
                        .fg(if selected { theme.primary } else { FG_MUTED })
                        .bg(row_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}{}", clamped_title, ep_tag),
                    Style::default()
                        .fg(Color::White)
                        .bg(row_bg)
                        .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::styled(" ".repeat(spacer_len), Style::default().bg(row_bg)),
                Span::styled(
                    badge.to_string(),
                    Style::default().fg(badge_color).bg(row_bg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default().bg(row_bg)),
            ]));

            let era = AnimeEra::from_year(item.season_year).short_badge();
            let mode = if item.dub { "DUB" } else { "SUB" };
            let cache_note = match status {
                PrefetchStatus::Ready => {
                    let rem = item
                        .prefetch
                        .as_ref()
                        .map(|p| p.remaining_secs(now) / 60)
                        .unwrap_or(0);
                    format!("Prefetched ({rem}m TTL)")
                }
                PrefetchStatus::Resolving => "Prefetching stream...".to_string(),
                PrefetchStatus::Queued => "Waiting in queue".to_string(),
                PrefetchStatus::Stale => "Expired · re-warms auto".to_string(),
            };
            let subline = format!(
                "     {} · {} · {} · {} · {}",
                item.quality, mode, item.sub_lang, era, cache_note
            );
            lines.push(Line::from(Span::styled(
                clamp_text(&subline, list_w.saturating_sub(1)),
                Style::default().fg(FG_MUTED).bg(row_bg),
            )));
        }

        frame.render_widget(Paragraph::new(lines), chunks[1]);
    }

    // 3. Bottom Session Status Bar
    let opt_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme.divider));
    let opt_inner = opt_block.inner(chunks[2]);
    frame.render_widget(opt_block, chunks[2]);

    let next_status = state
        .queue
        .items
        .front()
        .map(|i| match i.status(now) {
            PrefetchStatus::Ready => "Next episode warm (0ms launch)".to_string(),
            PrefetchStatus::Resolving => "Warming next episode link...".to_string(),
            PrefetchStatus::Queued => "Next item queued".to_string(),
            PrefetchStatus::Stale => "Refreshing next episode link".to_string(),
        })
        .unwrap_or_else(|| "Add episodes with [e] / [E] or load a playlist".to_string());

    let est_mins = state.queue.len() * 24;
    let est_str = if est_mins >= 60 {
        format!("~{}h {:02}m", est_mins / 60, est_mins % 60)
    } else {
        format!("~{}m", est_mins)
    };
    let opt_w = opt_inner.width as usize;
    let opt_lines = vec![
        Line::from(Span::styled(
            clamp_text(
                &format!(
                    " Loop: {}  ·  Items: {}  ·  Est: {}",
                    if state.queue.loop_mode { "ON " } else { "OFF" },
                    state.queue.len(),
                    est_str
                ),
                opt_w,
            ),
            Style::default().fg(CYAN),
        )),
        Line::from(Span::styled(
            clamp_text(&format!(" {}", next_status), opt_w),
            Style::default().fg(FG_MUTED),
        )),
    ];
    frame.render_widget(Paragraph::new(opt_lines), opt_inner);
}

fn render_saved_playlists_pane(
    frame: &mut Frame,
    area:  Rect,
    state: &AppState,
    theme: &ThemePalette,
) {
    let focused = state.queue_focus_pane == 1;
    let pane_border = if focused { theme.secondary } else { theme.border_idle };

    let pane = Block::default()
        .title(Line::from(vec![
            Span::styled(
                format!(" ❖ SAVED PLAYLISTS ({}) ", state.saved_playlists.len()),
                Style::default()
                    .fg(if focused { theme.secondary } else { theme.text_dim })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if focused { " [ACTIVE] " } else { " [Tab] " },
                Style::default().fg(if focused { Color::Black } else { FG_MUTED }).bg(
                    if focused { theme.secondary } else { BG_CARD },
                ),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(pane_border))
        .style(Style::default().bg(BG_MODAL));

    let inner = pane.inner(area);
    frame.render_widget(pane, area);

    if inner.width < 12 || inner.height < 6 {
        return;
    }

    if state.saved_playlists.is_empty() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(5)])
            .split(inner);
        let list_w = chunks[0].width as usize;
        let empty = vec![
            Line::from(Span::styled(
                clamp_text(" ── NO SAVED PLAYLISTS YET ─────────────────", list_w),
                Style::default().fg(theme.divider),
            )),
            Line::from(Span::styled(
                clamp_text(" Playlists store show/ep intent & audio/quality specs.", list_w),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                clamp_text(" Links resolve fresh on play so CDN URLs never rot.", list_w),
                Style::default().fg(FG_MUTED),
            )),
            Line::from(Span::styled(
                clamp_text(" • [s] Save current Session Queue as a playlist", list_w),
                Style::default().fg(EMERALD),
            )),
            Line::from(Span::styled(
                clamp_text(" • [i] Import shared .json packs from profile dir", list_w),
                Style::default().fg(CYAN),
            )),
        ];
        frame.render_widget(Paragraph::new(empty), chunks[0]);

        let tip_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme.divider));
        let tip_inner = tip_block.inner(chunks[1]);
        frame.render_widget(tip_block, chunks[1]);
        let t_w = tip_inner.width as usize;
        let tips = vec![
            Line::from(Span::styled(
                clamp_text(" ── WATCH PARTY WORKFLOW ────────────────────", t_w),
                Style::default().fg(theme.divider),
            )),
            Line::from(Span::styled(
                clamp_text(" 1. Enqueue eps [e]/[E] -> Save [s] -> Loop [L]", t_w),
                Style::default().fg(FG_MUTED),
            )),
            Line::from(Span::styled(
                clamp_text(" 2. Export [w] to share playlists/<name>.json", t_w),
                Style::default().fg(FG_MUTED),
            )),
            Line::from(Span::styled(
                clamp_text(" 3. Press [Tab] to switch between Queue & Playlists", t_w),
                Style::default().fg(AMBER),
            )),
        ];
        frame.render_widget(Paragraph::new(tips), tip_inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(56), // Saved playlist cards
            Constraint::Percentage(44), // Selected playlist preview items
        ])
        .split(inner);

    let list_w = chunks[0].width as usize;
    {
        let max_vis = (chunks[0].height as usize / 2).max(1);
        let start = state.playlist_cursor.saturating_sub(max_vis.saturating_sub(1));
        let mut lines = Vec::new();

        for (idx, pl) in state
            .saved_playlists
            .iter()
            .enumerate()
            .skip(start)
            .take(max_vis)
        {
            let selected = idx == state.playlist_cursor;
            let row_bg = if selected { BG_CARD_ACTIVE } else { BG_MODAL };
            let ptr = if selected { " ▶ ◈ " } else { "   ◈ " };
            let count_tag = if pl.loop_default {
                format!("{} items · Loop", pl.item_count)
            } else {
                format!("{} items", pl.item_count)
            };
            let title_budget = list_w
                .saturating_sub(ptr.chars().count() + count_tag.chars().count() + 3)
                .max(6);
            let clamped_name = clamp_text(&pl.name, title_budget);
            let spacer = list_w
                .saturating_sub(ptr.chars().count() + clamped_name.chars().count() + count_tag.chars().count() + 1)
                .max(1);

            lines.push(Line::from(vec![
                Span::styled(
                    ptr.to_string(),
                    Style::default()
                        .fg(if selected { theme.secondary } else { theme.accent })
                        .bg(row_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    clamped_name,
                    Style::default()
                        .fg(Color::White)
                        .bg(row_bg)
                        .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::styled(" ".repeat(spacer), Style::default().bg(row_bg)),
                Span::styled(
                    format!("{} ", count_tag),
                    Style::default().fg(theme.accent).bg(row_bg),
                ),
            ]));

            let preview = if pl.preview_titles.is_empty() {
                "     Empty playlist".to_string()
            } else {
                format!("     {}", pl.preview_titles.join(" · "))
            };
            lines.push(Line::from(Span::styled(
                clamp_text(&preview, list_w.saturating_sub(1)),
                Style::default().fg(FG_MUTED).bg(row_bg),
            )));
        }

        frame.render_widget(Paragraph::new(lines), chunks[0]);
    }

    // Bottom preview pane: items of selected playlist (interactive editor when playlist_focus_items)
    let editing_items = focused && state.playlist_focus_items;
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if editing_items { theme.secondary } else { theme.divider }));
    let preview_inner = preview_block.inner(chunks[1]);
    frame.render_widget(preview_block, chunks[1]);

    let p_w = preview_inner.width as usize;
    let header_title = if editing_items {
        " ◈ PLAYLIST ITEMS [EDITING] "
    } else if !state.playlist_preview_items.is_empty() {
        " ── SELECTED PLAYLIST ITEMS ([e] Edit/Reorder) "
    } else {
        " ── SELECTED PLAYLIST ITEMS "
    };
    let mut p_lines = vec![Line::from(Span::styled(
        clamp_text(header_title, p_w),
        Style::default()
            .fg(if editing_items { theme.secondary } else { theme.divider })
            .add_modifier(if editing_items { Modifier::BOLD } else { Modifier::empty() }),
    ))];
    if state.playlist_preview_items.is_empty() {
        p_lines.push(Line::from(Span::styled(
            "   Select a playlist above to inspect its lineup.",
            Style::default().fg(FG_MUTED),
        )));
    } else {
        let avail = preview_inner.height.saturating_sub(1).max(1) as usize;
        let start = if editing_items {
            state.playlist_item_cursor.saturating_sub(avail.saturating_sub(1))
        } else {
            0
        };
        for (i, it) in state
            .playlist_preview_items
            .iter()
            .enumerate()
            .skip(start)
            .take(avail)
        {
            let selected = editing_items && i == state.playlist_item_cursor;
            let row_bg = if selected { BG_CARD_ACTIVE } else { BG_MODAL };
            let mode = if it.dub { "DUB" } else { "SUB" };
            let sub_short = short_lang_code(&it.sub_lang);
            let right_meta = format!("{} · {} · {}", mode, it.quality, sub_short);
            let ep_tag = format!(" · E{:02}", it.episode);
            let prefix = if selected {
                format!(" ▶ {}. ", i + 1)
            } else {
                format!("   {}. ", i + 1)
            };
            let budget = p_w
                .saturating_sub(prefix.chars().count() + ep_tag.chars().count() + right_meta.chars().count() + 3)
                .max(6);
            let name = clamp_text(&it.title, budget);
            let gap = p_w
                .saturating_sub(prefix.chars().count() + name.chars().count() + ep_tag.chars().count() + right_meta.chars().count() + 1)
                .max(1);
            p_lines.push(Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default()
                        .fg(if selected { theme.secondary } else { FG_MUTED })
                        .bg(row_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}{}", name, ep_tag),
                    Style::default()
                        .fg(Color::White)
                        .bg(row_bg)
                        .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::styled(" ".repeat(gap), Style::default().bg(row_bg)),
                Span::styled(
                    format!("{} ", right_meta),
                    Style::default()
                        .fg(if selected { theme.accent } else { FG_MUTED })
                        .bg(row_bg),
                ),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(p_lines), preview_inner);
}

fn render_save_playlist_prompt(
    frame: &mut Frame,
    area:  Rect,
    state: &AppState,
    theme: &ThemePalette,
) {
    let block = Block::default()
        .title(Span::styled(
            " ❖ Save Current Session Queue as Playlist ",
            Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(EMERALD))
        .style(Style::default().bg(BG_CARD));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let line = Line::from(vec![
        Span::styled("  Name: ", Style::default().fg(FG_MUTED)),
        Span::styled(
            format!("{}_", state.queue_save_input),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   [Enter] Save  [Esc] Cancel", Style::default().fg(theme.accent)),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
}

fn render_queue_footer(
    frame: &mut Frame,
    area:  Rect,
    state: &AppState,
    theme: &ThemePalette,
) {
    let (line1, line2) = if state.queue_focus_pane == 0 {
        (
            Line::from(vec![
                Span::styled(" [Enter]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
                Span::styled(" Play  ", Style::default().fg(FG_MUTED)),
                Span::styled("[J/K]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
                Span::styled(" Move  ", Style::default().fg(FG_MUTED)),
                Span::styled("[a]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
                Span::styled(" Audio  ", Style::default().fg(FG_MUTED)),
                Span::styled("[l]", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Sub  ", Style::default().fg(FG_MUTED)),
                Span::styled("[v]", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
                Span::styled(" Qual  ", Style::default().fg(FG_MUTED)),
                Span::styled("[c]", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                Span::styled(" Config  ", Style::default().fg(FG_MUTED)),
                Span::styled("[d]", Style::default().fg(Color::Rgb(255, 85, 85)).add_modifier(Modifier::BOLD)),
                Span::styled(" Drop", Style::default().fg(FG_MUTED)),
            ]),
            Line::from(vec![
                Span::styled(" [s]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
                Span::styled(" Save Playlist  ", Style::default().fg(FG_MUTED)),
                Span::styled("[L]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
                Span::styled(" Loop  ", Style::default().fg(FG_MUTED)),
                Span::styled("[R]", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
                Span::styled(" Shuffle  ", Style::default().fg(FG_MUTED)),
                Span::styled("[Space]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
                Span::styled(" Auto  ", Style::default().fg(FG_MUTED)),
                Span::styled("[C]", Style::default().fg(Color::Rgb(255, 140, 80)).add_modifier(Modifier::BOLD)),
                Span::styled(" Clear  ", Style::default().fg(FG_MUTED)),
                Span::styled("[Tab]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
                Span::styled(" Playlists  ", Style::default().fg(FG_MUTED)),
                Span::styled("[Esc/Q]", Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD)),
                Span::styled(" Close", Style::default().fg(FG_MUTED)),
            ]),
        )
    } else if state.playlist_focus_items {
        (
            Line::from(vec![
                Span::styled(" [J/K]", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                Span::styled(" Move Item  ", Style::default().fg(FG_MUTED)),
                Span::styled("[d]", Style::default().fg(Color::Rgb(255, 85, 85)).add_modifier(Modifier::BOLD)),
                Span::styled(" Remove  ", Style::default().fg(FG_MUTED)),
                Span::styled("[a]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
                Span::styled(" Audio  ", Style::default().fg(FG_MUTED)),
                Span::styled("[v]", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
                Span::styled(" Qual  ", Style::default().fg(FG_MUTED)),
                Span::styled("[l]", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Sub  ", Style::default().fg(FG_MUTED)),
                Span::styled("[Enter]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
                Span::styled(" Play Playlist", Style::default().fg(FG_MUTED)),
            ]),
            Line::from(vec![
                Span::styled(" [Esc/e]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
                Span::styled(" Done Editing Items  ", Style::default().fg(FG_MUTED)),
                Span::styled("[Tab]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
                Span::styled(" Queue Pane  ", Style::default().fg(FG_MUTED)),
                Span::styled("[Q]", Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD)),
                Span::styled(" Close", Style::default().fg(FG_MUTED)),
            ]),
        )
    } else {
        (
            Line::from(vec![
                Span::styled(" [Enter]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
                Span::styled(" Load & Play  ", Style::default().fg(FG_MUTED)),
                Span::styled("[e]", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                Span::styled(" Edit Items  ", Style::default().fg(FG_MUTED)),
                Span::styled("[a]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
                Span::styled(" Append  ", Style::default().fg(FG_MUTED)),
                Span::styled("[w]", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
                Span::styled(" Export  ", Style::default().fg(FG_MUTED)),
                Span::styled("[i]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
                Span::styled(" Import  ", Style::default().fg(FG_MUTED)),
                Span::styled("[x]", Style::default().fg(Color::Rgb(255, 85, 85)).add_modifier(Modifier::BOLD)),
                Span::styled(" Delete", Style::default().fg(FG_MUTED)),
            ]),
            Line::from(vec![
                Span::styled(" [Tab]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
                Span::styled(" Queue Pane  ", Style::default().fg(FG_MUTED)),
                Span::styled("[C]", Style::default().fg(Color::Rgb(255, 140, 80)).add_modifier(Modifier::BOLD)),
                Span::styled(" Clear Queue  ", Style::default().fg(FG_MUTED)),
                Span::styled("[Esc/Q]", Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD)),
                Span::styled(" Close", Style::default().fg(FG_MUTED)),
            ]),
        )
    };
    frame.render_widget(Paragraph::new(vec![line1, line2]), area);
}

/// Compact Detail-screen Batch Quick-Queue Mini-Modal (`[E]`).
pub fn render_batch_queue_modal(frame: &mut Frame, state: &AppState, cfg: &Config) {
    let area = centered_rect(64, 52, frame.area());
    frame.render_widget(Clear, area);

    let theme = state.theme();
    let title = state
        .selected_anime
        .as_ref()
        .map(|a| a.display_title())
        .unwrap_or("Anime");
    let ep = state.selected_episode.unwrap_or(1);
    let max_aired = state
        .selected_anime
        .as_ref()
        .map(|a| a.aired_episodes())
        .unwrap_or(1)
        .max(1);
    let e3 = (ep + 2).min(max_aired);
    let remaining_count = max_aired.saturating_sub(ep) + 1;
    let parsed_preview = parse_batch_queue_input(&state.batch_queue_range, max_aired);
    let tail_pos = state.queue.len() + 1;
    let audio_tag = if state.batch_queue_dub { "DUB" } else { "SUB" };

    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(" ◈ ADD TO QUEUE: ", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{} (Ep {}) ", clamp_text(title, 32), ep),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(BG_MODAL));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if inner.width < 20 || inner.height < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Option 0: Queue Next
            Constraint::Length(2), // Option 1: Append Tail
            Constraint::Length(2), // Option 2: Next 3 Episodes
            Constraint::Length(2), // Option 3: Rest of Season
            Constraint::Length(3), // Option 4: Custom Range
            Constraint::Length(2), // Active presets summary
            Constraint::Min(2),    // Footer hints
        ])
        .split(inner.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }));

    let options = [
        (0usize, format!("[1] Queue Next (Ep {})", ep), "Insert at front (plays right after current)".to_string()),
        (1usize, format!("[2] Append to Queue (Ep {})", ep), format!("Add at tail (#{} in active session)", tail_pos)),
        (2usize, format!("[3] Next 3 Episodes ({}..{})", ep, e3), format!("Batch append ({} · {})", audio_tag, state.batch_queue_quality)),
        (3usize, format!("[4] Rest of Season ({}..{})", ep, max_aired), format!("Append {} remaining episode(s)", remaining_count)),
    ];

    for (idx, label, desc) in options {
        let sel = state.batch_queue_mode == idx;
        let bg = if sel { BG_CARD_ACTIVE } else { BG_MODAL };
        let line = Line::from(vec![
            Span::styled(if sel { " ▶ " } else { "   " }, Style::default().fg(theme.primary).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(label, Style::default().fg(Color::White).bg(bg).add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() })),
            Span::styled("  ->  ", Style::default().fg(theme.divider).bg(bg)),
            Span::styled(desc, Style::default().fg(FG_MUTED).bg(bg)),
        ]);
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(bg)), chunks[idx]);
    }

    let range_sel = state.batch_queue_mode == 4;
    let range_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if range_sel { EMERALD } else { theme.border_idle }))
        .style(Style::default().bg(BG_CARD));
    let r_inner = range_block.inner(chunks[4]);
    frame.render_widget(range_block, chunks[4]);
    let range_line = Line::from(vec![
        Span::styled(" [r] Custom Range: ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(
            if range_sel {
                format!("{}_", state.batch_queue_range)
            } else {
                state.batch_queue_range.clone()
            },
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("   ({} ep(s) matched · max {})", parsed_preview.len(), max_aired),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(range_line).style(Style::default().bg(BG_CARD)), r_inner);

    let _ = cfg;
    let dub_note = match state.dub_available {
        Some(true) => "Dub ready",
        Some(false) => "Sub only",
        None => "Auto-detect",
    };
    let settings_line = Line::from(vec![
        Span::styled(" Settings: ", Style::default().fg(FG_MUTED)),
        Span::styled(format!("[{}]", audio_tag), Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
        Span::styled(" · ", Style::default().fg(theme.divider)),
        Span::styled(format!("[{}]", state.batch_queue_quality), Style::default().fg(CYAN)),
        Span::styled(" · ", Style::default().fg(theme.divider)),
        Span::styled(format!("[{}]", state.batch_queue_sub_lang), Style::default().fg(theme.accent)),
        Span::styled(format!("   ({})", dub_note), Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(settings_line), chunks[5]);

    let hints = Line::from(vec![
        Span::styled(" [Enter]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
        Span::styled(" Queue  ", Style::default().fg(FG_MUTED)),
        Span::styled("[1-4]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
        Span::styled(" Run  ", Style::default().fg(FG_MUTED)),
        Span::styled("[r]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(" Range  ", Style::default().fg(FG_MUTED)),
        Span::styled("[Tab]", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
        Span::styled(" Sub/Dub  ", Style::default().fg(FG_MUTED)),
        Span::styled("[v]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(" Qual  ", Style::default().fg(FG_MUTED)),
        Span::styled("[l]", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(" Sub  ", Style::default().fg(FG_MUTED)),
        Span::styled("[Esc]", Style::default().fg(theme.text_dim).add_modifier(Modifier::BOLD)),
        Span::styled(" Cancel", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(hints).alignment(Alignment::Left), chunks[6]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::cache::Anime, services::queue::QueueItem};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_clamp_text_prevents_overflow_and_preserves_ellipsis() {
        assert_eq!(clamp_text("Frieren", 10), "Frieren");
        let clamped = clamp_text("Sousou no Frieren Beyond Journeys End", 14);
        assert!(clamped.chars().count() <= 14);
        assert_eq!(clamped, "Sousou no…");
    }

    #[test]
    fn test_render_queue_overlay_and_batch_modal_clean_layout() {
        let mut state = AppState::new();
        let mut anime = Anime::default();
        anime.id = 154587;
        anime.title_english = Some("Sousou no Frieren".to_string());
        anime.episodes = Some(28);
        anime.season_year = Some(2023);
        state.selected_anime = Some(anime);
        state.selected_episode = Some(12);
        state.queue.enqueue_back(QueueItem::new(
            154587,
            Some(52991),
            Some(2023),
            "Sousou no Frieren".to_string(),
            "Sousou no Frieren".to_string(),
            13,
            "1080p".to_string(),
            false,
            "English".to_string(),
            "mpv".to_string(),
        ));

        for (w, h) in [(80u16, 24u16), (124u16, 38u16)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| render_overlay(f, &state)).unwrap();
            let buf = terminal.backend().buffer();
            let mut rendered = String::new();
            for y in 0..h {
                for x in 0..w {
                    rendered.push_str(buf[(x, y)].symbol());
                }
                rendered.push('\n');
            }
            for ch in rendered.chars() {
                let cp = ch as u32;
                assert!(
                    !(0x1F300..=0x1FAFF).contains(&cp),
                    "unexpected emoji char {ch:?} in queue overlay"
                );
            }
            assert!(rendered.contains("SESSION QUEUE (1)"));
            assert!(rendered.contains("[QUEUED ]"));
            assert!(!rendered.contains("Ep 13[QUEUED ]"));

            state.open_batch_queue_modal(false);
            terminal
                .draw(|f| render_batch_queue_modal(f, &state, &Config::default()))
                .unwrap();
            let batch_buf = terminal.backend().buffer();
            let mut batch_txt = String::new();
            for y in 0..h {
                for x in 0..w {
                    batch_txt.push_str(batch_buf[(x, y)].symbol());
                }
                batch_txt.push('\n');
            }
            assert!(batch_txt.contains("ADD TO QUEUE:"));
            assert!(batch_txt.contains("Queue Next (Ep 12)"));
        }
    }
}

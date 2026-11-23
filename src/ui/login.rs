//! Connected Trackers & Sync Hub overlay (`[L]`).
//!
//! Redesigned dark Bento modal unifying AniList and MyAnimeList:
//!   - `[1] Live Trackers`: side-by-side provider cards with Browser OAuth2, Public Username
//!     Sync, Client ID setup, Token fallback, and Unified Telemetry bar.
//!   - `[2] Merge & Conflicts`: MergePolicy selector, auto-scrobble toggle, cross-tracker diff
//!     audit table, and 1-key `[P]` Push Parity reconciliation.
//!   - `[3] Archives & Vault`: Local JSON + official MAL `.xml` / `.xml.gz` auto-scanner,
//!     ingestion telemetry, and unified Solus library backup exporter.

use crate::config::{Config, TrackerAccount, TrackerProvider};
use crate::state::{AccountInputMode, AppState};
use crate::ui::theme::ThemePalette;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

const BG_DARK: Color = Color::Rgb(15, 16, 25);
const CARD_BG: Color = Color::Rgb(21, 23, 36);
const HEADER_BG: Color = Color::Rgb(24, 26, 40);
const EMERALD: Color = Color::Rgb(80, 250, 123);
const AMBER: Color = Color::Rgb(251, 191, 36);
const CYAN: Color = Color::Rgb(103, 232, 249);
const MAL_BLUE: Color = Color::Rgb(108, 160, 255);
const ROSE: Color = Color::Rgb(255, 105, 135);
const MUTED: Color = Color::Rgb(145, 152, 182);

/// Responsive centered modal percentages so `active_modal_rect` cleanly prevents cover bleed.
pub fn modal_pct(area: Rect) -> (u16, u16) {
    if area.width >= 112 && area.height >= 30 {
        (86, 84)
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

fn format_last_sync(ts: Option<i64>) -> String {
    match ts {
        Some(epoch) if epoch > 0 => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let secs = (now - epoch).max(0);
            if secs < 60 {
                "just now".to_string()
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86_400 {
                format!("{}h ago", secs / 3600)
            } else {
                format!("{}d ago", secs / 86_400)
            }
        }
        _ => "Never".to_string(),
    }
}

/// Render the `[L]` Connected Trackers & Sync Hub modal.
pub fn render_overlay(frame: &mut Frame, state: &AppState, cfg: &Config) {
    let area = frame.area();
    let theme = state.theme();
    let (pct_x, pct_y) = modal_pct(area);
    let modal = centered_rect(pct_x, pct_y, area);

    frame.render_widget(Clear, modal);
    frame.render_widget(Block::default().style(Style::default().bg(BG_DARK)), modal);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(BG_DARK))
        .title(Span::styled(
            " ✦ CONNECTED TRACKERS & SYNC HUB ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ))
        .title(
            Line::from(vec![Span::styled(
                status_chip_text(state),
                Style::default()
                    .fg(if state.auth.is_authenticated() { EMERALD } else { AMBER })
                    .add_modifier(Modifier::BOLD),
            )])
            .alignment(Alignment::Right),
        );

    let inner = outer.inner(modal);
    frame.render_widget(outer, modal);

    if inner.width < 28 || inner.height < 12 {
        return;
    }

    let status_h = if state.login_status.is_some() { 2 } else { 0 };
    let input_h =
        if state.oauth_pending.is_some() || state.account_input_mode != AccountInputMode::None {
            5
        } else {
            0
        };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),        // Sub-navigation Bento tab bar
            Constraint::Min(6),           // Active view body
            Constraint::Length(input_h),  // Inline OAuth wait or Typing input drawer
            Constraint::Length(status_h), // Live feedback banner
            Constraint::Length(2),        // Dynamic keycap footer bar
        ])
        .split(inner);

    render_tab_header(frame, chunks[0], state, &theme);

    match state.account_sync_tab {
        0 => render_live_trackers_view(frame, chunks[1], state, cfg, &theme),
        1 => render_merge_conflicts_view(frame, chunks[1], state, &theme),
        _ => render_archives_vault_view(frame, chunks[1], state, &theme),
    }

    if input_h > 0 {
        render_action_drawer(frame, chunks[2], state, &theme);
    }

    if let Some(ref msg) = state.login_status {
        let color = if msg.starts_with('✓')
            || msg.contains("Connected")
            || msg.contains("Synced")
            || msg.contains("Imported")
            || msg.contains("Exported")
        {
            EMERALD
        } else if msg.starts_with('!')
            || msg.contains("Failed")
            || msg.contains("Error")
            || msg.contains("Unavailable")
        {
            ROSE
        } else {
            CYAN
        };
        let status_block = Paragraph::new(Line::from(vec![
            Span::styled(" ◈ ", Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(
                msg.as_str(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(HEADER_BG))
        .wrap(Wrap { trim: true });
        frame.render_widget(status_block, chunks[3]);
    }

    render_footer(frame, chunks[4], state, &theme);
}

fn status_chip_text(state: &AppState) -> String {
    match (state.auth.has_anilist(), state.auth.has_mal()) {
        (true, true) => " ✦ AniList + ◆ MAL Connected ".to_string(),
        (true, false) => " ✦ AniList Connected ".to_string(),
        (false, true) => " ◆ MAL Connected ".to_string(),
        (false, false) => " ○ Offline Vault Mode ".to_string(),
    }
}

fn render_tab_header(frame: &mut Frame, area: Rect, state: &AppState, theme: &ThemePalette) {
    let conflict_cnt = state
        .merge_report
        .as_ref()
        .map(|r| r.conflict_diffs.len())
        .unwrap_or(0);

    let tab2_label = if conflict_cnt > 0 {
        format!(" [2] Merge & Conflicts ({}) ", conflict_cnt)
    } else {
        " [2] Merge & Conflicts ".to_string()
    };

    let tabs = [
        (0usize, " [1] Live Trackers ".to_string()),
        (1usize, tab2_label),
        (2usize, " [3] Archives & Vault ".to_string()),
    ];

    let mut spans = vec![Span::raw(" ")];
    for (idx, label) in tabs {
        let active = state.account_sync_tab == idx;
        let style = if active {
            Style::default()
                .fg(Color::Black)
                .bg(theme.secondary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(MUTED)
                .bg(HEADER_BG)
                .add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("  "));
    }

    let policy_badge = format!("⚖ {} ", state.auth.merge_policy.label());
    let used_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let right_len = policy_badge.chars().count() + 1;
    let pad = (area.width as usize).saturating_sub(used_len + right_len);
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(
            policy_badge,
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        ));
    }

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider))
        .style(Style::default().bg(BG_DARK));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn render_live_trackers_view(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    cfg: &Config,
    theme: &ThemePalette,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
        .split(area);

    let wide = area.width >= 88;
    let cards = Layout::default()
        .direction(if wide {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    let al_count = state.merge_report.as_ref().map(|r| r.anilist_count);
    let mal_count = state.merge_report.as_ref().map(|r| r.mal_count);

    render_provider_card(
        frame,
        cards[0],
        TrackerProvider::AniList,
        &state.auth.anilist,
        cfg.trackers.resolved_anilist_client_id().is_some(),
        al_count,
        state.account_focus_provider == TrackerProvider::AniList,
        theme,
    );

    render_provider_card(
        frame,
        cards[1],
        TrackerProvider::Mal,
        &state.auth.mal,
        cfg.trackers.custom_mal_client_id().is_some(),
        mal_count,
        state.account_focus_provider == TrackerProvider::Mal,
        theme,
    );

    let report = state.merge_report.as_ref();
    let unique_cnt = report.map(|r| r.unified_unique_count).unwrap_or(0);
    let overlap_cnt = report.map(|r| r.overlaps_count).unwrap_or(0);
    let conflict_cnt = report.map(|r| r.conflict_diffs.len()).unwrap_or(0);
    let scrobble_flag = if state.auth.auto_scrobble_both {
        ("ON (AL + MAL)", EMERALD)
    } else {
        ("AniList Only", AMBER)
    };

    let telemetry_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_idle))
        .style(Style::default().bg(CARD_BG))
        .title(Span::styled(
            " ◈ UNIFIED LIBRARY TELEMETRY ",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let t_inner = telemetry_block.inner(chunks[1]);
    frame.render_widget(telemetry_block, chunks[1]);

    let t_lines = vec![
        Line::from(vec![
            Span::styled(" Catalog: ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{unique_cnt} unique shows"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.divider)),
            Span::styled(format!("{overlap_cnt} linked AL↔MAL"), Style::default().fg(CYAN)),
            Span::styled("  ·  ", Style::default().fg(theme.divider)),
            Span::styled(
                "0 duplicates",
                Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.divider)),
            Span::styled(
                if conflict_cnt > 0 {
                    format!("{conflict_cnt} diffs (see [2])")
                } else {
                    "Parity OK".to_string()
                },
                Style::default().fg(if conflict_cnt > 0 { AMBER } else { EMERALD }),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Auto-Scrobble Both: ", Style::default().fg(MUTED)),
            Span::styled(
                scrobble_flag.0,
                Style::default()
                    .fg(scrobble_flag.1)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ([a])", Style::default().fg(MUTED)),
            Span::styled("    Merge Rule: ", Style::default().fg(MUTED)),
            Span::styled(
                state.auth.merge_policy.label(),
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ([m])", Style::default().fg(MUTED)),
        ]),
    ];
    frame.render_widget(Paragraph::new(t_lines), t_inner);
}

#[allow(clippy::too_many_arguments)]
fn render_provider_card(
    frame: &mut Frame,
    area: Rect,
    provider: TrackerProvider,
    account: &TrackerAccount,
    has_client_id: bool,
    synced_count: Option<usize>,
    focused: bool,
    theme: &ThemePalette,
) {
    let accent = match provider {
        TrackerProvider::AniList => CYAN,
        TrackerProvider::Mal => MAL_BLUE,
    };
    let title_str = match provider {
        TrackerProvider::AniList => " ✦ ANILIST ",
        TrackerProvider::Mal => " ◆ MYANIMELIST ",
    };

    let (status_badge, badge_color) = if account.has_write_token() {
        ("[● OAUTH CONNECTED]", EMERALD)
    } else if account.username.is_some() {
        ("[◔ PUBLIC USERNAME]", CYAN)
    } else if account.last_imported_file.is_some() {
        ("[◒ ARCHIVE IMPORTED]", AMBER)
    } else {
        ("[○ NOT CONNECTED]", MUTED)
    };

    let border_color = if focused { accent } else { theme.border_idle };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(CARD_BG))
        .title(Span::styled(
            title_str,
            Style::default()
                .fg(if focused { Color::Black } else { accent })
                .bg(if focused { accent } else { CARD_BG })
                .add_modifier(Modifier::BOLD),
        ));

    if focused {
        block = block.title(
            Line::from(vec![Span::styled(
                " ◆ ACTIVE ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )])
            .alignment(Alignment::Right),
        );
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let handle = account
        .username
        .as_deref()
        .map(|u| format!("@{u}"))
        .unwrap_or_else(|| "No account linked".to_string());
    let mode_str = account.sync_mode.as_deref().unwrap_or("Offline");
    let write_cap = if account.has_write_token() {
        ("LIVE Two-Way Database Sync ✓", EMERALD)
    } else if account.username.is_some() {
        match provider {
            TrackerProvider::Mal => ("Read-Only (Press [p] Password Login for 2-Way Sync)", AMBER),
            TrackerProvider::AniList => ("Read-Only (Connect [o] OAuth for Live Writeback)", AMBER),
        }
    } else {
        match provider {
            TrackerProvider::Mal => ("Idle — Press [p] Password Login or [u] Username", MUTED),
            TrackerProvider::AniList => ("Idle — Link via [o] Browser or [u] Username", MUTED),
        }
    };
    let cid_status = match provider {
        TrackerProvider::Mal => {
            if has_client_id {
                ("Custom OAuth ID Configured ✓", EMERALD)
            } else {
                ("Built-In ✓ (Ready — No Client ID needed)", EMERALD)
            }
        }
        TrackerProvider::AniList => {
            if has_client_id {
                ("Configured ✓", EMERALD)
            } else {
                ("Optional ([k] Client ID / [t] Token)", MUTED)
            }
        }
    };
    let synced_str = synced_count
        .map(|c| format!("{c} entries"))
        .unwrap_or_else(|| "—".to_string());

    let (action_row_1, action_row_2) = match provider {
        TrackerProvider::Mal => (
            Line::from(vec![
                Span::styled(" [p]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
                Span::styled(" Password Login  ", Style::default().fg(Color::White)),
                Span::styled("[u]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Username  ", Style::default().fg(Color::White)),
                Span::styled("[o]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Browser", Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(" [r]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Refresh         ", Style::default().fg(Color::White)),
                Span::styled("[t]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Token     ", Style::default().fg(Color::White)),
                Span::styled("[x]", Style::default().fg(ROSE).add_modifier(Modifier::BOLD)),
                Span::styled(" Unlink", Style::default().fg(ROSE)),
            ]),
        ),
        TrackerProvider::AniList => (
            Line::from(vec![
                Span::styled(" [o]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Browser OAuth  ", Style::default().fg(Color::White)),
                Span::styled("[u]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Username  ", Style::default().fg(Color::White)),
                Span::styled("[t]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Paste Token", Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(" [k]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Client ID     ", Style::default().fg(Color::White)),
                Span::styled("[r]", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Refresh   ", Style::default().fg(Color::White)),
                Span::styled("[x]", Style::default().fg(ROSE).add_modifier(Modifier::BOLD)),
                Span::styled(" Unlink", Style::default().fg(ROSE)),
            ]),
        ),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(" Status:   ", Style::default().fg(MUTED)),
            Span::styled(
                status_badge,
                Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Handle:   ", Style::default().fg(MUTED)),
            Span::styled(
                handle,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  ({mode_str})"), Style::default().fg(MUTED)),
        ]),
        Line::from(vec![
            Span::styled(" Write:    ", Style::default().fg(MUTED)),
            Span::styled(write_cap.0, Style::default().fg(write_cap.1)),
        ]),
        Line::from(vec![
            Span::styled(" OAuth ID: ", Style::default().fg(MUTED)),
            Span::styled(cid_status.0, Style::default().fg(cid_status.1)),
        ]),
        Line::from(vec![
            Span::styled(" Snapshot: ", Style::default().fg(MUTED)),
            Span::styled(synced_str, Style::default().fg(Color::White)),
            Span::styled("  ·  Synced ", Style::default().fg(MUTED)),
            Span::styled(format_last_sync(account.last_sync_time), Style::default().fg(CYAN)),
        ]),
        Line::from(""),
        action_row_1,
        action_row_2,
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_merge_conflicts_view(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    theme: &ThemePalette,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(6)])
        .split(area);

    let policy_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_idle))
        .style(Style::default().bg(CARD_BG))
        .title(Span::styled(
            " ⚖ RECONCILIATION STRATEGY & PARITY ",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        ));
    let p_inner = policy_block.inner(chunks[0]);
    frame.render_widget(policy_block, chunks[0]);

    let p_lines = vec![
        Line::from(vec![
            Span::styled(" Active Merge Rule: ", Style::default().fg(MUTED)),
            Span::styled(
                state.auth.merge_policy.label(),
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ([m] cycle)", Style::default().fg(MUTED)),
            Span::styled("    Scrobble Mode: ", Style::default().fg(MUTED)),
            Span::styled(
                if state.auth.auto_scrobble_both {
                    "Dual AL+MAL ✓"
                } else {
                    "Primary Only"
                },
                Style::default().fg(if state.auth.auto_scrobble_both {
                    EMERALD
                } else {
                    MUTED
                }),
            ),
            Span::styled("  ([a])", Style::default().fg(MUTED)),
        ]),
        Line::from(vec![
            Span::styled(" Actions: ", Style::default().fg(MUTED)),
            Span::styled("[P]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
            Span::styled(
                " Push Reconciled Parity Updates to Lagging Website(s)   ",
                Style::default().fg(Color::White),
            ),
            Span::styled("[r]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(" Re-audit Both Lists", Style::default().fg(Color::White)),
        ]),
    ];
    frame.render_widget(Paragraph::new(p_lines), p_inner);

    let diff_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(CARD_BG))
        .title(Span::styled(
            " CROSS-TRACKER DIFF AUDIT ",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let d_inner = diff_block.inner(chunks[1]);
    frame.render_widget(diff_block, chunks[1]);

    let diffs = state
        .merge_report
        .as_ref()
        .map(|r| r.conflict_diffs.as_slice())
        .unwrap_or(&[]);

    if diffs.is_empty() {
        let empty = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "   ✓ 100% Parity Achieved ",
                    Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "— Zero conflicting shows between AniList & MyAnimeList.",
                    Style::default().fg(MUTED),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "   Tip: Any show linked across AniList and MAL with mismatched progress or list status",
                Style::default().fg(MUTED),
            )]),
            Line::from(vec![Span::styled(
                "   will appear here so you can push the winning state in 1 keypress ([P]).",
                Style::default().fg(MUTED),
            )]),
        ];
        frame.render_widget(Paragraph::new(empty).wrap(Wrap { trim: true }), d_inner);
        return;
    }

    let max_rows = d_inner.height.saturating_sub(2) as usize;
    let cursor = state.merge_diff_cursor.min(diffs.len().saturating_sub(1));
    let start = if cursor >= max_rows && max_rows > 0 {
        cursor + 1 - max_rows
    } else {
        0
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "  TITLE                          ",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "AL            ",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "MAL           ",
            Style::default().fg(MAL_BLUE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "RESOLVED WINNER",
            Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
        ),
    ])];

    for (idx, d) in diffs.iter().enumerate().skip(start).take(max_rows.max(1)) {
        let sel = idx == cursor;
        let marker = if sel { "▶ " } else { "  " };
        let al_str = format!(
            "{} E{}",
            d.al_status
                .as_deref()
                .unwrap_or("-")
                .chars()
                .take(4)
                .collect::<String>(),
            d.al_progress.unwrap_or(0)
        );
        let mal_str = format!(
            "{} E{}",
            d.mal_status
                .as_deref()
                .unwrap_or("-")
                .chars()
                .take(4)
                .collect::<String>(),
            d.mal_progress.unwrap_or(0)
        );
        let push_tag = match (d.needs_push_to_al, d.needs_push_to_mal) {
            (true, false) => "→ Push AL",
            (false, true) => "→ Push MAL",
            (true, true) => "→ Push Both",
            (false, false) => "Parity",
        };
        let title_cut: String = d.title.chars().take(28).collect();
        let row_bg = if sel { HEADER_BG } else { CARD_BG };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{marker}{:<30} ", title_cut),
                Style::default()
                    .fg(if sel {
                        Color::White
                    } else {
                        Color::Rgb(220, 224, 240)
                    })
                    .bg(row_bg)
                    .add_modifier(if sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(
                format!("{:<14}", al_str),
                Style::default().fg(CYAN).bg(row_bg),
            ),
            Span::styled(
                format!("{:<14}", mal_str),
                Style::default().fg(MAL_BLUE).bg(row_bg),
            ),
            Span::styled(
                format!(
                    "{} E{} ({})",
                    d.resolved_status, d.resolved_progress, push_tag
                ),
                Style::default()
                    .fg(EMERALD)
                    .bg(row_bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), d_inner);
}

fn render_archives_vault_view(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    theme: &ThemePalette,
) {
    let cols = Layout::default()
        .direction(if area.width >= 88 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let import_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(CARD_BG))
        .title(Span::styled(
            " IMPORT QUEUE (JSON & MAL .XML / .XML.GZ) ",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let i_inner = import_block.inner(cols[0]);
    frame.render_widget(import_block, cols[0]);

    if state.detected_export_files.is_empty() {
        let empty = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No library exports detected in ~/Downloads or ~",
                Style::default().fg(MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Supported formats:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "   • MAL Official Export: animelist_*.xml or .xml.gz",
                Style::default().fg(MAL_BLUE),
            )),
            Line::from(Span::styled(
                "   • Solus / AniList JSON: solus_backup_*.json / anilist*.json",
                Style::default().fg(CYAN),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Drop an export in ~/Downloads and press [s] to rescan.",
                Style::default().fg(AMBER),
            )),
        ];
        frame.render_widget(Paragraph::new(empty).wrap(Wrap { trim: true }), i_inner);
    } else {
        let mut lines = Vec::new();
        for (idx, path) in state.detected_export_files.iter().enumerate() {
            let sel = idx == state.detected_file_cursor;
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("export");
            let lower = name.to_lowercase();
            let (tag, tag_col) = if lower.ends_with(".xml.gz") {
                ("[MAL .GZ]", MAL_BLUE)
            } else if lower.ends_with(".xml") {
                ("[MAL XML]", MAL_BLUE)
            } else {
                ("[JSON]", CYAN)
            };
            let bg = if sel { HEADER_BG } else { CARD_BG };
            lines.push(Line::from(vec![
                Span::styled(
                    if sel { " ▶ " } else { "   " },
                    Style::default()
                        .fg(theme.primary)
                        .bg(bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<9} ", tag),
                    Style::default().fg(tag_col).bg(bg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    name,
                    Style::default()
                        .fg(if sel {
                            Color::White
                        } else {
                            Color::Rgb(210, 214, 232)
                        })
                        .bg(bg),
                ),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                " [Enter]",
                Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Ingest & Merge   ", Style::default().fg(Color::White)),
            Span::styled("[s]", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(" Rescan ~/Downloads", Style::default().fg(Color::White)),
        ]));
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), i_inner);
    }

    let vault_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_idle))
        .style(Style::default().bg(CARD_BG))
        .title(Span::styled(
            " BACKUP VAULT & STATS ",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        ));
    let v_inner = vault_block.inner(cols[1]);
    frame.render_widget(vault_block, cols[1]);

    let mut v_lines = vec![
        Line::from(Span::styled(
            " Solus Unified Snapshot Backup",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " Exports Watchlist + Continue Watching + AniList ID + MAL ID.",
            Style::default().fg(MUTED),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [e]", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
            Span::styled(
                " Export Library to ~/Downloads/solus_backup_*.json",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
    ];

    if let Some(ref st) = state.import_stats {
        v_lines.push(Line::from(Span::styled(
            " Last Import Summary:",
            Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
        )));
        v_lines.push(Line::from(format!(
            "  Total: {}  Watching: {}  Planning: {}",
            st.total_anime, st.watching_count, st.planning_count
        )));
        v_lines.push(Line::from(format!(
            "  Completed: {}  Episodes Logged: {}",
            st.completed_count, st.episodes_logged
        )));
    }

    frame.render_widget(Paragraph::new(v_lines).wrap(Wrap { trim: true }), v_inner);
}

fn render_action_drawer(frame: &mut Frame, area: Rect, state: &AppState, _theme: &ThemePalette) {
    if let Some(provider) = state.oauth_pending {
        let accent = if provider == TrackerProvider::Mal {
            MAL_BLUE
        } else {
            CYAN
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .style(Style::default().bg(HEADER_BG))
            .title(Span::styled(
                format!(
                    " ⟳ AWAITING {} BROWSER AUTHORIZATION ",
                    provider.display_name().to_uppercase()
                ),
                Style::default()
                    .fg(Color::Black)
                    .bg(accent)
                    .add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let lines = vec![
            Line::from(vec![
                Span::styled(" Callback listening on ", Style::default().fg(MUTED)),
                Span::styled(
                    "http://127.0.0.1:41932",
                    Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " — Complete sign-in in your browser tab.",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled(" SSH / Headless? Press ", Style::default().fg(MUTED)),
                Span::styled("[t]", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
                Span::styled(" to paste token manually, or ", Style::default().fg(MUTED)),
                Span::styled("[Esc]", Style::default().fg(ROSE).add_modifier(Modifier::BOLD)),
                Span::styled(" to cancel.", Style::default().fg(MUTED)),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        return;
    }

    let (title, hint, accent) = match state.account_input_mode {
        AccountInputMode::PublicUsername(p) => (
            format!(" @{} PUBLIC USERNAME SYNC ", p.display_name().to_uppercase()),
            "Type username & press [Enter] to pull anime list immediately ([Esc] cancel)".to_string(),
            if p == TrackerProvider::Mal {
                MAL_BLUE
            } else {
                CYAN
            },
        ),
        AccountInputMode::ManualToken(p) => (
            format!(" {} ACCESS TOKEN PASTE ", p.display_name().to_uppercase()),
            "Paste OAuth access token & press [Enter] for full live writeback ([Esc] cancel)"
                .to_string(),
            AMBER,
        ),
        AccountInputMode::ClientId(p) => {
            let redirect_hint = if p == TrackerProvider::Mal {
                "myanimelist.net/apiconfig • Redirect URI: http://127.0.0.1:41932/mal/callback"
            } else {
                "anilist.co/settings/developer • Redirect URI: http://127.0.0.1:41932/anilist/callback"
            };
            (
                format!(" SET {} OAUTH CLIENT ID ", p.display_name().to_uppercase()),
                format!("{redirect_hint} ([Enter] Save, [Esc] Cancel)"),
                if p == TrackerProvider::Mal {
                    MAL_BLUE
                } else {
                    CYAN
                },
            )
        }
        AccountInputMode::MalPasswordLoginUsername => (
            " ◆ MYANIMELIST DIRECT LOGIN (STEP 1/2: USERNAME) ".to_string(),
            "Type your MyAnimeList username & press [Enter] to continue ([Esc] cancel)".to_string(),
            MAL_BLUE,
        ),
        AccountInputMode::MalPasswordLoginPassword => (
            format!(
                " ◆ MYANIMELIST DIRECT LOGIN (STEP 2/2: PASSWORD FOR @{}) ",
                state.pending_mal_login_username
            ),
            "Type your MAL password & press [Enter] — sent via HTTPS for tokens, never saved to disk ([Esc] cancel)"
                .to_string(),
            EMERALD,
        ),
        AccountInputMode::None => return,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(HEADER_BG))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let masked = if matches!(state.account_input_mode, AccountInputMode::MalPasswordLoginPassword) {
        "•".repeat(state.account_text_input.chars().count())
    } else if matches!(state.account_input_mode, AccountInputMode::ManualToken(_))
        && state.account_text_input.len() > 12
    {
        format!(
            "{}••••••{}",
            &state.account_text_input[..6],
            &state.account_text_input[state.account_text_input.len() - 4..]
        )
    } else {
        state.account_text_input.clone()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(" > ", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::styled(
                masked,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(accent)),
        ]),
        Line::from(Span::styled(format!(" {hint}"), Style::default().fg(MUTED))),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState, theme: &ThemePalette) {
    let hints: Vec<(&str, &str)> = if state.account_input_mode != AccountInputMode::None {
        vec![("Enter", "Submit"), ("Esc", "Cancel Input")]
    } else if state.oauth_pending.is_some() {
        vec![("t", "Manual Token Fallback"), ("Esc", "Cancel OAuth")]
    } else {
        match state.account_sync_tab {
            0 => vec![
                ("Tab/←→", "Focus AL/MAL"),
                ("p", "MAL Password"),
                ("u", "Username"),
                ("o", "Browser OAuth"),
                ("m", "Policy"),
                ("a", "Auto-Both"),
                ("r", "Refresh"),
                ("Esc", "Close"),
            ],
            1 => vec![
                ("↑↓", "Select Diff"),
                ("P", "Push Parity"),
                ("m", "Cycle Rule"),
                ("a", "Auto-Both"),
                ("r", "Re-audit"),
                ("1/3", "Switch View"),
                ("Esc", "Close"),
            ],
            _ => vec![
                ("↑↓", "Select File"),
                ("Enter", "Import File"),
                ("s", "Rescan"),
                ("e", "Export Backup"),
                ("1/2", "Switch View"),
                ("Esc", "Close"),
            ],
        }
    };

    let mut spans = vec![Span::raw(" ")];
    for (i, (k, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(theme.divider)));
        }
        spans.push(Span::styled(
            format!("[{k}]"),
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {desc}"), Style::default().fg(MUTED)));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(BG_DARK))
            .wrap(Wrap { trim: true }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_login_hub_renders_dual_trackers_cleanly() {
        let mut state = AppState::new();
        state.auth.anilist.username = Some("al_user".into());
        state.auth.anilist.token = Some("tok".into());
        state.auth.mal.username = Some("mal_user".into());

        let cfg = Config::default();
        let backend = TestBackend::new(124, 38);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_overlay(f, &state, &cfg))
            .expect("login overlay should render cleanly");

        let buf = terminal.backend().buffer();
        let mut screen = String::new();
        for y in 0..38 {
            for x in 0..124 {
                screen.push_str(buf[(x, y)].symbol());
            }
            screen.push('\n');
        }
        assert!(screen.contains("CONNECTED TRACKERS & SYNC HUB"));
        assert!(screen.contains("ANILIST"));
        assert!(screen.contains("MYANIMELIST"));
        assert!(screen.contains("@al_user"));
        assert!(screen.contains("@mal_user"));
        assert!(screen.contains("0 duplicates"));
    }

    #[test]
    fn test_login_hub_compact_viewport_renders_all_views() {
        let cfg = Config::default();
        for tab in [0usize, 1, 2] {
            let mut state = AppState::new();
            state.account_sync_tab = tab;
            let backend = TestBackend::new(76, 24);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| render_overlay(f, &state, &cfg))
                .expect("compact hub should render without panic");
        }
    }

    #[test]
    fn test_login_hub_mal_password_drawer_masks_password() {
        let cfg = Config::default();
        let mut state = AppState::new();
        state.account_focus_provider = TrackerProvider::Mal;

        // Step 1: Username entry renders clearly
        state.account_input_mode = AccountInputMode::MalPasswordLoginUsername;
        state.account_text_input = "my_mal_handle".to_string();
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
        terminal.draw(|f| render_overlay(f, &state, &cfg)).unwrap();
        let step1_text: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(step1_text.contains("STEP 1/2: USERNAME"));
        assert!(step1_text.contains("my_mal_handle"));
        assert!(step1_text.contains("Built-In"));

        // Step 2: Password entry masks every character with '•'
        state.pending_mal_login_username = "my_mal_handle".to_string();
        state.account_input_mode = AccountInputMode::MalPasswordLoginPassword;
        state.account_text_input = "SuperSecretPass99".to_string();
        terminal.draw(|f| render_overlay(f, &state, &cfg)).unwrap();
        let step2_text: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(step2_text.contains("STEP 2/2: PASSWORD FOR @my_mal_handle"));
        assert!(!step2_text.contains("SuperSecretPass99"));
        assert!(step2_text.contains("•••••••••••••••••"));
    }
}

//! Multi-user "Who's Watching?" profile switcher & manager modal (`[P]`).

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    db::user::{ProfileSummary, PROFILE_AVATAR_PRESETS},
    state::{AppState, ProfileInputMode},
    ui::theme::ThemePalette,
};

const BG_MODAL: Color = Color::Rgb(15, 16, 25);
const BG_CARD: Color = Color::Rgb(20, 22, 34);
const BG_CARD_SELECTED: Color = Color::Rgb(28, 28, 46);
const FG_PRIMARY: Color = Color::Rgb(235, 238, 250);
const FG_MUTED: Color = Color::Rgb(145, 150, 175);
const EMERALD: Color = Color::Rgb(80, 250, 123);

/// Responsive modal bounds matching `Home` backdrop clearing.
pub fn modal_pct(area: Rect) -> (u16, u16) {
    if area.width >= 110 && area.height >= 32 {
        (78, 68)
    } else {
        (94, 84)
    }
}

/// Render the centered "Who's Watching?" profile modal overlay.
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    let frame_area = frame.area();
    let (pct_x, pct_y) = modal_pct(frame_area);
    let area = centered_rect(pct_x, pct_y, frame_area);
    frame.render_widget(Clear, area);

    let theme = state.theme();
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " ◈ SOLUS // WHO'S WATCHING? ",
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" • ", Style::default().fg(theme.divider)),
            Span::styled(
                format!("Active: {} {} ", state.active_profile_icon, state.active_profile_name),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(BG_MODAL));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 20 || inner.height < 8 {
        return;
    }

    let padded = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    };

    let form_h = if state.profile_input_mode != ProfileInputMode::None {
        6
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),      // Subtitle / isolation notice
            Constraint::Min(9),         // Profile Bento cards
            Constraint::Length(form_h), // Inline create / rename prompt
            Constraint::Length(3),      // Scope note + keybind dock
        ])
        .split(padded);

    // 1. Header subtitle
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("✦ ", Style::default().fg(theme.accent)),
            Span::styled(
                "Select a profile to isolate watch history, continue watching, watchlist, and trackers.",
                Style::default().fg(FG_MUTED),
            ),
        ]),
        Line::from(Span::styled(
            "─".repeat(padded.width as usize),
            Style::default().fg(theme.divider),
        )),
    ]);
    frame.render_widget(header, chunks[0]);

    // 2. Profile Bento cards
    render_profile_cards(frame, chunks[1], state, &theme);

    // 3. Inline Create / Rename form
    if state.profile_input_mode != ProfileInputMode::None {
        render_input_form(frame, chunks[2], state, &theme);
    }

    // 4. Shared cache note + footer controls
    render_footer(frame, chunks[3], state, &theme);
}

fn render_profile_cards(frame: &mut Frame, area: Rect, state: &AppState, theme: &ThemePalette) {
    if state.profiles_list.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No profiles found. Press [n] to create a profile.",
            Style::default().fg(FG_MUTED),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(empty, area);
        return;
    }

    let max_visible = if area.width >= 90 {
        3
    } else if area.width >= 56 {
        2
    } else {
        1
    };

    let total = state.profiles_list.len();
    let visible = total.min(max_visible);
    let cursor = state.profile_cursor.min(total.saturating_sub(1));
    let start = cursor.saturating_sub(visible.saturating_sub(1));
    let slice = &state.profiles_list[start..(start + visible).min(total)];

    let mut constraints = Vec::with_capacity(slice.len());
    let pct = (100 / slice.len().max(1)) as u16;
    for idx in 0..slice.len() {
        if idx + 1 == slice.len() {
            constraints.push(Constraint::Min(0));
        } else {
            constraints.push(Constraint::Percentage(pct));
        }
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, summary) in slice.iter().enumerate() {
        let global_idx = start + i;
        let is_selected = global_idx == cursor;
        render_single_card(frame, cols[i], summary, is_selected, theme);
    }
}

fn render_single_card(
    frame: &mut Frame,
    area: Rect,
    summary: &ProfileSummary,
    is_selected: bool,
    theme: &ThemePalette,
) {
    let card_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    if card_area.width < 14 || card_area.height < 6 {
        return;
    }

    let border_fg = if is_selected {
        theme.primary
    } else if summary.is_active {
        EMERALD
    } else {
        theme.border_idle
    };
    let card_bg = if is_selected { BG_CARD_SELECTED } else { BG_CARD };

    let status_tag = if summary.is_active && is_selected {
        " ACTIVE · SELECTED "
    } else if summary.is_active {
        " ACTIVE "
    } else if is_selected {
        " SELECTED "
    } else {
        " PROFILE "
    };
    let status_fg = if summary.is_active { EMERALD } else { theme.secondary };

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                format!(" {} ", summary.profile.avatar_icon),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                status_tag,
                Style::default()
                    .fg(status_fg)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_fg))
        .style(Style::default().bg(card_bg));

    let inner = block.inner(card_area);
    frame.render_widget(block, card_area);

    let tracker_label = match (summary.auth.has_anilist(), summary.auth.has_mal()) {
        (true, true) => {
            let al = summary.auth.anilist.username.as_deref().unwrap_or("AL");
            let mal = summary.auth.mal.username.as_deref().unwrap_or("MAL");
            format!("󰄬 AL: {al} · MAL: {mal}")
        }
        (true, false) => format!(
            "󰄬 AL: {}",
            summary.auth.anilist.username.as_deref().unwrap_or("AniList")
        ),
        (false, true) => format!(
            "󰄬 MAL: {}",
            summary.auth.mal.username.as_deref().unwrap_or("MAL")
        ),
        (false, false) => "• Local Vault".to_string(),
    };

    let tracker_fg = if summary.auth.is_authenticated() {
        EMERALD
    } else {
        FG_MUTED
    };

    let audio_label = if summary.profile.audio_mode.eq_ignore_ascii_case("dub") {
        "Dub Audio"
    } else {
        "Sub Audio"
    };
    let theme_name = crate::ui::theme::get_theme(summary.profile.theme_index.max(0) as usize).name;

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("{}  {}", summary.profile.avatar_icon, summary.profile.display_name),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Line::from(""),
        Line::from(Span::styled(tracker_label, Style::default().fg(tracker_fg)))
            .alignment(Alignment::Center),
        Line::from(Span::styled(
            format!(
                "{} eps · {:.1}h · {} saved",
                summary.episodes_watched, summary.hours_watched, summary.watchlist_count
            ),
            Style::default().fg(FG_PRIMARY),
        ))
        .alignment(Alignment::Center),
        Line::from(Span::styled(
            format!("{} · {}", audio_label, theme_name),
            Style::default().fg(FG_MUTED),
        ))
        .alignment(Alignment::Center),
    ];

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_input_form(frame: &mut Frame, area: Rect, state: &AppState, theme: &ThemePalette) {
    let heading = match state.profile_input_mode {
        ProfileInputMode::CreateName => " ✦ CREATE NEW PROFILE ",
        ProfileInputMode::RenameName => " ✦ EDIT PROFILE ",
        ProfileInputMode::None => return,
    };

    let block = Block::default()
        .title(Span::styled(
            heading,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(Color::Rgb(19, 21, 34)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut badge_spans = vec![Span::styled(
        "  Badge : ",
        Style::default().fg(FG_MUTED),
    )];
    for (idx, (icon, label)) in PROFILE_AVATAR_PRESETS.iter().enumerate() {
        let active = idx == state.profile_avatar_cursor % PROFILE_AVATAR_PRESETS.len();
        let chip = format!(" {} {} ", icon, label);
        badge_spans.push(Span::styled(
            chip,
            if active {
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG_MUTED)
            },
        ));
        badge_spans.push(Span::raw("  "));
    }

    let audio_str = if state.profile_audio_dub {
        "Dub (English)"
    } else {
        "Sub (Japanese)"
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("  Name  : ", Style::default().fg(FG_MUTED)),
            Span::styled(
                format!("{}█", state.profile_name_input),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   •   Audio : ", Style::default().fg(FG_MUTED)),
            Span::styled(
                audio_str,
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(badge_spans),
        Line::from(vec![
            Span::styled("  [Tab]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
            Span::styled(" Cycle Badge   ", Style::default().fg(FG_MUTED)),
            Span::styled("[↑↓]", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
            Span::styled(" Toggle Sub/Dub   ", Style::default().fg(FG_MUTED)),
            Span::styled("[Enter]", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
            Span::styled(" Save   ", Style::default().fg(FG_MUTED)),
            Span::styled("[Esc]", Style::default().fg(Color::Rgb(255, 105, 135)).add_modifier(Modifier::BOLD)),
            Span::styled(" Cancel", Style::default().fg(FG_MUTED)),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState, theme: &ThemePalette) {
    let lines = vec![
        Line::from(vec![
            Span::styled("  Shared Across Profiles : ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
            Span::styled(
                "Anime Catalog Cache · Cover Art · Offline Downloads",
                Style::default().fg(FG_MUTED),
            ),
        ]),
        Line::from(Span::styled(
            "─".repeat(area.width as usize),
            Style::default().fg(theme.divider),
        )),
        if state.profile_input_mode == ProfileInputMode::None {
            Line::from(vec![
                dock_chip("[Enter]", "Switch", EMERALD),
                Span::raw("  "),
                dock_chip("[h/l]", "Select", theme.secondary),
                Span::raw("  "),
                dock_chip("[n]", "New Profile", theme.primary),
                Span::raw("  "),
                dock_chip("[r]", "Rename", theme.accent),
                Span::raw("  "),
                dock_chip("[i]", "Badge", theme.secondary),
                Span::raw("  "),
                dock_chip("[d]", "Sub/Dub", theme.secondary),
                Span::raw("  "),
                dock_chip("[x]", "Delete", Color::Rgb(255, 105, 135)),
                Span::raw("  "),
                dock_chip("[Esc]", "Close", FG_MUTED),
            ])
            .alignment(Alignment::Center)
        } else {
            Line::from(Span::styled(
                "Typing Profile Name — press [Enter] to save or [Esc] to cancel",
                Style::default().fg(theme.accent),
            ))
            .alignment(Alignment::Center)
        },
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

fn dock_chip(key: &str, label: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!("{key} {label}"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;
    use crate::db::user::UserProfile;
    use crate::state::Screen;
    use ratatui::{backend::TestBackend, Terminal};

    fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn has_color_emoji(s: &str) -> bool {
        s.chars().any(|c| {
            let u = c as u32;
            (0x1F300..=0x1FAFF).contains(&u)
        })
    }

    #[test]
    fn test_profile_modal_renders_cards_without_concatenation_or_color_emojis() {
        let mut state = AppState::new();
        state.screen = Screen::ProfileModal;
        state.active_profile_id = "default".to_string();
        state.active_profile_name = "Abdullah".to_string();
        state.active_profile_icon = "◈".to_string();

        let mut al_auth = AuthConfig::default();
        al_auth.anilist.username = Some("abdullah3".to_string());

        state.open_profile_modal(vec![
            ProfileSummary {
                profile: UserProfile {
                    id: "default".into(),
                    display_name: "Abdullah".into(),
                    avatar_icon: "◈".into(),
                    theme_index: 0,
                    audio_mode: "sub".into(),
                    created_at: 100,
                    last_active_at: 200,
                },
                episodes_watched: 42,
                hours_watched: 16.8,
                watchlist_count: 5,
                auth: al_auth,
                is_active: true,
            },
            ProfileSummary {
                profile: UserProfile {
                    id: "brother".into(),
                    display_name: "Brother".into(),
                    avatar_icon: "✦".into(),
                    theme_index: 1,
                    audio_mode: "dub".into(),
                    created_at: 150,
                    last_active_at: 150,
                },
                episodes_watched: 8,
                hours_watched: 3.2,
                watchlist_count: 2,
                auth: AuthConfig::default(),
                is_active: false,
            },
        ]);

        let backend = TestBackend::new(132, 38);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_overlay(f, &state)).unwrap();

        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("SOLUS // WHO'S WATCHING?"));
        assert!(text.contains("Abdullah"));
        assert!(text.contains("Brother"));
        assert!(text.contains("42 eps · 16.8h · 5 saved"));
        assert!(text.contains("• Local Vault"));
        assert!(text.contains("[Enter] Switch"));
        assert!(text.contains("[n] New Profile"));
        assert!(!text.contains("ACTIVEBrother"));
        assert!(!text.contains("Switch[n]"));
        assert!(!has_color_emoji(&text));
    }

    #[test]
    fn test_profile_modal_create_input_renders_cleanly() {
        let mut state = AppState::new();
        state.screen = Screen::ProfileModal;
        state.profile_input_mode = ProfileInputMode::CreateName;
        state.profile_name_input = "Sarah".to_string();

        let backend = TestBackend::new(132, 38);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_overlay(f, &state)).unwrap();

        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("CREATE NEW PROFILE"));
        assert!(text.contains("Sarah█"));
        assert!(text.contains("[Tab] Cycle Badge"));
        assert!(!has_color_emoji(&text));
    }
}

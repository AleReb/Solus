//! Contextual Bento command matrix & keybinding overlay (`?`).

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    state::{AppState, Screen},
    ui::theme::ThemePalette,
};

const BG_MODAL: Color = Color::Rgb(15, 16, 25);
const BG_CARD: Color = Color::Rgb(19, 20, 32);
const BG_CARD_ACTIVE: Color = Color::Rgb(24, 24, 40);
const BG_KEYCAP: Color = Color::Rgb(30, 32, 50);
const BG_KEYCAP_ACTIVE: Color = Color::Rgb(42, 36, 68);
const FG_PRIMARY: Color = Color::Rgb(235, 238, 250);
const FG_MUTED: Color = Color::Rgb(150, 155, 180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpSection {
    Browsing,
    Playback,
    Features,
    System,
}

struct BindingRow {
    key: &'static str,
    desc: &'static str,
}

struct SectionSpec {
    section: HelpSection,
    title: &'static str,
    subtitle: &'static str,
    bindings: &'static [BindingRow],
}

const BROWSING_BINDINGS: &[BindingRow] = &[
    BindingRow { key: "Enter/Click", desc: "Open anime detail stage" },
    BindingRow { key: "h/j/k/l", desc: "Navigate shelves & posters" },
    BindingRow { key: "r", desc: "Resume next episode" },
    BindingRow { key: "/", desc: "Instant anime search" },
    BindingRow { key: "+ / x", desc: "Add or remove watchlist" },
    BindingRow { key: "R", desc: "Refresh AniList shelves" },
];

const PLAYBACK_BINDINGS: &[BindingRow] = &[
    BindingRow { key: "Enter/Space", desc: "Launch episode playback" },
    BindingRow { key: "d", desc: "Toggle Sub / Dub audio" },
    BindingRow { key: "q", desc: "Cycle resolution (720p+)" },
    BindingRow { key: "p", desc: "Switch player (mpv/iina/vlc)" },
    BindingRow { key: "u", desc: "Cycle Anime4K shader (2x/4K)" },
    BindingRow { key: "i / w / Esc", desc: "Status / Web / Go back" },
];

const FEATURES_BINDINGS: &[BindingRow] = &[
    BindingRow { key: "a", desc: "Taste Analytics Bento Hub" },
    BindingRow { key: "w", desc: "Tailored recommendation wizard" },
    BindingRow { key: "c", desc: "Weekly airing calendar" },
    BindingRow { key: "m", desc: "Mobile LAN remote & QR" },
    BindingRow { key: "C", desc: "LAN Cast Deck & DLNA stream" },
    BindingRow { key: "L", desc: "AniList OAuth connect / sync" },
];

const SYSTEM_BINDINGS: &[BindingRow] = &[
    BindingRow { key: "s", desc: "Open Settings Control Hub" },
    BindingRow { key: "t / T", desc: "Cycle active UI theme" },
    BindingRow { key: "!", desc: "Open System Doctor & Pipeline" },
    BindingRow { key: "U", desc: "Check or install self-update" },
    BindingRow { key: "X", desc: "Stop active player process" },
    BindingRow { key: "Q", desc: "Quit Solus immediately" },
];

/// Returns the responsive modal `(percent_x, percent_y)` matching `render_overlay`.
pub fn modal_pct(area: Rect) -> (u16, u16) {
    if area.width >= 100 && area.height >= 32 {
        (86, 84)
    } else {
        (96, 92)
    }
}

fn active_section(screen: &Screen) -> HelpSection {
    match screen {
        Screen::Home | Screen::Search => HelpSection::Browsing,
        Screen::Detail
        | Screen::PlaybackQuery
        | Screen::PlaybackOptions
        | Screen::Playback => HelpSection::Playback,
        Screen::TasteAnalytics
        | Screen::TailoredWizard
        | Screen::TailoredSummary
        | Screen::TailoredResults
        | Screen::Calendar
        | Screen::CastModal
        | Screen::CastDeck
        | Screen::RemoteModal => HelpSection::Features,
        Screen::Settings
        | Screen::Setup
        | Screen::UpdateModal
        | Screen::AniListLogin
        | Screen::Help => HelpSection::System,
    }
}

fn context_label(screen: &Screen) -> &'static str {
    match screen {
        Screen::Home => "HOME EXPLORE",
        Screen::Search => "SEARCH",
        Screen::Detail => "ANIME DETAIL",
        Screen::PlaybackQuery | Screen::PlaybackOptions | Screen::Playback => "PLAYBACK ENGINE",
        Screen::TasteAnalytics => "TASTE ANALYTICS",
        Screen::TailoredWizard | Screen::TailoredSummary | Screen::TailoredResults => "TAILORED PICKS",
        Screen::Calendar => "RELEASE CALENDAR",
        Screen::CastModal | Screen::CastDeck => "LAN CASTING",
        Screen::RemoteModal => "MOBILE REMOTE",
        Screen::Settings => "SETTINGS HUB",
        Screen::Setup => "SYSTEM DOCTOR",
        Screen::UpdateModal => "SELF-UPDATE",
        Screen::AniListLogin => "ANILIST AUTH",
        Screen::Help => "COMMAND MATRIX",
    }
}

/// Render the contextual 2x2 Bento command matrix modal overlay.
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    let frame_area = frame.area();
    let (pct_x, pct_y) = modal_pct(frame_area);
    let area = centered_rect(pct_x, pct_y, frame_area);
    frame.render_widget(Clear, area);

    let theme = state.theme();
    let context_screen = if state.screen == Screen::Help {
        &state.overlay_base
    } else {
        &state.screen
    };
    let focused_section = active_section(context_screen);
    let ctx_name = context_label(context_screen);

    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " ◈ SOLUS // COMMAND MATRIX ",
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("• CONTEXT: ", Style::default().fg(FG_MUTED)),
            Span::styled(
                format!("{} ", ctx_name),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(BG_MODAL));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if inner.width < 12 || inner.height < 6 {
        return;
    }

    let padded = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    };

    let show_header_rule = padded.height >= 25;
    let show_dock_rule = padded.height >= 22;
    let header_h = if show_header_rule { 2 } else { 1 };
    let dock_h = if show_dock_rule { 2 } else { 1 };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Min(0),
            Constraint::Length(dock_h),
        ])
        .split(padded);

    render_header(frame, rows[0], ctx_name, &theme, show_header_rule);

    let sections = [
        SectionSpec {
            section: HelpSection::Browsing,
            title: "Browsing & Discovery",
            subtitle: "Home shelves & search",
            bindings: BROWSING_BINDINGS,
        },
        SectionSpec {
            section: HelpSection::Playback,
            title: "Detail & Playback",
            subtitle: "Streams, audio & shaders",
            bindings: PLAYBACK_BINDINGS,
        },
        SectionSpec {
            section: HelpSection::Features,
            title: "Intelligence & Companion",
            subtitle: "Analytics, calendar & LAN",
            bindings: FEATURES_BINDINGS,
        },
        SectionSpec {
            section: HelpSection::System,
            title: "System & Quick Bridges",
            subtitle: "Hub, doctor & runtime",
            bindings: SYSTEM_BINDINGS,
        },
    ];

    render_bento_cards(frame, rows[1], &sections, focused_section, &theme);
    render_action_dock(frame, rows[2], &theme, show_dock_rule);
}

fn render_header(
    frame: &mut Frame,
    area: Rect,
    ctx_name: &str,
    theme: &ThemePalette,
    show_rule: bool,
) {
    let header_rows = if show_rule && area.height >= 2 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1)])
            .split(area)
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(header_rows[0]);

    let left = Line::from(vec![
        Span::styled(" ◆ ", Style::default().fg(theme.accent)),
        Span::styled("Active Pane Highlighted: ", Style::default().fg(FG_MUTED)),
        Span::styled(
            ctx_name,
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let right = Line::from(vec![
        Span::styled("Theme: ", Style::default().fg(FG_MUTED)),
        Span::styled(
            format!("{} ", theme.name),
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .alignment(Alignment::Right);

    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(Paragraph::new(right), cols[1]);

    if show_rule && header_rows.len() > 1 {
        let rule = "─".repeat(area.width as usize);
        frame.render_widget(
            Paragraph::new(Span::styled(rule, Style::default().fg(theme.divider))),
            header_rows[1],
        );
    }
}

fn render_bento_cards(
    frame: &mut Frame,
    area: Rect,
    sections: &[SectionSpec; 4],
    focused: HelpSection,
    theme: &ThemePalette,
) {
    // 2x2 Bento matrix when horizontal budget avoids label truncation; 1-column fallback on narrow panes.
    if area.width >= 76 && area.height >= 16 {
        let grid_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let top_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(grid_rows[0]);
        let bot_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(grid_rows[1]);

        let rects = [top_cols[0], top_cols[1], bot_cols[0], bot_cols[1]];
        for (idx, spec) in sections.iter().enumerate() {
            render_section_card(frame, rects[idx], spec, spec.section == focused, theme);
        }
    } else {
        let constraints = [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ];
        let stack = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);
        for (idx, spec) in sections.iter().enumerate() {
            render_section_card(frame, stack[idx], spec, spec.section == focused, theme);
        }
    }
}

fn render_section_card(
    frame: &mut Frame,
    area: Rect,
    spec: &SectionSpec,
    is_active: bool,
    theme: &ThemePalette,
) {
    let card_bg = if is_active { BG_CARD_ACTIVE } else { BG_CARD };
    let keycap_bg = if is_active {
        BG_KEYCAP_ACTIVE
    } else {
        BG_KEYCAP
    };
    let border_fg = if is_active {
        theme.primary
    } else {
        theme.border_idle
    };
    let title_fg = if is_active {
        theme.primary
    } else {
        theme.secondary
    };

    let mut title_spans = vec![Span::styled(
        format!(" {} ", spec.title),
        Style::default().fg(title_fg).add_modifier(Modifier::BOLD),
    )];
    if is_active {
        title_spans.push(Span::styled(
            "◆ ACTIVE ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_fg))
        .style(Style::default().bg(card_bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 8 || inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let max_rows = inner.height as usize;

    // Include contextual subtitle on spacious cards while keeping all 6 bindings visible when compact.
    if max_rows > spec.bindings.len() + 1 {
        lines.push(Line::from(Span::styled(
            format!("  {}", spec.subtitle),
            Style::default().fg(theme.text_dim),
        )));
    }

    let key_col_width: usize = if inner.width >= 42 { 13 } else { 11 };
    let key_fg = if is_active {
        theme.accent
    } else {
        theme.primary
    };

    for row in spec.bindings {
        if lines.len() >= max_rows {
            break;
        }
        let pill = format!(" {} ", row.key);
        let pill_chars = pill.chars().count();
        let pad = key_col_width.saturating_sub(pill_chars) + 2;

        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                pill,
                Style::default()
                    .fg(key_fg)
                    .bg(keycap_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(pad)),
            Span::styled(row.desc, Style::default().fg(FG_PRIMARY)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_action_dock(frame: &mut Frame, area: Rect, theme: &ThemePalette, show_rule: bool) {
    let rows = if show_rule && area.height >= 2 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1)])
            .split(area)
    };

    if show_rule && rows.len() > 1 {
        let rule = "─".repeat(area.width as usize);
        frame.render_widget(
            Paragraph::new(Span::styled(rule, Style::default().fg(theme.divider))),
            rows[0],
        );
    }

    let dock_area = if show_rule && rows.len() > 1 {
        rows[1]
    } else {
        rows[0]
    };

    let theme_label = format!("Theme: {}", theme.name);
    let dock = Line::from(vec![
        dock_pill("s", "Settings Hub", theme.primary),
        Span::raw("   "),
        dock_pill("t", &theme_label, theme.secondary),
        Span::raw("   "),
        dock_pill("!", "System Doctor", theme.accent),
        Span::raw("   "),
        dock_pill("?/Esc", "Dismiss", FG_MUTED),
    ])
    .alignment(Alignment::Center);

    frame.render_widget(Paragraph::new(dock), dock_area);
}

fn dock_pill(key: &str, label: &str, accent: Color) -> Span<'static> {
    Span::styled(
        format!("[{}] {}", key, label),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )
}

/// Render the toast notification bar at the bottom of the screen.
pub fn render_toast(frame: &mut Frame, message: &str) {
    let area = frame.area();
    let height = 1u16;
    let toast_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(height),
        width: area.width,
        height,
    };

    let toast = Paragraph::new(Span::styled(
        format!("  ✓ {}", message),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Rgb(180, 0, 255))
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(toast, toast_area);
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

    #[test]
    fn test_command_matrix_renders_all_bento_sections_desktop_and_compact() {
        for (w, h) in [(140u16, 40u16), (90u16, 28u16)] {
            let mut state = AppState::default();
            state.screen = Screen::Help;
            state.overlay_base = Screen::Home;

            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| render_overlay(f, &state))
                .expect("command matrix should render cleanly");

            let text = buffer_text(terminal.backend().buffer());
            assert!(text.contains("SOLUS // COMMAND MATRIX"), "missing header at {}x{}", w, h);
            assert!(text.contains("Browsing & Discovery"), "missing browsing card at {}x{}", w, h);
            assert!(text.contains("Detail & Playback"), "missing playback card at {}x{}", w, h);
            assert!(
                text.contains("Intelligence & Companion"),
                "missing features card at {}x{}",
                w,
                h
            );
            assert!(
                text.contains("System & Quick Bridges"),
                "missing system card at {}x{}",
                w,
                h
            );
            assert!(text.contains("Settings Hub"), "missing Settings action dock at {}x{}", w, h);
            assert!(text.contains("System Doctor"), "missing Doctor action dock at {}x{}", w, h);
        }
    }

    #[test]
    fn test_command_matrix_highlights_active_overlay_base_context() {
        let mut state = AppState::default();
        state.screen = Screen::Help;
        state.overlay_base = Screen::Detail;

        let backend = TestBackend::new(130, 38);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_overlay(f, &state)).unwrap();

        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("CONTEXT: ANIME DETAIL"), "expected ANIME DETAIL badge");
        assert!(
            text.contains("Detail & Playback◆ ACTIVE") || text.contains("Detail & Playback ◆ ACTIVE"),
            "expected Detail & Playback card to carry ACTIVE badge"
        );
    }

    #[test]
    fn test_command_matrix_handles_narrow_terminal_without_panicking() {
        let mut state = AppState::default();
        state.screen = Screen::Help;
        state.overlay_base = Screen::TasteAnalytics;

        let backend = TestBackend::new(64, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_overlay(f, &state))
            .expect("narrow terminal fallback must not panic");

        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("COMMAND MATRIX"));
        assert!(text.contains("TASTE ANALYTICS"));
    }
}

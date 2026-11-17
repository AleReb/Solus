//! Playback screen — ani-cli log stream + player controls.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::{state::AppState, ui::theme::ThemePalette};

/// Render the playback screen.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let theme = state.theme();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header: now playing
            Constraint::Min(0),    // log stream
            Constraint::Length(3), // controls
        ])
        .split(area);

    render_header(frame, chunks[0], state, &theme);
    render_logs(frame, chunks[1], state, &theme);
    render_controls(frame, chunks[2], &theme);
}

fn render_header(frame: &mut Frame, area: ratatui::layout::Rect, state: &AppState, theme: &ThemePalette) {
    let title = state
        .now_playing
        .as_deref()
        .unwrap_or("Starting playback...");

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" ", Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)),
        Span::styled(title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_idle)),
    );
    frame.render_widget(header, area);
}

fn render_logs(frame: &mut Frame, area: ratatui::layout::Rect, state: &AppState, theme: &ThemePalette) {
    // Show last N lines that fit in the area
    let visible = area.height.saturating_sub(2) as usize;
    let logs    = &state.playback_logs;
    let start   = logs.len().saturating_sub(visible);

    let items: Vec<ListItem> = logs[start..]
        .iter()
        .map(|line| {
            let style = if line.contains("error") || line.contains("Error") {
                Style::default().fg(Color::Rgb(255, 80, 80))
            } else if line.contains("›") || line.starts_with('[') {
                Style::default().fg(theme.primary)
            } else {
                Style::default().fg(Color::Rgb(180, 180, 180))
            };
            ListItem::new(Line::from(Span::styled(format!("  {}", line), style)))
        })
        .collect();

    let log_list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                " Output ",
                Style::default().fg(theme.text_dim),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_idle)),
    );
    frame.render_widget(log_list, area);
}

fn render_controls(frame: &mut Frame, area: ratatui::layout::Rect, theme: &ThemePalette) {
    let controls = Paragraph::new(Line::from(vec![
        Span::styled(" q ", Style::default().fg(Color::Black).bg(theme.primary).add_modifier(Modifier::BOLD)),
        Span::styled(" Stop    ", Style::default().fg(theme.text_dim)),
        Span::styled(" n ", Style::default().fg(Color::Black).bg(theme.border_idle).add_modifier(Modifier::BOLD)),
        Span::styled(" Next Ep    ", Style::default().fg(theme.text_dim)),
        Span::styled(" a ", Style::default().fg(Color::Black).bg(theme.secondary).add_modifier(Modifier::BOLD)),
        Span::styled(" Sub/Dub    ", Style::default().fg(theme.text_dim)),
        Span::styled(" Esc ", Style::default().fg(Color::Black).bg(theme.border_idle).add_modifier(Modifier::BOLD)),
        Span::styled(" Return", Style::default().fg(theme.text_dim)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_idle)),
    );
    frame.render_widget(controls, area);
}

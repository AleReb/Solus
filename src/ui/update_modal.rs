//! In-app self-updater & plain-English "What's New & Fixed" transparency modal.
//!
//! Displays clear, jargon-free summaries of what changed and what was fixed:
//! 1. **Available**: Before updating, shows version diff + plain-English "What's New & Fixed".
//! 2. **Downloading**: While updating, shows live download gauge alongside the plain-English summary.
//! 3. **RestartRequired**: After atomic swap, summarizes what was installed and prompts restart.
//! 4. **PostUpdateWelcome**: On first launch after an update (including silent auto-installs),
//!    welcomes the user with a friendly overview of what changed.

use crate::services::updater::{default_current_release_notes, simplify_release_notes};
use crate::state::{AppState, UpdateModalPhase};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph, Wrap},
    Frame,
};

const EMERALD: Color = Color::Rgb(80, 250, 123);
const CYAN: Color = Color::Rgb(0, 210, 255);
const AMBER: Color = Color::Rgb(251, 191, 36);
const CORAL: Color = Color::Rgb(255, 80, 80);

/// Render the Update Modal overlay centered on top of the current screen.
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    let Some(ref phase) = state.update_modal_phase else {
        return;
    };

    let area = centered_rect(80, 76, frame.area());
    frame.render_widget(Clear, area);

    let theme = state.theme();
    let border_color = match phase {
        UpdateModalPhase::Available { .. } => theme.primary,
        UpdateModalPhase::Downloading { .. } => CYAN,
        UpdateModalPhase::RestartRequired { .. } => EMERALD,
        UpdateModalPhase::PostUpdateWelcome { .. } => EMERALD,
        UpdateModalPhase::Failed { .. } => CORAL,
    };

    let title = match phase {
        UpdateModalPhase::Available { .. } => " ❖ Solus Update Available ❖ ".to_string(),
        UpdateModalPhase::Downloading { .. } => " ❖ Downloading Solus Update ❖ ".to_string(),
        UpdateModalPhase::RestartRequired { .. } => " ❖ Update Successfully Applied ❖ ".to_string(),
        UpdateModalPhase::PostUpdateWelcome { version_label, .. } => {
            format!(" ❖ Solus Just Got Better! ({}) ❖ ", version_label)
        }
        UpdateModalPhase::Failed { .. } => " ⚠ Solus Update Notice ⚠ ".to_string(),
    };

    let modal_bg = Color::Rgb(15, 16, 25);
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(modal_bg));

    let inner = block.inner(area).inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    frame.render_widget(block, area);

    match phase {
        UpdateModalPhase::Available { info } => {
            render_available(frame, inner, info, border_color);
        }
        UpdateModalPhase::Downloading {
            info,
            progress_pct,
            speed,
        } => {
            render_downloading(frame, inner, info.as_ref(), *progress_pct, speed, border_color);
        }
        UpdateModalPhase::RestartRequired {
            new_version,
            simple_notes,
        } => {
            render_restart_required(frame, inner, new_version, simple_notes, border_color);
        }
        UpdateModalPhase::PostUpdateWelcome {
            version_label,
            simple_notes,
        } => {
            render_post_update_welcome(frame, inner, version_label, simple_notes, border_color);
        }
        UpdateModalPhase::Failed { error } => {
            render_failed(frame, inner, error, border_color);
        }
    }
}

fn notes_for_info(info: &crate::services::updater::UpdateInfo) -> Vec<String> {
    let mut notes = if !info.simple_notes.is_empty() {
        info.simple_notes.clone()
    } else if let Some(ref raw) = info.release_notes {
        simplify_release_notes(raw)
    } else {
        default_current_release_notes()
    };

    if notes.len() < 6 {
        for fallback in default_current_release_notes() {
            if notes.len() >= 6 {
                break;
            }
            let prefix = fallback
                .split_once(':')
                .map(|(head, _)| head.to_ascii_lowercase())
                .unwrap_or_else(|| fallback.to_ascii_lowercase());
            let already_present = notes.iter().any(|existing| {
                let ex_prefix = existing
                    .split_once(':')
                    .map(|(head, _)| head.to_ascii_lowercase())
                    .unwrap_or_else(|| existing.to_ascii_lowercase());
                ex_prefix == prefix || existing.eq_ignore_ascii_case(&fallback)
            });
            if !already_present {
                notes.push(fallback);
            }
        }
    }

    notes
}

fn push_plain_english_section(lines: &mut Vec<Line<'static>>, notes: &[String], header: &str) {
    lines.push(Line::from(Span::styled(
        header.to_string(),
        Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    for (idx, note) in notes.iter().take(6).enumerate() {
        lines.push(format_plain_english_bullet(note));
        if idx + 1 < notes.len().min(6) {
            lines.push(Line::from(""));
        }
    }
}

fn append_body_with_how_to_use(spans: &mut Vec<Span<'static>>, body: &str) {
    if let Some((desc, guide)) = body.split_once("How to use:") {
        if !desc.is_empty() {
            spans.push(Span::styled(
                desc.to_string(),
                Style::default().fg(Color::White),
            ));
        }
        spans.push(Span::styled(
            "How to use:",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            guide.to_string(),
            Style::default().fg(Color::Rgb(215, 225, 240)),
        ));
    } else {
        spans.push(Span::styled(
            body.to_string(),
            Style::default().fg(Color::White),
        ));
    }
}

fn format_plain_english_bullet(note: &str) -> Line<'static> {
    let trimmed = note.trim();
    for (badge, color) in [("[NEW]", EMERALD), ("[FIXED]", CYAN), ("[IMPROVED]", AMBER)] {
        if let Some(rest) = trimmed.strip_prefix(badge) {
            let mut spans = vec![
                Span::styled("  • ", Style::default().fg(color)),
                Span::styled(
                    format!("{badge} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ];
            append_body_with_how_to_use(&mut spans, rest.trim());
            return Line::from(spans);
        }
    }

    let mut spans = vec![Span::styled("  • ", Style::default().fg(CYAN))];
    append_body_with_how_to_use(&mut spans, trimmed);
    Line::from(spans)
}

fn render_available(
    frame: &mut Frame,
    area: Rect,
    info: &crate::services::updater::UpdateInfo,
    accent: Color,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Version diff
            Constraint::Min(6),    // Plain-English highlights
            Constraint::Length(2), // Reassurance line
            Constraint::Length(2), // Action buttons
        ])
        .split(area);

    let version_lines = vec![Line::from(vec![
        Span::styled("Current Version: ", Style::default().fg(Color::Rgb(160, 160, 180))),
        Span::styled(
            &info.current_version,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ──►   ", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled("New Version: ", Style::default().fg(Color::Rgb(160, 160, 180))),
        Span::styled(
            &info.latest_version,
            Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
        ),
    ])];
    frame.render_widget(
        Paragraph::new(version_lines).alignment(Alignment::Center),
        chunks[0],
    );

    let notes = notes_for_info(info);
    let mut notes_lines = Vec::new();
    push_plain_english_section(&mut notes_lines, &notes, "What's New & Fixed for You (Plain English):");
    frame.render_widget(
        Paragraph::new(notes_lines).wrap(Wrap { trim: false }),
        chunks[1],
    );

    let safety = Paragraph::new(Line::from(Span::styled(
        "Your anime progress, watch queue, playlists, and settings stay safely preserved.",
        Style::default().fg(Color::Rgb(150, 155, 175)),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(safety, chunks[2]);

    let action_line = if info.is_homebrew {
        Line::from(vec![
            Span::styled("[Esc] ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(
                "Close  (Run 'brew upgrade solus' in your terminal)",
                Style::default().fg(Color::Rgb(200, 200, 220)),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " [Enter / u] Update Now ",
                Style::default()
                    .fg(Color::Black)
                    .bg(EMERALD)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   ", Style::default()),
            Span::styled("[b] ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("Background   ", Style::default().fg(Color::White)),
            Span::styled("[Esc] ", Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
            Span::styled("Remind Me Later", Style::default().fg(Color::Rgb(180, 180, 195))),
        ])
    };
    frame.render_widget(
        Paragraph::new(vec![action_line]).alignment(Alignment::Center),
        chunks[3],
    );
}

fn render_downloading(
    frame: &mut Frame,
    area: Rect,
    info: Option<&crate::services::updater::UpdateInfo>,
    pct: f32,
    speed: &str,
    accent: Color,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Header
            Constraint::Length(3), // Progress gauge
            Constraint::Length(2), // Speed & details
            Constraint::Min(5),    // What's New & Fixed during download
            Constraint::Length(2), // Background hint
        ])
        .split(area);

    let target_label = info
        .map(|i| format!("Updating Solus: {}  ──►  {}", i.current_version, i.latest_version))
        .unwrap_or_else(|| "Downloading & installing latest Solus update...".to_string());
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            target_label,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        chunks[0],
    );

    let clamped_pct = pct.clamp(0.0, 100.0);
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Download Progress ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(accent)),
        )
        .gauge_style(Style::default().fg(accent).bg(Color::Rgb(30, 30, 45)))
        .percent(clamped_pct as u16)
        .label(Span::styled(
            format!("{:.1}%", clamped_pct),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(gauge, chunks[1]);

    let details = Paragraph::new(vec![Line::from(vec![
        Span::styled("Speed: ", Style::default().fg(Color::Rgb(140, 140, 160))),
        Span::styled(speed, Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)),
        Span::styled("   •   ", Style::default().fg(Color::Rgb(80, 80, 100))),
        Span::styled(
            "Your watch history and queue stay safe during the update",
            Style::default().fg(Color::Rgb(180, 180, 200)),
        ),
    ])])
    .alignment(Alignment::Center);
    frame.render_widget(details, chunks[2]);

    let notes = info
        .map(notes_for_info)
        .unwrap_or_else(default_current_release_notes);
    let mut notes_lines = Vec::new();
    push_plain_english_section(
        &mut notes_lines,
        &notes,
        "What's New & Fixed in This Update (Plain English):",
    );
    frame.render_widget(
        Paragraph::new(notes_lines).wrap(Wrap { trim: false }),
        chunks[3],
    );

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[b / Esc] ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(
            "Finish Quietly in Background",
            Style::default().fg(Color::Rgb(170, 175, 195)),
        ),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[4]);
}

fn render_restart_required(
    frame: &mut Frame,
    area: Rect,
    new_version: &str,
    simple_notes: &[String],
    accent: Color,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Success banner
            Constraint::Min(5),    // What's included
            Constraint::Length(2), // Safety note
            Constraint::Length(2), // Action prompt
        ])
        .split(area);

    let success_banner = Paragraph::new(vec![Line::from(Span::styled(
        format!("✓ Solus has been updated to {} and is ready!", new_version),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    ))])
    .alignment(Alignment::Center);
    frame.render_widget(success_banner, chunks[0]);

    let notes = if simple_notes.is_empty() {
        default_current_release_notes()
    } else {
        simple_notes.to_vec()
    };
    let mut notes_lines = Vec::new();
    push_plain_english_section(&mut notes_lines, &notes, "What's Included in This Update:");
    frame.render_widget(
        Paragraph::new(notes_lines).wrap(Wrap { trim: false }),
        chunks[1],
    );

    let instructions = Paragraph::new(Line::from(Span::styled(
        "Your anime progress, queue, playlists, and settings are safely preserved.",
        Style::default().fg(Color::Rgb(160, 160, 180)),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(instructions, chunks[2]);

    let actions = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "[ Enter ] ",
            Style::default().fg(EMERALD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Exit Solus to Relaunch",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("          ", Style::default().fg(Color::Reset)),
        Span::styled(
            "[ Esc ] ",
            Style::default().fg(Color::Rgb(160, 160, 180)).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Continue Current Session",
            Style::default().fg(Color::Rgb(160, 160, 180)),
        ),
    ])])
    .alignment(Alignment::Center);
    frame.render_widget(actions, chunks[3]);
}

fn render_post_update_welcome(
    frame: &mut Frame,
    area: Rect,
    _version_label: &str,
    simple_notes: &[String],
    _accent: Color,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Greeting
            Constraint::Min(6),    // Plain-English What's New & Fixed
            Constraint::Length(2), // Tip
            Constraint::Length(2), // Close button
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "You are now running the latest version of Solus! Here is what changed:",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        chunks[0],
    );

    let notes = if simple_notes.is_empty() {
        default_current_release_notes()
    } else {
        simple_notes.to_vec()
    };
    let mut notes_lines = Vec::new();
    push_plain_english_section(
        &mut notes_lines,
        &notes,
        "What's New & Fixed for You (Plain English):",
    );
    frame.render_widget(
        Paragraph::new(notes_lines).wrap(Wrap { trim: false }),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Tip: Re-open these release notes anytime from Control Hub [s] -> What's New.",
            Style::default().fg(Color::Rgb(150, 155, 175)),
        )))
        .alignment(Alignment::Center),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " [ Enter / Space / Esc ] Start Watching ",
                Style::default()
                    .fg(Color::Black)
                    .bg(EMERALD)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center),
        chunks[3],
    );
}

fn render_failed(frame: &mut Frame, area: Rect, error: &str, accent: Color) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Header
            Constraint::Min(4),    // Error message
            Constraint::Length(2), // Close prompt
        ])
        .split(area);

    let header = Paragraph::new(vec![Line::from(Span::styled(
        "Automatic update could not be completed right now.",
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    ))])
    .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    let err_msg = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Details: {}", error),
            Style::default().fg(Color::Rgb(255, 160, 160)),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Your current Solus installation was not modified.",
            Style::default().fg(Color::Rgb(170, 175, 195)),
        )),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: false });
    frame.render_widget(err_msg, chunks[1]);

    let actions = Paragraph::new(vec![Line::from(vec![
        Span::styled("[Enter / Esc] ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("Dismiss", Style::default().fg(Color::Rgb(180, 180, 180))),
    ])])
    .alignment(Alignment::Center);
    frame.render_widget(actions, chunks[2]);
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
    use crate::services::updater::UpdateInfo;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_update_modal_shows_plain_english_notes_across_all_phases() {
        let mut state = AppState::new();
        let info = UpdateInfo {
            current_version: "v1.0.7 (4d9ef06)".to_string(),
            latest_version: "v1.0.8 (87f9ca4)".to_string(),
            download_url: "https://example.com/solus".to_string(),
            asset_name: "solus-macos-aarch64".to_string(),
            release_notes: Some("Automated build for commit `87f9ca4`: feat: add auto_fullscreen toggle".to_string()),
            simple_notes: vec![
                "[NEW] Added an Auto-Fullscreen setting in the Control Hub.".to_string(),
                "[FIXED] Episodes stay in your queue until 90% completion.".to_string(),
            ],
            is_homebrew: false,
        };

        // 1. Available phase
        state.open_update_modal(info.clone());
        let mut terminal = Terminal::new(TestBackend::new(140, 42)).unwrap();
        terminal.draw(|f| render_overlay(f, &state)).unwrap();
        let avail_text: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(avail_text.contains("Update Now"));
        assert!(avail_text.contains("Plain English"));
        assert!(avail_text.contains("Auto-Fullscreen"));

        // 2. Downloading phase still displays plain-English notes
        state.update_modal_phase = Some(UpdateModalPhase::Downloading {
            info: Some(info.clone()),
            progress_pct: 64.0,
            speed: "8.2 MB/s".to_string(),
        });
        terminal.draw(|f| render_overlay(f, &state)).unwrap();
        let dl_text: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(dl_text.contains("64.0%"));
        assert!(dl_text.contains("Episodes stay in your queue until 90% completion"));

        // 3. PostUpdateWelcome phase
        state.open_post_update_welcome("v1.0.8".to_string(), info.simple_notes.clone());
        terminal.draw(|f| render_overlay(f, &state)).unwrap();
        let welcome_text: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(welcome_text.contains("Solus Just Got Better! (v1.0.8)"));
        assert!(welcome_text.contains("Start Watching"));
    }
}

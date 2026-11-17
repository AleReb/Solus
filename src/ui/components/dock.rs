//! Shared priority-aware 2-row Bottom Command Dock builder used by Home and Detail screens.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Single clickable/keybound pill in a command dock row.
#[derive(Clone)]
pub struct DockChip<A: Copy + Eq> {
    pub icon:        &'static str,
    pub key:         &'static str,
    pub label:       String,
    pub key_color:   Color,
    pub label_color: Color,
    pub priority:    u8,
    pub bg:          Option<Color>,
    pub action:      Option<A>,
}

impl<A: Copy + Eq> DockChip<A> {
    pub fn width(&self) -> usize {
        let icon_w = if self.icon.is_empty() {
            0
        } else {
            self.icon.chars().count() + 1
        };
        let key_w = self.key.chars().count();
        let lbl_w = if self.label.is_empty() {
            0
        } else {
            1 + self.label.chars().count()
        };
        let pad = if self.bg.is_some() { 2 } else { 0 };
        icon_w + key_w + lbl_w + pad
    }

    pub fn push_spans(&self, spans: &mut Vec<Span<'static>>) {
        if let Some(bg_col) = self.bg {
            let mut text = String::from(" ");
            if !self.icon.is_empty() {
                text.push_str(self.icon);
                text.push(' ');
            }
            text.push_str(self.key);
            if !self.label.is_empty() {
                text.push(' ');
                text.push_str(&self.label);
            }
            text.push(' ');
            spans.push(Span::styled(
                text,
                Style::default()
                    .fg(self.key_color)
                    .bg(bg_col)
                    .add_modifier(Modifier::BOLD),
            ));
            return;
        }

        if !self.icon.is_empty() {
            spans.push(Span::styled(
                format!("{} ", self.icon),
                Style::default().fg(self.key_color),
            ));
        }
        spans.push(Span::styled(
            self.key.to_string(),
            Style::default()
                .fg(self.key_color)
                .add_modifier(Modifier::BOLD),
        ));
        if !self.label.is_empty() {
            spans.push(Span::styled(
                format!(" {}", self.label),
                Style::default().fg(self.label_color),
            ));
        }
    }
}

/// Fitted dock row with exact left/right alignment and mouse hitbox spans.
pub struct DockRowPlan<A: Copy + Eq> {
    pub line:     Line<'static>,
    pub hitboxes: Vec<(u16, u16, A)>,
}

/// Select highest-priority chips (`priority` 0 first) that fit cleanly inside `budget`, preserving display order.
pub fn select_chips_by_priority<A: Copy + Eq>(
    candidates: &[DockChip<A>],
    budget: usize,
    sep_w: usize,
) -> Vec<DockChip<A>> {
    if budget == 0 || candidates.is_empty() {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by_key(|&i| (candidates[i].priority, i));

    let mut chosen = vec![false; candidates.len()];
    let mut used = 0usize;
    let mut count = 0usize;

    for idx in order {
        let extra = candidates[idx].width() + if count > 0 { sep_w } else { 0 };
        if used + extra <= budget {
            chosen[idx] = true;
            used += extra;
            count += 1;
        }
    }

    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| if chosen[i] { Some(c.clone()) } else { None })
        .collect()
}

/// Pack left/right dock chips cleanly into `width` without overflowing or mid-word clipping.
pub fn build_dock_row_plan<A: Copy + Eq>(
    badge_text: &str,
    badge_fg: Color,
    badge_bg: Color,
    left_candidates: Vec<DockChip<A>>,
    right_candidates: Vec<DockChip<A>>,
    width: usize,
) -> DockRowPlan<A> {
    let badge_w = badge_text.chars().count();
    let gutter_w = if width >= 116 { 2usize } else { 1usize };
    let gutter_str = if gutter_w == 2 { "  " } else { " " };
    let trailing_w = 1usize;

    let mut right_chosen =
        select_chips_by_priority(&right_candidates, width.saturating_sub(badge_w + 4), gutter_w);
    if right_chosen.is_empty() && !right_candidates.is_empty() {
        if let Some(top) = right_candidates.iter().min_by_key(|c| c.priority) {
            if badge_w + 3 + top.width() + trailing_w <= width {
                right_chosen.push(top.clone());
            }
        }
    }

    let right_w: usize = if right_chosen.is_empty() {
        0
    } else {
        right_chosen.iter().map(DockChip::width).sum::<usize>()
            + gutter_w * right_chosen.len().saturating_sub(1)
            + trailing_w
    };

    let badge_gap = if width >= badge_w + 2 { 2usize } else { 0usize };
    let min_center_gap = if right_w > 0 { gutter_w.max(2) } else { 0usize };
    let left_budget = width.saturating_sub(badge_w + badge_gap + min_center_gap + right_w);
    let left_chosen = select_chips_by_priority(&left_candidates, left_budget, gutter_w);

    let left_w: usize = if left_chosen.is_empty() {
        0
    } else {
        left_chosen.iter().map(DockChip::width).sum::<usize>()
            + gutter_w * left_chosen.len().saturating_sub(1)
    };

    let center_gap = width.saturating_sub(badge_w + badge_gap + left_w + right_w);

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut hitboxes: Vec<(u16, u16, A)> = Vec::new();
    let mut cursor_x = 0usize;

    spans.push(Span::styled(
        badge_text.to_string(),
        Style::default()
            .fg(badge_fg)
            .bg(badge_bg)
            .add_modifier(Modifier::BOLD),
    ));
    cursor_x += badge_w;

    if badge_gap > 0 {
        spans.push(Span::raw("  "));
        cursor_x += badge_gap;
    }

    for (idx, chip) in left_chosen.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(gutter_str));
            cursor_x += gutter_w;
        }
        let w = chip.width();
        if let Some(act) = chip.action {
            hitboxes.push((cursor_x as u16, (cursor_x + w) as u16, act));
        }
        chip.push_spans(&mut spans);
        cursor_x += w;
    }

    if center_gap > 0 {
        spans.push(Span::raw(" ".repeat(center_gap)));
        cursor_x += center_gap;
    }

    for (idx, chip) in right_chosen.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(gutter_str));
            cursor_x += gutter_w;
        }
        let w = chip.width();
        if let Some(act) = chip.action {
            hitboxes.push((cursor_x as u16, (cursor_x + w) as u16, act));
        }
        chip.push_spans(&mut spans);
        cursor_x += w;
    }

    if !right_chosen.is_empty() {
        spans.push(Span::raw(" "));
    }

    DockRowPlan {
        line: Line::from(spans),
        hitboxes,
    }
}

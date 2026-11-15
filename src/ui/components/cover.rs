//! Anime cover art rendering.
//!
//! - Ghostty / kitty-protocol terminals → real image via ratatui-image
//! - COSMIC / other terminals           → colored halfblock with per-show unique color

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

/// Dynamically scales and center/upper-biased crops an image so its physical pixel aspect ratio
/// exactly matches the character cell area (`target_cols x target_rows`) for the detected font size.
/// By ensuring pixel dimensions are an exact integer multiple of `(target_cols * font_w, target_rows * font_h)`,
/// `ratatui-image`'s ratio calculation matches identically, guaranteeing 100% frame fill with 0% deadspace.
pub fn scale_to_frame_cover(
    img: image::DynamicImage,
    target_cols: u16,
    target_rows: u16,
    font_size: (u16, u16),
    upper_bias: f32,
) -> image::DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 || target_cols == 0 || target_rows == 0 {
        return img;
    }

    let font_w = (font_size.0 as u32).max(1);
    let font_h = (font_size.1 as u32).max(1);

    let base_pw = target_cols as u32 * font_w;
    let base_ph = target_rows as u32 * font_h;

    let multiplier = (1280 / base_pw).clamp(2, 16);
    let target_w = base_pw * multiplier;
    let target_h = base_ph * multiplier;
    let target_aspect = target_w as f32 / target_h as f32;
    let current_aspect = w as f32 / h as f32;

    if current_aspect < target_aspect {
        let scale = target_w as f32 / w as f32;
        let scaled_h = ((h as f32 * scale).round() as u32).max(target_h);
        let scaled = img.resize(target_w, scaled_h, image::imageops::FilterType::CatmullRom);

        let excess_h = scaled.height().saturating_sub(target_h);
        let top = ((excess_h as f32) * upper_bias).round() as u32;
        scaled.crop_imm(0, top.min(excess_h), target_w, target_h)
    } else {
        let scale = target_h as f32 / h as f32;
        let scaled_w = ((w as f32 * scale).round() as u32).max(target_w);
        let scaled = img.resize(scaled_w, target_h, image::imageops::FilterType::CatmullRom);

        let excess_w = scaled.width().saturating_sub(target_w);
        let left = excess_w / 2;
        scaled.crop_imm(left, 0, target_w, target_h)
    }
}

/// Shared `Resize` mode using CatmullRom instead of ratatui-image's default `FilterType::Nearest`.
pub fn hd_resize_mode() -> ratatui_image::Resize {
    ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::CatmullRom))
}

/// Shared `StatefulImage` widget configured with high-quality CatmullRom downsampling.
pub fn hd_image_widget() -> ratatui_image::StatefulImage {
    ratatui_image::StatefulImage::new(None).resize(hd_resize_mode())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QuadrantCell {
    ch: char,
    fg: Color,
    bg: Color,
}

/// Encode a 2×2 block of sRGB pixels `[TL, TR, BL, BR]` into the perceptually closest
/// Unicode Block Element (`U+2580..U+259F`) with 24-bit foreground and background colors.
fn encode_quadrant_cell(pixels: [[u8; 3]; 4]) -> QuadrantCell {
    const PARTITIONS: &[(char, [bool; 4])] = &[
        ('█', [true, true, true, true]),
        ('▀', [true, true, false, false]),
        ('▌', [true, false, true, false]),
        ('▚', [true, false, false, true]),
        ('▘', [true, false, false, false]),
        ('▝', [false, true, false, false]),
        ('▖', [false, false, true, false]),
        ('▗', [false, false, false, true]),
    ];

    let mut best_err = u64::MAX;
    let mut best = QuadrantCell {
        ch: '█',
        fg: Color::Rgb(pixels[0][0], pixels[0][1], pixels[0][2]),
        bg: Color::Rgb(pixels[0][0], pixels[0][1], pixels[0][2]),
    };

    for &(ch, mask) in PARTITIONS {
        let (mut fg_r, mut fg_g, mut fg_b, mut fg_n) = (0u32, 0u32, 0u32, 0u32);
        let (mut bg_r, mut bg_g, mut bg_b, mut bg_n) = (0u32, 0u32, 0u32, 0u32);
        for i in 0..4 {
            let p = pixels[i];
            if mask[i] {
                fg_r += p[0] as u32;
                fg_g += p[1] as u32;
                fg_b += p[2] as u32;
                fg_n += 1;
            } else {
                bg_r += p[0] as u32;
                bg_g += p[1] as u32;
                bg_b += p[2] as u32;
                bg_n += 1;
            }
        }

        let fg = [
            ((fg_r + fg_n / 2) / fg_n) as u8,
            ((fg_g + fg_n / 2) / fg_n) as u8,
            ((fg_b + fg_n / 2) / fg_n) as u8,
        ];
        let bg = if bg_n == 0 {
            fg
        } else {
            [
                ((bg_r + bg_n / 2) / bg_n) as u8,
                ((bg_g + bg_n / 2) / bg_n) as u8,
                ((bg_b + bg_n / 2) / bg_n) as u8,
            ]
        };

        let mut err = 0u64;
        for i in 0..4 {
            let p = pixels[i];
            let target = if mask[i] { fg } else { bg };
            let dr = (p[0] as i64) - (target[0] as i64);
            let dg = (p[1] as i64) - (target[1] as i64);
            let db = (p[2] as i64) - (target[2] as i64);
            // Perceptual RGB weights (human eye is most sensitive to luminance/green).
            err += (2 * dr * dr + 4 * dg * dg + 3 * db * db) as u64;
        }

        if err < best_err {
            best_err = err;
            best = QuadrantCell {
                ch: if fg == bg { '█' } else { ch },
                fg: Color::Rgb(fg[0], fg[1], fg[2]),
                bg: Color::Rgb(bg[0], bg[1], bg[2]),
            };
            if err == 0 {
                break;
            }
        }
    }

    best
}

/// High-resolution 2×2 Unicode quadrant block protocol for terminals without Kitty/Sixel/iTerm2
/// (e.g. Alacritty, Zellij, SSH). Renders 4 subpixels per cell (`2*W × 2*H`) using Lanczos3
/// resampling — doubling horizontal sharpness compared to standard 1×2 `Halfblocks`.
#[derive(Clone)]
pub struct StatefulQuadrants {
    source: ratatui_image::protocol::ImageSource,
    current_rect: Rect,
    cells: Vec<QuadrantCell>,
    hash: u64,
}

impl StatefulQuadrants {
    pub fn new(source: ratatui_image::protocol::ImageSource) -> Self {
        Self {
            source,
            current_rect: Rect::default(),
            cells: Vec::new(),
            hash: 0,
        }
    }
}

impl StatefulProtocol for StatefulQuadrants {
    fn needs_resize(&mut self, resize: &ratatui_image::Resize, area: Rect) -> Option<Rect> {
        resize.needs_resize(&self.source, self.current_rect, area, false)
    }

    fn resize_encode(
        &mut self,
        resize: &ratatui_image::Resize,
        _background_color: Option<image::Rgb<u8>>,
        area: Rect,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let force = self.source.hash != self.hash;
        let Some(rect) = resize.needs_resize(&self.source, self.current_rect, area, force) else {
            return;
        };
        if rect.width == 0 || rect.height == 0 {
            self.current_rect = rect;
            self.cells.clear();
            self.hash = self.source.hash;
            return;
        }

        let target_w = (rect.width as u32) * 2;
        let target_h = (rect.height as u32) * 2;
        let rgb = self
            .source
            .image
            .resize_exact(target_w, target_h, image::imageops::FilterType::CatmullRom)
            .to_rgb8();

        let mut cells = Vec::with_capacity((rect.width as usize) * (rect.height as usize));
        for cy in 0..rect.height as u32 {
            for cx in 0..rect.width as u32 {
                let p0 = rgb.get_pixel(cx * 2, cy * 2).0;
                let p1 = rgb.get_pixel(cx * 2 + 1, cy * 2).0;
                let p2 = rgb.get_pixel(cx * 2, cy * 2 + 1).0;
                let p3 = rgb.get_pixel(cx * 2 + 1, cy * 2 + 1).0;
                cells.push(encode_quadrant_cell([p0, p1, p2, p3]));
            }
        }

        self.cells = cells;
        self.current_rect = rect;
        self.hash = self.source.hash;
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let w = self.current_rect.width;
        let h = self.current_rect.height;
        if w == 0 || h == 0 || self.cells.is_empty() {
            return;
        }
        let draw_w = w.min(area.width);
        let draw_h = h.min(area.height);
        for y in 0..draw_h {
            let row_offset = (y as usize) * (w as usize);
            for x in 0..draw_w {
                let c = self.cells[row_offset + (x as usize)];
                if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                    cell.set_char(c.ch).set_fg(c.fg).set_bg(c.bg);
                }
            }
        }
    }
}

/// Build a `StatefulProtocol` safely across UI and background worker threads without Kitty ID collisions.
pub fn build_stateful_protocol(
    mut picker: Picker,
    img: image::DynamicImage,
) -> Box<dyn StatefulProtocol> {
    static NEXT_KITTY_IMAGE_ID: std::sync::atomic::AtomicU32 =
        std::sync::atomic::AtomicU32::new(100_000);
    if picker.protocol_type == ratatui_image::picker::ProtocolType::Kitty {
        let id = NEXT_KITTY_IMAGE_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .max(1);
        let source = ratatui_image::protocol::ImageSource::new(img, picker.font_size);
        Box::new(ratatui_image::protocol::kitty::StatefulKitty::new(
            source,
            id,
            picker.is_tmux,
        ))
    } else if picker.protocol_type == ratatui_image::picker::ProtocolType::Halfblocks {
        let source = ratatui_image::protocol::ImageSource::new(img, picker.font_size);
        Box::new(StatefulQuadrants::new(source))
    } else {
        picker.new_resize_protocol(img)
    }
}

// ─── Color generation ─────────────────────────────────────────────────────────

/// Convert HSL to RGB.  h ∈ [0,360), s ∈ [0,1], l ∈ [0,1].
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

/// Deterministic color pair from anime ID.
/// Same ID → same colors every time (no flicker, no storage needed).
pub fn color_from_id(anime_id: i64) -> (Color, Color) {
    // Knuth multiplicative hash → hue in [0, 360)
    let hash    = (anime_id.unsigned_abs().wrapping_mul(2_654_435_761)) as u32;
    let hue     = (hash % 360) as f32;
    let hue2    = (hue + 40.0) % 360.0;

    let (r, g, b)    = hsl_to_rgb(hue,  0.65, 0.55); // primary — vibrant
    let (r2, g2, b2) = hsl_to_rgb(hue2, 0.50, 0.30); // secondary — darker complement

    (Color::Rgb(r, g, b), Color::Rgb(r2, g2, b2))
}

// ─── Halfblock cover widget ───────────────────────────────────────────────────

/// A colored halfblock cover widget for terminals without image protocol support.
/// Renders a two-tone block with the show's abbreviated title centered.
pub struct HalfblockCover<'a> {
    pub anime_id: i64,
    pub title:    &'a str,
}

impl<'a> Widget for HalfblockCover<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let (primary, secondary) = color_from_id(self.anime_id);
        let split                = area.height / 2;
        let title_row            = split.saturating_sub(1);

        for row in 0..area.height {
            let bg = if row < split { primary } else { secondary };

            for col in 0..area.width {
                let x = area.x + col;
                let y = area.y + row;

                if x < buf.area.width && y < buf.area.height {
                    let cell = buf.cell_mut((x, y)).unwrap();
                    cell.set_char('▓');
                    cell.set_style(Style::default().fg(bg).bg(bg));
                }
            }
        }

        // Centered title abbreviation (up to 8 chars)
        let abbrev   = abbreviated_title(self.title, area.width as usize);
        let text_len = abbrev.chars().count() as u16;
        let text_x   = area.x + area.width.saturating_sub(text_len) / 2;
        let text_y   = area.y + title_row;

        if text_y < buf.area.height {
            let (r, g, b) = hsl_to_rgb(0.0, 0.0, 1.0); // white text
            for (i, ch) in abbrev.chars().enumerate() {
                let cx = text_x + i as u16;
                if cx < buf.area.width {
                    let cell = buf.cell_mut((cx, text_y)).unwrap();
                    cell.set_char(ch);
                    cell.set_style(
                        Style::default()
                            .fg(Color::Rgb(r, g, b))
                            .bg(primary),
                    );
                }
            }
        }
    }
}

/// Produce a short title label for the cover (max `width` chars, uppercase initials).
fn abbreviated_title(title: &str, max_width: usize) -> String {
    // Try full title if it fits
    if title.len() <= max_width.saturating_sub(2) {
        return title.to_string();
    }

    // Initials of each word (max 4 letters)
    let initials: String = title
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(4)
        .collect::<String>()
        .to_uppercase();

    initials
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_id_is_deterministic() {
        let (a1, b1) = color_from_id(1535);
        let (a2, b2) = color_from_id(1535);
        assert_eq!(a1, a2);
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_color_from_id_differs_per_show() {
        let (a1, _) = color_from_id(1);
        let (a2, _) = color_from_id(2);
        // Different IDs should (almost certainly) produce different colors
        assert_ne!(a1, a2);
    }

    #[test]
    fn test_color_from_id_handles_zero() {
        // Should not panic
        let _ = color_from_id(0);
    }

    #[test]
    fn test_color_from_id_handles_negative() {
        // Should not panic
        let _ = color_from_id(-999);
    }

    #[test]
    fn test_hsl_to_rgb_red() {
        let (r, g, b) = hsl_to_rgb(0.0, 1.0, 0.5);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_hsl_to_rgb_white() {
        let (r, g, b) = hsl_to_rgb(0.0, 0.0, 1.0);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);
    }

    #[test]
    fn test_hsl_to_rgb_black() {
        let (r, g, b) = hsl_to_rgb(0.0, 0.0, 0.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_abbreviated_title_short_title_unchanged() {
        assert_eq!(abbreviated_title("Naruto", 20), "Naruto");
    }

    #[test]
    fn test_abbreviated_title_long_title_uses_initials() {
        // "Attack on Titan" → "AOT"
        let abbrev = abbreviated_title("Attack on Titan", 6);
        assert_eq!(abbrev, "AOT");
    }

    #[test]
    fn test_abbreviated_title_max_four_initials() {
        let abbrev = abbreviated_title("A Very Long Anime Title With Many Words", 4);
        assert!(abbrev.len() <= 4);
    }

    #[test]
    fn test_abbreviated_title_uppercase() {
        let abbrev = abbreviated_title("fullmetal alchemist brotherhood", 5);
        assert_eq!(abbrev, abbrev.to_uppercase());
    }

    #[test]
    fn test_quadrant_protocol_preserves_vertical_edges_and_2x2_corners() {
        use ratatui::widgets::StatefulWidget;

        // Vertical left/right split -> must pick '▌' (which 1x2 Halfblocks cannot represent).
        let cell_v = encode_quadrant_cell([
            [255, 0, 0],
            [0, 0, 255],
            [255, 0, 0],
            [0, 0, 255],
        ]);
        assert_eq!(cell_v.ch, '▌');
        assert_eq!(cell_v.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell_v.bg, Color::Rgb(0, 0, 255));

        // Single bright top-left subpixel -> must pick '▘'.
        let cell_tl = encode_quadrant_cell([
            [255, 255, 255],
            [10, 10, 10],
            [10, 10, 10],
            [10, 10, 10],
        ]);
        assert_eq!(cell_tl.ch, '▘');
        assert_eq!(cell_tl.fg, Color::Rgb(255, 255, 255));
        assert_eq!(cell_tl.bg, Color::Rgb(10, 10, 10));

        // Full StatefulQuadrants widget render via build_stateful_protocol(Halfblocks).
        let mut picker = Picker::new((10, 20));
        picker.protocol_type = ratatui_image::picker::ProtocolType::Halfblocks;

        // Create an 80x40 image (desired 8x2 cells; fits into 4x1) with a sharp center vertical edge.
        let mut rgb = image::RgbImage::new(80, 40);
        for (x, _y, px) in rgb.enumerate_pixels_mut() {
            *px = if x < 40 {
                image::Rgb([220, 30, 30])
            } else {
                image::Rgb([30, 80, 230])
            };
        }
        let dyn_img = image::DynamicImage::ImageRgb8(rgb);
        let mut proto = build_stateful_protocol(picker, dyn_img);
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        hd_image_widget().render(area, &mut buf, &mut proto);

        // Far-left and far-right cells retain pure red and pure blue colors.
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "█");
        assert_eq!(buf.cell((0, 0)).unwrap().fg, Color::Rgb(220, 30, 30));
        assert_eq!(buf.cell((3, 0)).unwrap().symbol(), "█");
        assert_eq!(buf.cell((3, 0)).unwrap().fg, Color::Rgb(30, 80, 230));

        // Sub-cell vertical edge inside a single 1x1 cell resolves to '▌'.
        let mut picker2 = Picker::new((10, 20));
        picker2.protocol_type = ratatui_image::picker::ProtocolType::Halfblocks;
        let mut edge_img = image::RgbImage::new(20, 20);
        for (x, _y, px) in edge_img.enumerate_pixels_mut() {
            *px = if x < 10 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            };
        }
        let mut edge_proto = build_stateful_protocol(picker2, image::DynamicImage::ImageRgb8(edge_img));
        let edge_area = Rect::new(0, 0, 1, 1);
        let mut edge_buf = Buffer::empty(edge_area);
        hd_image_widget().render(edge_area, &mut edge_buf, &mut edge_proto);
        assert_eq!(edge_buf.cell((0, 0)).unwrap().symbol(), "▌");
    }
}

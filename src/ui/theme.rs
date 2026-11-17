use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub name: &'static str,
    pub primary: Color,      // Active borders, primary titles, focus badges
    pub secondary: Color,    // Complementary tags, badges, search highlights
    pub accent: Color,       // Ratings, episode numbers, live statuses
    pub border_idle: Color,  // Unfocused card frames (subtle, non-distracting)
    pub border_focus: Color, // Focused card frames (glowing, crisp)
    pub text: Color,         // Primary high-contrast text
    pub text_dim: Color,     // Secondary metadata, genres, format
    pub divider: Color,      // Horizontal rail rules and internal separators
}

pub const THEMES: [ThemePalette; 14] = [
    // 0: Anime Violet / Synth (Default for Solus)
    ThemePalette {
        name: "Anime Violet",
        primary: Color::Rgb(189, 147, 249),    // Lavender Purple
        secondary: Color::Rgb(255, 121, 198),  // Neon Pink
        accent: Color::Rgb(251, 191, 36),      // Star Amber
        border_idle: Color::Rgb(55, 45, 75),   // Deep Plum
        border_focus: Color::Rgb(189, 147, 249),
        text: Color::White,
        text_dim: Color::Rgb(165, 150, 190),
        divider: Color::Rgb(75, 58, 100),
    },
    // 1: Tokyo Night (Refined cyan, lilac and amber)
    ThemePalette {
        name: "Tokyo Night",
        primary: Color::Rgb(125, 207, 255),    // Sky Cyan
        secondary: Color::Rgb(187, 154, 247),  // Lilac
        accent: Color::Rgb(224, 175, 104),     // Amber Gold
        border_idle: Color::Rgb(45, 52, 75),   // Muted Navy
        border_focus: Color::Rgb(125, 207, 255),
        text: Color::Rgb(240, 244, 255),
        text_dim: Color::Rgb(130, 140, 175),
        divider: Color::Rgb(55, 62, 90),
    },
    // 2: Cyberpunk Neon (Hot pink and electric cyan)
    ThemePalette {
        name: "Cyberpunk Neon",
        primary: Color::Rgb(255, 0, 127),      // Hot Pink
        secondary: Color::Rgb(0, 240, 255),    // Neon Cyan
        accent: Color::Rgb(255, 230, 0),       // Laser Yellow
        border_idle: Color::Rgb(50, 30, 70),   // Deep Violet
        border_focus: Color::Rgb(255, 0, 127),
        text: Color::White,
        text_dim: Color::Rgb(160, 140, 190),
        divider: Color::Rgb(65, 38, 90),
    },
    // 3: Dracula Night (High contrast purple, cyan and acid green)
    ThemePalette {
        name: "Dracula Night",
        primary: Color::Rgb(189, 147, 249),    // Dracula Purple
        secondary: Color::Rgb(139, 233, 253),  // Dracula Cyan
        accent: Color::Rgb(80, 250, 123),      // Acid Green
        border_idle: Color::Rgb(68, 71, 90),   // Selection Slate
        border_focus: Color::Rgb(189, 147, 249),
        text: Color::Rgb(248, 248, 242),
        text_dim: Color::Rgb(140, 150, 185),
        divider: Color::Rgb(75, 80, 105),
    },
    // 4: Nord Frost (Glacial cyan, polar blue and aurora green)
    ThemePalette {
        name: "Nord Frost",
        primary: Color::Rgb(136, 192, 208),    // Frost Cyan
        secondary: Color::Rgb(129, 161, 193),  // Glacial Blue
        accent: Color::Rgb(163, 190, 140),     // Aurora Green
        border_idle: Color::Rgb(50, 60, 75),   // Polar Dark
        border_focus: Color::Rgb(136, 192, 208),
        text: Color::Rgb(236, 239, 244),
        text_dim: Color::Rgb(145, 160, 180),
        divider: Color::Rgb(65, 78, 100),
    },
    // 5: OLED Platinum (Crisp white and platinum minimalist)
    ThemePalette {
        name: "OLED Platinum",
        primary: Color::Rgb(236, 239, 244),    // Pure Platinum
        secondary: Color::Rgb(190, 200, 215),  // Silver
        accent: Color::Rgb(255, 215, 0),       // Warm Gold
        border_idle: Color::Rgb(50, 55, 65),   // Dark Slate
        border_focus: Color::Rgb(236, 239, 244),
        text: Color::White,
        text_dim: Color::Rgb(140, 145, 160),
        divider: Color::Rgb(70, 75, 85),
    },
    // 6: Lain Wired Y2K (CRT phosphor green, silver static, phosphor amber)
    ThemePalette {
        name: "Lain Wired Y2K",
        primary: Color::Rgb(110, 255, 168),    // CRT Phosphor Green
        secondary: Color::Rgb(185, 224, 210),  // Pale Terminal Static
        accent: Color::Rgb(255, 208, 102),     // Amber Scanline
        border_idle: Color::Rgb(36, 62, 48),   // Dim Cathode Frame
        border_focus: Color::Rgb(110, 255, 168),
        text: Color::Rgb(234, 252, 242),
        text_dim: Color::Rgb(125, 168, 145),
        divider: Color::Rgb(50, 84, 65),
    },
    // 7: Y2K Chrome Ice (Starlight blue-white, periwinkle chrome, metallic glint)
    ThemePalette {
        name: "Y2K Chrome Ice",
        primary: Color::Rgb(178, 222, 255),    // Starlight Ice
        secondary: Color::Rgb(148, 178, 255),  // Frosted Periwinkle
        accent: Color::Rgb(255, 232, 168),     // Warm Halftone Gold
        border_idle: Color::Rgb(46, 60, 88),   // Deep Cobalt Slate
        border_focus: Color::Rgb(178, 222, 255),
        text: Color::Rgb(242, 248, 255),
        text_dim: Color::Rgb(142, 162, 198),
        divider: Color::Rgb(62, 78, 110),
    },
    // 8: EVA Unit-01 (Obsidian NERV frame, LCL tangerine, bio-lime)
    ThemePalette {
        name: "EVA Unit-01",
        primary: Color::Rgb(128, 255, 114),    // AT-Field Bio-Lime
        secondary: Color::Rgb(255, 148, 64),   // LCL Warning Orange
        accent: Color::Rgb(255, 224, 92),      // Sync Telemetry Yellow
        border_idle: Color::Rgb(58, 44, 78),   // NERV Armor Slate
        border_focus: Color::Rgb(128, 255, 114),
        text: Color::Rgb(244, 245, 250),
        text_dim: Color::Rgb(155, 148, 176),
        divider: Color::Rgb(74, 58, 96),
    },
    // 9: Akira Neo-Tokyo (Ashen midnight, Kaneda crimson red, steel cyan)
    ThemePalette {
        name: "Akira Neo-Tokyo",
        primary: Color::Rgb(255, 78, 92),      // Kaneda Vermilion Red
        secondary: Color::Rgb(125, 214, 255),  // Neon Expressway Cyan
        accent: Color::Rgb(255, 210, 80),      // Capsule Amber
        border_idle: Color::Rgb(72, 42, 48),   // Dark Tarmac Iron
        border_focus: Color::Rgb(255, 78, 92),
        text: Color::Rgb(248, 244, 244),
        text_dim: Color::Rgb(168, 142, 148),
        divider: Color::Rgb(92, 54, 60),
    },
    // 10: Ghost in the Shell (Section-9 cyber teal, matrix emerald, cyber gold)
    ThemePalette {
        name: "Ghost in the Shell",
        primary: Color::Rgb(72, 236, 218),     // Section-9 Aqua
        secondary: Color::Rgb(98, 255, 168),   // Cybernetic Emerald
        accent: Color::Rgb(255, 204, 102),     // Hologram Gold
        border_idle: Color::Rgb(36, 64, 74),   // Deep Optical Navy
        border_focus: Color::Rgb(72, 236, 218),
        text: Color::Rgb(234, 249, 250),
        text_dim: Color::Rgb(128, 166, 176),
        divider: Color::Rgb(48, 82, 92),
    },
    // 11: Monogatari Crimson (Sumi ink, crimson rose, warm champagne parchment)
    ThemePalette {
        name: "Monogatari Crimson",
        primary: Color::Rgb(242, 95, 132),     // Crimson Rose
        secondary: Color::Rgb(244, 212, 166),  // Champagne Parchment
        accent: Color::Rgb(255, 198, 92),      // Shrine Lantern Gold
        border_idle: Color::Rgb(72, 44, 54),   // Ink Burgundy
        border_focus: Color::Rgb(242, 95, 132),
        text: Color::Rgb(252, 244, 246),
        text_dim: Color::Rgb(176, 145, 155),
        divider: Color::Rgb(92, 56, 68),
    },
    // 12: Catppuccin Mocha (Espresso dark, pastel mauve, sapphire, peach)
    ThemePalette {
        name: "Catppuccin Mocha",
        primary: Color::Rgb(203, 166, 247),    // Mocha Mauve
        secondary: Color::Rgb(116, 199, 236),  // Mocha Sapphire
        accent: Color::Rgb(250, 179, 135),     // Warm Peach
        border_idle: Color::Rgb(69, 71, 90),   // Surface1
        border_focus: Color::Rgb(203, 166, 247),
        text: Color::Rgb(205, 214, 244),
        text_dim: Color::Rgb(147, 153, 178),
        divider: Color::Rgb(88, 91, 112),
    },
    // 13: Gruvbox Dark (Warm charcoal, burnt tangerine, sage aqua, ochre)
    ThemePalette {
        name: "Gruvbox Dark",
        primary: Color::Rgb(254, 128, 25),     // Bright Orange
        secondary: Color::Rgb(142, 192, 124),  // Studio Aqua
        accent: Color::Rgb(250, 189, 47),      // Yellow Ochre
        border_idle: Color::Rgb(80, 73, 69),   // Dark2 Charcoal
        border_focus: Color::Rgb(254, 128, 25),
        text: Color::Rgb(235, 219, 178),
        text_dim: Color::Rgb(168, 153, 132),
        divider: Color::Rgb(102, 92, 84),
    },
];

pub fn get_theme(index: usize) -> &'static ThemePalette {
    &THEMES[index % THEMES.len()]
}

/// Dynamically extract an ambilight theme palette from an anime's cover art.
///
/// Samples image pixels in HSV color space to identify dominant vibrant colors,
/// boosting luminance and saturation so the terminal borders and highlights
/// vividly harmonize with the anime currently in focus.
pub fn extract_ambilight_palette(img: &image::DynamicImage, base: &ThemePalette) -> ThemePalette {
    use image::GenericImageView;

    let thumb = img.thumbnail(48, 48);
    let mut hue_bins = [0.0f32; 12];
    let mut bin_rgb_sums = [(0.0f32, 0.0f32, 0.0f32); 12];

    for (_, _, pixel) in thumb.pixels() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        // Skip near-black or very dark pixels
        if max < 0.16 {
            continue;
        }

        let sat = if max == 0.0 { 0.0 } else { delta / max };
        // Skip desaturated grays/whites
        if sat < 0.18 || (max > 0.90 && sat < 0.15) {
            continue;
        }

        let hue = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta).rem_euclid(6.0))
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        let bin_idx = ((hue / 30.0) as usize) % 12;
        let weight = sat * (0.35 + 0.65 * max);

        hue_bins[bin_idx] += weight;
        bin_rgb_sums[bin_idx].0 += r * weight;
        bin_rgb_sums[bin_idx].1 += g * weight;
        bin_rgb_sums[bin_idx].2 += b * weight;
    }

    // Find the most dominant hue bin
    let mut best_bin = 0;
    let mut best_weight = 0.0f32;
    for (i, &w) in hue_bins.iter().enumerate() {
        if w > best_weight {
            best_weight = w;
            best_bin = i;
        }
    }

    if best_weight <= 0.05 {
        // Not enough color information; keep base theme
        return *base;
    }

    let p_sum = bin_rgb_sums[best_bin];
    let avg_r = (p_sum.0 / best_weight).clamp(0.0, 1.0);
    let avg_g = (p_sum.1 / best_weight).clamp(0.0, 1.0);
    let avg_b = (p_sum.2 / best_weight).clamp(0.0, 1.0);

    // Boost primary for high vibrancy on dark terminal background
    let (pr, pg, pb) = boost_vibrancy(avg_r, avg_g, avg_b);

    // Find secondary bin with at least 60 degrees hue separation
    let mut second_bin = (best_bin + 6) % 12;
    let mut second_weight = 0.0f32;
    for (i, &w) in hue_bins.iter().enumerate() {
        let diff = (i as isize - best_bin as isize).abs();
        let dist = diff.min(12 - diff);
        if dist >= 2 && w > second_weight {
            second_weight = w;
            second_bin = i;
        }
    }

    let (sr, sg, sb) = if second_weight > 0.05 {
        let s_sum = bin_rgb_sums[second_bin];
        let sr = (s_sum.0 / second_weight).clamp(0.0, 1.0);
        let sg = (s_sum.1 / second_weight).clamp(0.0, 1.0);
        let sb = (s_sum.2 / second_weight).clamp(0.0, 1.0);
        boost_vibrancy(sr, sg, sb)
    } else {
        // Fallback: rotate primary hue by ~150 degrees
        ((255 - pr).clamp(80, 240), (255 - pg).clamp(100, 240), (255 - pb).clamp(120, 255))
    };

    let accent = Color::Rgb(251, 191, 36); // Crisp golden amber for ratings & counters
    let border_focus = Color::Rgb(pr, pg, pb);
    let primary = Color::Rgb(pr, pg, pb);
    let secondary = Color::Rgb(sr, sg, sb);
    let border_idle = Color::Rgb(
        (pr / 5).saturating_add(30),
        (pg / 5).saturating_add(30),
        (pb / 5).saturating_add(45),
    );
    let divider = Color::Rgb(
        (pr / 6).saturating_add(40),
        (pg / 6).saturating_add(40),
        (pb / 6).saturating_add(55),
    );
    let text = Color::Rgb(248, 248, 252);
    let text_dim = Color::Rgb(
        (pr / 5).saturating_add(150),
        (pg / 5).saturating_add(150),
        (pb / 5).saturating_add(170),
    );

    ThemePalette {
        name: "Ambilight",
        primary,
        secondary,
        accent,
        border_idle,
        border_focus,
        text,
        text_dim,
        divider,
    }
}

fn boost_vibrancy(r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let target_val = 0.88f32;
    let target_sat = 0.70f32;

    if delta == 0.0 {
        return (200, 200, 220);
    }

    let sat = delta / max;
    let sat_mult = if sat < target_sat { target_sat / sat.max(0.05) } else { 1.0 };
    let val_mult = if max < target_val { target_val / max.max(0.05) } else { 1.0 };

    let mid = (r + g + b) / 3.0;
    let nr = ((mid + (r - mid) * sat_mult) * val_mult).clamp(0.0, 1.0);
    let ng = ((mid + (g - mid) * sat_mult) * val_mult).clamp(0.0, 1.0);
    let nb = ((mid + (b - mid) * sat_mult) * val_mult).clamp(0.0, 1.0);

    ((nr * 255.0) as u8, (ng * 255.0) as u8, (nb * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ambilight_from_vibrant_red_image() {
        let mut img = image::RgbImage::new(48, 48);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb([220, 30, 40]);
        }
        let dyn_img = image::DynamicImage::ImageRgb8(img);
        let base = get_theme(0);
        let theme = extract_ambilight_palette(&dyn_img, base);
        assert_eq!(theme.name, "Ambilight");
        if let Color::Rgb(r, g, b) = theme.primary {
            assert!(r > g && r > b, "Red should dominate");
        } else {
            panic!("Expected RGB color");
        }
    }

    #[test]
    fn test_extract_ambilight_monochrome_keeps_base() {
        let mut img = image::RgbImage::new(48, 48);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb([10, 10, 10]);
        }
        let dyn_img = image::DynamicImage::ImageRgb8(img);
        let base = get_theme(0);
        let theme = extract_ambilight_palette(&dyn_img, base);
        assert_eq!(theme.name, base.name);
    }
}

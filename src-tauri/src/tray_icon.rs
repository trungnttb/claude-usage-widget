//! Draws the tray icon.
//!
//! On Windows `TrayIcon::set_title` does nothing, so the only way to put a
//! number in the notification area is to draw it into the icon itself. A 5x7
//! bitmap font is used rather than a font crate: at this size a rasterized
//! typeface is blurrier than hand-placed pixels, and it keeps a megabyte of
//! font machinery out of a binary that exists to show two digits.

pub const SIZE: u32 = 32;

/// The same three-step scale the widget uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Danger,
}

impl Level {
    pub fn from_percent(percent: f64) -> Self {
        if percent >= 80.0 {
            Self::Danger
        } else if percent >= 50.0 {
            Self::Warn
        } else {
            Self::Ok
        }
    }

    fn rgb(self) -> [u8; 3] {
        match self {
            Self::Ok => [0x4a, 0xde, 0x80],
            Self::Warn => [0xfb, 0xbf, 0x24],
            Self::Danger => [0xf8, 0x71, 0x71],
        }
    }
}

const GLYPH_WIDTH: usize = 5;
const GLYPH_HEIGHT: usize = 7;

/// Each row is five bits, most significant bit leftmost.
fn glyph(character: char) -> Option<[u8; GLYPH_HEIGHT]> {
    let rows = match character {
        '0' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        '%' => [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
        '$' => [
            0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100,
        ],
        'k' => [
            0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b01110, 0b00000, 0b00000, 0b00000,
        ],
        _ => return None,
    };
    Some(rows)
}

/// Renders `text` centred, as RGBA bytes for a `SIZE` by `SIZE` image.
///
/// The background stays transparent so the icon suits a light or dark taskbar
/// without knowing which one it is on.
pub fn render(text: &str, level: Level) -> Vec<u8> {
    render_rows(&[(text, level)])
}

/// Renders one or two lines of text, each in its own colour.
///
/// Two lines is how the session and weekly figures share one icon. They are
/// stacked rather than placed side by side because a tray icon is square:
/// side by side would halve the glyph size, stacked only halves the height,
/// which the 5x7 font can still afford.
pub fn render_rows(rows: &[(&str, Level)]) -> Vec<u8> {
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
    let lines: Vec<(Vec<[u8; GLYPH_HEIGHT]>, Level)> = rows
        .iter()
        .map(|(text, level)| (text.chars().filter_map(glyph).collect(), *level))
        .filter(|(glyphs, _): &(Vec<_>, Level)| !glyphs.is_empty())
        .collect();
    if lines.is_empty() {
        return pixels;
    }

    /// Blank pixels between stacked lines.
    const LINE_GAP: usize = 2;
    let count = lines.len();
    let widest = lines.iter().map(|(g, _)| g.len()).max().unwrap_or(1);

    // Largest whole-pixel scale that still fits, so edges stay sharp. Spacing
    // between glyphs is measured separately and takes whatever is left over:
    // scaling it with the glyphs costs a whole step of size at three
    // characters, which is the difference between legible and not.
    let vertical_room = (SIZE as usize).saturating_sub(LINE_GAP * (count - 1));
    let scale = (SIZE as usize / (widest * GLYPH_WIDTH))
        .min(vertical_room / (GLYPH_HEIGHT * count))
        .max(1);

    let line_height = GLYPH_HEIGHT * scale;
    let block_height = line_height * count + LINE_GAP * (count - 1);
    let mut top = (SIZE as usize).saturating_sub(block_height) / 2;

    for (glyphs, level) in &lines {
        draw_line(&mut pixels, glyphs, *level, scale, top);
        top += line_height + LINE_GAP;
    }
    pixels
}

fn draw_line(
    pixels: &mut [u8],
    glyphs: &[[u8; GLYPH_HEIGHT]],
    level: Level,
    scale: usize,
    top: usize,
) {
    let count = glyphs.len();
    let tracking = if count > 1 {
        let spare = (SIZE as usize).saturating_sub(count * GLYPH_WIDTH * scale);
        scale.min(spare / (count - 1))
    } else {
        0
    };
    let drawn_width = count * GLYPH_WIDTH * scale + (count - 1) * tracking;
    let left = (SIZE as usize).saturating_sub(drawn_width) / 2;
    let [r, g, b] = level.rgb();

    for (index, rows) in glyphs.iter().enumerate() {
        let glyph_left = left + index * (GLYPH_WIDTH * scale + tracking);
        for (row, bits) in rows.iter().enumerate() {
            for column in 0..GLYPH_WIDTH {
                if bits & (1 << (GLYPH_WIDTH - 1 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = glyph_left + column * scale + dx;
                        let y = top + row * scale + dy;
                        if x >= SIZE as usize || y >= SIZE as usize {
                            continue;
                        }
                        let offset = (y * SIZE as usize + x) * 4;
                        pixels[offset] = r;
                        pixels[offset + 1] = g;
                        pixels[offset + 2] = b;
                        pixels[offset + 3] = 0xff;
                    }
                }
            }
        }
    }
}

/// Size of the badge overlaid on the Windows taskbar button.
pub const BADGE_SIZE: u32 = 16;

/// A filled circle in the level colour, for the taskbar overlay.
///
/// Text is not an option at this size — the overlay is drawn at about a
/// quarter of the button — so the badge carries only the colour.
pub fn render_badge(level: Level) -> Vec<u8> {
    let size = BADGE_SIZE as usize;
    let mut pixels = vec![0u8; size * size * 4];
    let [r, g, b] = level.rgb();
    let centre = (size as f64 - 1.0) / 2.0;
    let radius = size as f64 / 2.0 - 0.5;

    for y in 0..size {
        for x in 0..size {
            let distance = ((x as f64 - centre).powi(2) + (y as f64 - centre).powi(2)).sqrt();
            // One pixel of falloff at the edge, so the circle is not jagged.
            let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let offset = (y * size + x) * 4;
            pixels[offset] = r;
            pixels[offset + 1] = g;
            pixels[offset + 2] = b;
            pixels[offset + 3] = (coverage * 255.0) as u8;
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit_pixels(pixels: &[u8]) -> usize {
        pixels.chunks(4).filter(|px| px[3] > 0).count()
    }

    #[test]
    fn the_image_is_the_size_the_tray_expects() {
        let pixels = render("71", Level::Warn);
        assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);
    }

    #[test]
    fn digits_are_drawn() {
        assert!(lit_pixels(&render("71", Level::Ok)) > 40);
    }

    /// A transparent background lets one icon suit a light or dark taskbar.
    #[test]
    fn the_background_stays_transparent() {
        let pixels = render("8", Level::Ok);
        let total = (SIZE * SIZE) as usize;
        assert!(lit_pixels(&pixels) < total / 2);
        assert_eq!(pixels[0..4], [0, 0, 0, 0]);
    }

    #[test]
    fn the_colour_follows_the_level() {
        for (level, expected) in [
            (Level::Ok, [0x4a, 0xde, 0x80]),
            (Level::Warn, [0xfb, 0xbf, 0x24]),
            (Level::Danger, [0xf8, 0x71, 0x71]),
        ] {
            let pixels = render("8", level);
            let first = pixels
                .chunks(4)
                .find(|px| px[3] > 0)
                .expect("something drawn");
            assert_eq!(&first[0..3], &expected, "{level:?}");
        }
    }

    #[test]
    fn levels_follow_the_same_thresholds_as_the_widget() {
        assert_eq!(Level::from_percent(49.9), Level::Ok);
        assert_eq!(Level::from_percent(50.0), Level::Warn);
        assert_eq!(Level::from_percent(79.9), Level::Warn);
        assert_eq!(Level::from_percent(80.0), Level::Danger);
    }

    /// Longer text must shrink rather than spill outside the icon.
    #[test]
    fn text_that_does_not_fit_is_scaled_down() {
        for text in ["7", "71", "100", "$1.2k"] {
            let pixels = render(text, Level::Ok);
            assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize, "{text}");
            assert!(lit_pixels(&pixels) > 0, "{text}");
        }
    }

    /// Height of the drawn glyphs, in pixels.
    fn drawn_height(pixels: &[u8]) -> usize {
        let rows: Vec<usize> = (0..SIZE as usize)
            .filter(|y| (0..SIZE as usize).any(|x| pixels[(y * SIZE as usize + x) * 4 + 3] > 0))
            .collect();
        match (rows.first(), rows.last()) {
            (Some(first), Some(last)) => last - first + 1,
            _ => 0,
        }
    }

    /// A tray icon is about 16 logical pixels tall on screen, so a glyph drawn
    /// at 7 pixels of a 32 pixel image is not readable. Up to three characters
    /// must come out at double size or better.
    #[test]
    fn up_to_three_characters_stay_legible() {
        for text in ["7", "71", "100"] {
            assert!(
                drawn_height(&render(text, Level::Ok)) >= GLYPH_HEIGHT * 2,
                "{text} drew too small"
            );
        }
    }

    #[test]
    fn nothing_is_drawn_outside_the_icon() {
        for text in ["7", "71", "100", "2.5M"] {
            let pixels = render(text, Level::Ok);
            assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize, "{text}");
            assert!(drawn_height(&pixels) <= SIZE as usize, "{text}");
        }
    }

    #[test]
    fn unknown_characters_are_skipped_rather_than_drawn_as_blanks() {
        assert_eq!(
            lit_pixels(&render("7@1", Level::Ok)),
            lit_pixels(&render("71", Level::Ok))
        );
    }

    /// Prints the icons as text so a person can check they are legible.
    /// `cargo test -- --ignored --nocapture preview`
    #[test]
    #[ignore = "visual check, prints rather than asserts"]
    fn preview() {
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("mot dong: 71", render("71", Level::Warn)),
            ("mot dong: 100", render("100", Level::Danger)),
            (
                "hai dong: 71 tren, 27 duoi",
                render_rows(&[("71", Level::Warn), ("27", Level::Ok)]),
            ),
            (
                "hai dong: 100 tren, 8 duoi",
                render_rows(&[("100", Level::Danger), ("8", Level::Ok)]),
            ),
        ];
        for (text, pixels) in cases {
            println!("\n{text}");
            for y in 0..SIZE as usize {
                let row: String = (0..SIZE as usize)
                    .map(|x| {
                        if pixels[(y * SIZE as usize + x) * 4 + 3] > 0 {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect();
                println!("{row}");
            }
        }
    }

    #[test]
    fn the_badge_is_a_filled_circle_in_the_level_colour() {
        let size = BADGE_SIZE as usize;
        let pixels = render_badge(Level::Danger);
        assert_eq!(pixels.len(), size * size * 4);

        let centre = ((size / 2) * size + size / 2) * 4;
        assert_eq!(&pixels[centre..centre + 3], &[0xf8, 0x71, 0x71]);
        assert_eq!(pixels[centre + 3], 0xff);

        // Corners fall outside the circle, so the badge reads as round.
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn empty_text_draws_nothing_rather_than_panicking() {
        assert_eq!(lit_pixels(&render("", Level::Ok)), 0);
    }
}

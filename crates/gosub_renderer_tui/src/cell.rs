use unicode_width::UnicodeWidthChar;

/// Upper bounds on the canvas, so a stray coordinate can't turn into a multi-gigabyte allocation.
const MAX_COLS: usize = 2_000;
const MAX_ROWS: usize = 20_000;

/// One character cell: the glyph, its foreground colour, and the background painted behind it.
/// `bg: None` means nothing painted there, so the terminal's own background shows through.
/// No attributes (bold/italic/underline) yet — this is the skeleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: (u8, u8, u8),
    pub bg: Option<(u8, u8, u8)>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: (0xd0, 0xd0, 0xd0),
            bg: None,
        }
    }
}

/// A page-sized grid of character cells, addressed in page space (cell 0,0 is the top-left of the
/// document, not of the viewport). Rows grow on demand, so an unknown page height costs nothing
/// up front.
#[derive(Debug, Default)]
pub struct CellCanvas {
    rows: Vec<Vec<Cell>>,
}

impl CellCanvas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Number of rows that currently hold content.
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// The cell at (`col`, `row`), or `None` where nothing was ever painted.
    ///
    /// Rows are ragged — a short line allocates only the cells it used — so this returns `None`
    /// past the end of a row as well as past the end of the page.
    pub fn cell(&self, col: usize, row: usize) -> Option<Cell> {
        self.rows.get(row)?.get(col).copied()
    }

    /// Write `ch` at (`col`, `row`), preserving whatever background was painted there.
    /// Out-of-bounds writes past the canvas limits are dropped.
    pub fn set(&mut self, col: usize, row: usize, ch: char, fg: (u8, u8, u8)) {
        if let Some(slot) = self.slot(col, row) {
            slot.ch = ch;
            slot.fg = fg;
        }
    }

    /// Paint `bg` behind the cells covering `[col0, col1) × [row0, row1)`, leaving their glyphs
    /// alone.
    ///
    /// Glyphs survive deliberately. A rectangle painted after text would, in a pixel rasterizer,
    /// legitimately cover it — but cell coordinates are rounded, so a rect that merely *abuts*
    /// text in CSS pixels can overlap it once quantized. Erasing on overlap would silently eat
    /// text; recolouring behind it cannot.
    pub fn fill_bg(&mut self, col0: usize, row0: usize, col1: usize, row1: usize, bg: (u8, u8, u8)) {
        for row in row0..row1.min(MAX_ROWS) {
            for col in col0..col1.min(MAX_COLS) {
                if let Some(slot) = self.slot(col, row) {
                    slot.bg = Some(bg);
                }
            }
        }
    }

    /// Borrow the cell at (`col`, `row`), growing the grid to reach it. `None` past the limits.
    fn slot(&mut self, col: usize, row: usize) -> Option<&mut Cell> {
        if col >= MAX_COLS || row >= MAX_ROWS {
            return None;
        }
        if row >= self.rows.len() {
            self.rows.resize_with(row + 1, Vec::new);
        }
        let line = self.rows.get_mut(row)?;
        if col >= line.len() {
            line.resize(col + 1, Cell::default());
        }
        line.get_mut(col)
    }

    /// Render rows `[start, start + count)` as ANSI truecolor lines, adapted to `background`.
    ///
    /// Wide characters (CJK) occupy two cells; the second is written as a `\0` filler by
    /// [`crate::rasterizer`], and is skipped here so the terminal's own advance stays in step.
    pub fn to_ansi(&self, start: usize, count: usize, background: Background) -> String {
        let mut out = String::new();
        for row in start..start.saturating_add(count) {
            let Some(line) = self.rows.get(row) else {
                out.push('\n');
                continue;
            };
            type Style = ((u8, u8, u8), Option<(u8, u8, u8)>);
            let mut last: Option<Style> = None;
            for cell in line {
                if cell.ch == '\0' {
                    continue;
                }
                let style: Style = (background.adapt(cell.fg), cell.bg.map(|c| background.adapt(c)));
                if last != Some(style) {
                    let ((r, g, b), bg) = style;
                    out.push_str(&format!("\x1b[38;2;{r};{g};{b}m"));
                    match bg {
                        Some((r, g, b)) => out.push_str(&format!("\x1b[48;2;{r};{g};{b}m")),
                        // Nothing painted here: fall back to the terminal's own background.
                        None => out.push_str("\x1b[49m"),
                    }
                    last = Some(style);
                }
                out.push(cell.ch);
            }
            out.push_str("\x1b[0m\n");
        }
        out
    }
}

/// What the terminal's own background looks like.
///
/// The canvas stores the page's true CSS colours; this decides how they're presented. It matters
/// because the skeleton doesn't paint backgrounds: a page that sets black-on-white renders as
/// black-on-whatever-your-terminal-is, which on a dark terminal is invisible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Background {
    /// Emit CSS colours unchanged — correct when the terminal is light, as most pages assume.
    Light,
    /// Invert lightness, keeping hue and saturation, so the page's own palette stays recognisable:
    /// black body text becomes white, `#334488` links become a lighter blue, mid-greys barely move.
    #[default]
    Dark,
}

impl Background {
    /// Map a page colour onto something legible against this terminal background.
    pub fn adapt(self, fg: (u8, u8, u8)) -> (u8, u8, u8) {
        match self {
            Background::Light => fg,
            Background::Dark => invert_lightness(fg),
        }
    }
}

/// Flip a colour's HSL lightness (`l` → `1 - l`), leaving hue and saturation alone.
///
/// Chosen over a flat "brighten dark colours" rule because it is reversible and hue-preserving:
/// the relative relationships in a page's palette survive, so a link still reads as a link.
fn invert_lightness((r, g, b): (u8, u8, u8)) -> (u8, u8, u8) {
    let (r, g, b) = (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    // Greys have no hue to preserve; just flip them.
    let delta = max - min;
    if delta <= f32::EPSILON {
        let v = to_u8(1.0 - l);
        return (v, v, v);
    }

    let s = if l > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    } / 6.0;

    let (r, g, b) = hsl_to_rgb(h, s, 1.0 - l);
    (to_u8(r), to_u8(g), to_u8(b))
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match (h * 6.0) as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Width of `c` in character cells. Zero-width combining marks report 0; CJK reports 2.
pub fn char_cells(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Width of `s` in character cells.
pub fn str_cells(s: &str) -> usize {
    s.chars().map(char_cells).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_background_makes_black_body_text_readable() {
        // The whole point: pages set `color: #000`, and the skeleton paints no background.
        assert_eq!(Background::Dark.adapt((0x00, 0x00, 0x00)), (0xff, 0xff, 0xff));
    }

    #[test]
    fn dark_background_keeps_link_hue() {
        // HN's link blue: should lighten, but still be blue (b > g > r).
        let (r, g, b) = Background::Dark.adapt((0x33, 0x44, 0x88));
        assert!(b > g && g > r, "hue not preserved: {r:02x}{g:02x}{b:02x}");
        assert!(b > 0x88, "link should be lighter than the original, got {b:02x}");
    }

    #[test]
    fn dark_background_barely_moves_mid_grey() {
        // Lightness 0.51 inverts to 0.49 — a colour this close to the middle should stay put,
        // staying legible against either background rather than flipping to the other extreme.
        let (r, g, b) = Background::Dark.adapt((0x82, 0x82, 0x82));
        assert_eq!((r, g, b), (0x7d, 0x7d, 0x7d));
    }

    #[test]
    fn light_background_is_a_passthrough() {
        assert_eq!(Background::Light.adapt((0x33, 0x44, 0x88)), (0x33, 0x44, 0x88));
    }

    #[test]
    fn inverting_lightness_twice_round_trips() {
        for c in [(0x33, 0x44, 0x88), (0x12, 0xab, 0x5e), (0xff, 0x00, 0x00)] {
            let there_and_back = invert_lightness(invert_lightness(c));
            let (r, g, b) = there_and_back;
            assert!(
                r.abs_diff(c.0) <= 1 && g.abs_diff(c.1) <= 1 && b.abs_diff(c.2) <= 1,
                "{c:?} round-tripped to {there_and_back:?}"
            );
        }
    }
}

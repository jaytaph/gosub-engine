use std::sync::Arc;

use gosub_interface::font_system::FontSystem;
use gosub_render_pipeline::common::geo::Rect;
use gosub_render_pipeline::common::media::MediaStore;
use gosub_render_pipeline::common::texture::TextureId;
use gosub_render_pipeline::common::texture_store::TextureStore;
use gosub_render_pipeline::painter::commands::border::BorderStyle;
use gosub_render_pipeline::painter::commands::brush::Brush;
use gosub_render_pipeline::painter::commands::rectangle::Rectangle;
use gosub_render_pipeline::painter::commands::text::Text;
use gosub_render_pipeline::painter::commands::PaintCommand;
use gosub_render_pipeline::rasterizer::{PassScope, Rasterable};
use gosub_render_pipeline::tiler::Tile;
use parking_lot::Mutex;

use crate::cell::{char_cells, CellCanvas};
use crate::font::{wrap_cells, CELL_H, CELL_W};

/// Default foreground for text with a non-solid brush (a gradient or image fill we can't express).
const DEFAULT_FG: (u8, u8, u8) = (0xd0, 0xd0, 0xd0);

/// Draws paint commands into a [`CellCanvas`] instead of a pixel buffer.
///
/// Deviates from the pixel rasterizers in one visible way: it returns `None` from `rasterize`, so
/// every tile ends up `TileState::Empty` and the engine's `TileCache` handle carries no tiles. The
/// canvas is the real output and the host reads it directly. That is deliberate — the tile and
/// compositing machinery exists to amortize pixel cost that doesn't exist at 80×24, and its host
/// path is `u32`-per-pixel anyway (`TileTarget::buf: &mut [u32]`), which a cell can't fit in.
pub struct TuiRasterizer {
    canvas: Arc<Mutex<CellCanvas>>,
    font_system: Arc<Mutex<dyn FontSystem>>,
}

impl TuiRasterizer {
    pub fn new(canvas: Arc<Mutex<CellCanvas>>, font_system: Arc<Mutex<dyn FontSystem>>) -> Self {
        Self { canvas, font_system }
    }
}

impl Rasterable for TuiRasterizer {
    fn begin_pass(&self, scope: PassScope) {
        // The canvas is one shared surface, not per-tile pixel buffers, so nothing else discards
        // last pass's output. Drop it when the whole page is about to be redrawn; keep it on an
        // incremental pass, where the tiles that aren't dirty will never be drawn again.
        if scope == PassScope::FullRebuild {
            self.canvas.lock().clear();
        }
    }

    fn rasterize(
        &self,
        tile: &Tile,
        _texture_store: &mut TextureStore,
        _media_store: &MediaStore,
    ) -> Option<TextureId> {
        let mut canvas = self.canvas.lock();

        // Clear to the page background first, exactly as the pixel rasterizers do — `Tile::bgcolor`
        // carries the canvas-wide colour lifted off <html>/<body> in stage 4.
        if let Some((r, g, b, a)) = tile.bgcolor {
            if a >= ALPHA_CUTOFF {
                fill_rect(&mut canvas, tile.rect, (to_u8(r), to_u8(g), to_u8(b)));
            }
        }

        for element in &tile.elements {
            for command in &element.paint_commands {
                // Commands are in paint order, so this is a painter's algorithm over cells.
                // Svg and image brushes are still skipped; PushLayer/PopLayer never reach the
                // tile path at all.
                match command {
                    PaintCommand::Text(text) => draw_text(&mut canvas, text),
                    PaintCommand::Rectangle(rect) => draw_rectangle(&mut canvas, rect, tile.rect),
                    _ => {}
                }
            }
        }

        None
    }

    fn font_system(&self) -> Option<Arc<Mutex<dyn FontSystem>>> {
        // Hands the cell metrics to the layouter. Returning `None` here would silently fall back
        // to a proportional font system for layout and the grid alignment would be lost.
        Some(self.font_system.clone())
    }
}

/// Below this alpha a fill is treated as absent. A cell holds one opaque colour, so there is no
/// way to blend: the choice is paint it solid or skip it, and painting a faint overlay solid would
/// obliterate the page behind it.
const ALPHA_CUTOFF: f32 = 0.5;

/// Paint a rectangle's background and border into the canvas. Border-radius is ignored — a cell
/// grid has no sub-cell geometry to round.
fn draw_rectangle(canvas: &mut CellCanvas, rectangle: &Rectangle, tile_rect: Rect) {
    if let Some(Brush::Solid(color)) = rectangle.background() {
        if color.a() >= ALPHA_CUTOFF {
            if let Some(clipped) = intersect(rectangle.rect(), tile_rect) {
                fill_rect(canvas, clipped, (to_u8(color.r()), to_u8(color.g()), to_u8(color.b())));
            }
        }
    }
    draw_border(canvas, rectangle, tile_rect);
}

/// Box-drawing characters for one border style.
struct BoxGlyphs {
    h: char,
    v: char,
    tl: char,
    tr: char,
    bl: char,
    br: char,
}

/// Map a CSS border style onto box-drawing characters.
///
/// The 3D styles (groove/ridge/inset/outset) rely on shading two edges differently, which a single
/// character can't express — they fall back to a solid line.
fn glyphs_for(style: &BorderStyle) -> BoxGlyphs {
    match style {
        BorderStyle::Double => BoxGlyphs {
            h: '═',
            v: '║',
            tl: '╔',
            tr: '╗',
            bl: '╚',
            br: '╝',
        },
        BorderStyle::Dashed => BoxGlyphs {
            h: '╌',
            v: '╎',
            tl: '┌',
            tr: '┐',
            bl: '└',
            br: '┘',
        },
        BorderStyle::Dotted => BoxGlyphs {
            h: '┈',
            v: '┊',
            tl: '┌',
            tr: '┐',
            bl: '└',
            br: '┘',
        },
        _ => BoxGlyphs {
            h: '─',
            v: '│',
            tl: '┌',
            tr: '┐',
            bl: '└',
            br: '┘',
        },
    }
}

/// The colour a border side paints with, or `None` when that side draws nothing.
fn side_color(width: f32, style: &BorderStyle, brush: &Brush) -> Option<(u8, u8, u8)> {
    if width <= 0.0 || style.is_invisible() {
        return None;
    }
    match brush {
        Brush::Solid(c) if c.a() >= ALPHA_CUTOFF => Some((to_u8(c.r()), to_u8(c.g()), to_u8(c.b()))),
        _ => None,
    }
}

/// Stroke a rectangle's border as box-drawing characters.
///
/// Any border, however thin, claims a whole cell — there is nothing finer to draw with. Side
/// positions come from the element's **full** rect (clipping first would strand borders along tile
/// seams); only the writes are limited to the current tile.
fn draw_border(canvas: &mut CellCanvas, rectangle: &Rectangle, tile_rect: Rect) {
    let border = rectangle.border();
    let (widths, styles, brushes) = (border.widths(), border.styles(), border.brushes());

    // [top, right, bottom, left]
    let colors: [Option<(u8, u8, u8)>; 4] = std::array::from_fn(|i| side_color(widths[i], &styles[i], &brushes[i]));
    if colors.iter().all(Option::is_none) {
        return;
    }

    let (c0, r0, c1, r1) = cell_bounds(rectangle.rect());
    if c1 <= c0 || r1 <= r0 {
        return;
    }
    let (tc0, tr0, tc1, tr1) = cell_bounds(tile_rect);

    // Only draw the parts of each side that fall inside this tile.
    let (col_lo, col_hi) = (c0.max(tc0), c1.min(tc1));
    let (row_lo, row_hi) = (r0.max(tr0), r1.min(tr1));

    let mut stroke = |col: usize, row: usize, ch: char, color: Option<(u8, u8, u8)>| {
        if let Some(color) = color {
            if (col_lo..col_hi).contains(&col) && (row_lo..row_hi).contains(&row) {
                canvas.set(col, row, ch, color);
            }
        }
    };

    let (top, right, bottom, left) = (colors[0], colors[1], colors[2], colors[3]);
    let (g_top, g_bottom) = (glyphs_for(&styles[0]), glyphs_for(&styles[2]));
    let (g_left, g_right) = (glyphs_for(&styles[3]), glyphs_for(&styles[1]));

    for col in col_lo..col_hi {
        stroke(col, r0, g_top.h, top);
        stroke(col, r1 - 1, g_bottom.h, bottom);
    }
    for row in row_lo..row_hi {
        stroke(c0, row, g_left.v, left);
        stroke(c1 - 1, row, g_right.v, right);
    }

    // Corners, drawn last so they win over the sides that meet there. A corner only exists where
    // both of its sides are actually painted.
    if top.is_some() && left.is_some() {
        stroke(c0, r0, g_top.tl, top);
    }
    if top.is_some() && right.is_some() {
        stroke(c1 - 1, r0, g_top.tr, top);
    }
    if bottom.is_some() && left.is_some() {
        stroke(c0, r1 - 1, g_bottom.bl, bottom);
    }
    if bottom.is_some() && right.is_some() {
        stroke(c1 - 1, r1 - 1, g_bottom.br, bottom);
    }
}

/// Cell bounds `(col0, row0, col1, row1)` of a page-space rect; the upper bounds are exclusive.
fn cell_bounds(rect: Rect) -> (usize, usize, usize, usize) {
    let col0 = (rect.x / f64::from(CELL_W)).round().max(0.0) as usize;
    let row0 = (rect.y / f64::from(CELL_H)).round().max(0.0) as usize;
    let col1 = ((rect.x + rect.width) / f64::from(CELL_W)).round().max(0.0) as usize;
    let row1 = ((rect.y + rect.height) / f64::from(CELL_H)).round().max(0.0) as usize;
    (col0, row0, col1, row1)
}

/// Fill the cells covered by a page-space rect.
fn fill_rect(canvas: &mut CellCanvas, rect: Rect, bg: (u8, u8, u8)) {
    let (col0, row0, col1, row1) = cell_bounds(rect);
    canvas.fill_bg(col0, row0, col1, row1, bg);
}

/// Overlap of two page-space rects, or `None` when they don't touch.
///
/// Paint commands carry the element's full page rect, and the same command is emitted for every
/// tile it crosses — so a `<body>` background would repaint the entire grid once per tile without
/// this clip.
fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
}

/// Paint one text command into the canvas.
///
/// Paint commands are in page coordinates, and the same command is emitted once per tile it
/// intersects — so a wide run is drawn several times at identical cell positions. That's
/// idempotent, which is why tile clipping can be ignored here.
fn draw_text(canvas: &mut CellCanvas, text: &Text) {
    let fg = match &text.brush {
        Brush::Solid(color) => (to_u8(color.r()), to_u8(color.g()), to_u8(color.b())),
        _ => DEFAULT_FG,
    };

    let col0 = (text.rect.x / f64::from(CELL_W)).round();
    let row0 = (text.rect.y / f64::from(CELL_H)).round();
    if col0 < 0.0 || row0 < 0.0 {
        return;
    }
    let (col0, row0) = (col0 as usize, row0 as usize);

    // Mirror `painter::paint_text_style` exactly: it shapes with
    // `max_width = available_width.max(rect_width)` for start-aligned text, and that shaping is
    // what layout reserved room for. Wrapping at `available_width` alone yields a narrower limit,
    // an extra line, and text that overruns into the element below it.
    let wrap_px = text.available_width.max(text.rect.width).max(1.0);
    let max_cols = Some((wrap_px / f64::from(CELL_W)).floor().max(1.0) as usize);

    for (line_no, line) in wrap_cells(&text.text, max_cols).iter().enumerate() {
        let row = row0 + line_no;
        let mut col = col0;
        for ch in line.chars() {
            let width = char_cells(ch);
            if width == 0 {
                // Combining marks have nowhere to go in a cell grid; drop them.
                continue;
            }
            if ch == ' ' {
                // Advance without writing. Cells are blank by default, so this is visually
                // identical — and it stops an inline box's own leading/trailing space from
                // erasing a neighbour whose rect rounds into the same cell.
                col += width;
                continue;
            }
            canvas.set(col, row, ch, fg);
            // A wide glyph owns the next cell too: park a filler so later writes don't land
            // inside it and the terminal's advance stays in step.
            for filler in 1..width {
                canvas.set(col + filler, row, '\0', fg);
            }
            col += width;
        }
    }
}

/// CSS colour channel (0.0–1.0) to an 8-bit terminal channel.
fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::CellFontSystem;

    fn rasterizer() -> TuiRasterizer {
        let canvas = Arc::new(Mutex::new(CellCanvas::new()));
        TuiRasterizer::new(canvas, Arc::new(Mutex::new(CellFontSystem::new())))
    }

    #[test]
    fn a_full_rebuild_discards_the_previous_page() {
        // Nothing else drops last pass's output: the canvas is one shared surface rather than
        // per-tile buffers, so without this a shorter page keeps the old page's tail visible.
        let r = rasterizer();
        r.canvas.lock().set(0, 0, 'x', (0, 0, 0));

        r.begin_pass(PassScope::FullRebuild);

        assert_eq!(r.canvas.lock().rows(), 0, "full rebuild should clear the canvas");
    }

    #[test]
    fn an_incremental_pass_keeps_the_page() {
        // A hover pass only re-rasterizes the tiles it dirtied; clearing here would blank
        // everything else, because those tiles will never be drawn again.
        let r = rasterizer();
        r.canvas.lock().set(0, 0, 'x', (0, 0, 0));

        r.begin_pass(PassScope::Incremental);

        assert_eq!(
            r.canvas.lock().cell(0, 0).map(|c| c.ch),
            Some('x'),
            "incremental pass must not clear carried-over tiles"
        );
    }
}

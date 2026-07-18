use std::sync::Arc;

use gosub_interface::font::{FontBlob, FontError, FontStyle};
use gosub_interface::font_system::{FontQuery, FontSystem, ResolvedFont, RunMetrics, ShapedRun, ShapedText, TextStyle};

use crate::cell::str_cells;

/// Cell width in CSS pixels. Layout runs in CSS pixels, so this is the exchange rate between the
/// two coordinate systems: a box `n` cells wide is `n * CELL_W` px wide.
pub const CELL_W: f32 = 8.0;
/// Cell height in CSS pixels.
pub const CELL_H: f32 = 16.0;
/// Baseline offset within a cell. Only used to fill `ShapedRun::baseline` — nothing in the text
/// path reads it, since cells have no sub-cell vertical positioning.
const ASCENT: f32 = 12.0;

/// Greedy word wrap at a cell-column limit, with CSS `white-space: normal` collapsing.
///
/// Shared by [`CellFontSystem::shape`] (which layout measures through) and the rasterizer (which
/// draws the result). Both must produce identical lines or text overflows the box layout reserved
/// for it — the same metric-mismatch hazard the `Text::available_width` field exists to avoid.
///
/// Leading and trailing whitespace is collapsed to a single space but **kept**: it is what
/// separates adjacent inline boxes (`<span>83 points</span> by <a>alice</a>`). Dropping it makes
/// neighbouring elements abut — "83 pointsbyalice".
pub fn wrap_cells(text: &str, max_cols: Option<usize>) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let leading = text.starts_with(char::is_whitespace);
    let trailing = text.ends_with(char::is_whitespace);

    let mut lines = wrap_words(text, max_cols.unwrap_or(usize::MAX).max(1));

    // A whitespace-only node still separates its neighbours, so it measures as one cell.
    if lines.is_empty() {
        return vec![" ".to_string()];
    }

    // Only the first line can carry the leading space, only the last the trailing one.
    if leading {
        if let Some(first) = lines.first_mut() {
            first.insert(0, ' ');
        }
    }
    if trailing {
        if let Some(last) = lines.last_mut() {
            last.push(' ');
        }
    }
    lines
}

fn wrap_words(text: &str, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;

    for word in text.split_whitespace() {
        let word_w = str_cells(word);
        let sep = usize::from(!cur.is_empty());

        if !cur.is_empty() && cur_w + sep + word_w > max {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0;
        }

        // A word wider than the limit is *not* broken — CSS `overflow-wrap: normal` lets it
        // overflow instead. This is also what makes min-content measurement correct: at max = 1
        // every word lands on its own line, so the reported width is the longest word.
        if !cur.is_empty() {
            cur.push(' ');
            cur_w += 1;
        }
        cur.push_str(word);
        cur_w += word_w;
    }

    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// A `FontSystem` that reports character-cell metrics instead of real font metrics.
///
/// This is what makes text-mode layout work: every advance is a whole number of cells, so Taffy's
/// measure callbacks produce a box tree that already sits on the character grid. It resolves every
/// query to one synthetic `monospace` face — there is no real font behind it, and none is needed,
/// because the cell rasterizer draws `Text::text` directly and never touches glyph IDs or
/// `ResolvedFont::blob`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CellFontSystem;

impl CellFontSystem {
    pub fn new() -> Self {
        Self
    }

    /// The single synthetic face every query resolves to. The blob is empty: `FontBlob` holds an
    /// `Arc<dyn AsRef<[u8]>>`, so "no font bytes" is representable without shipping a TTF.
    fn face(
        style: FontStyle,
        weight: gosub_interface::font_system::FontWeight,
        stretch: gosub_interface::font_system::FontStretch,
    ) -> ResolvedFont {
        ResolvedFont {
            family: "monospace".to_string(),
            style,
            weight,
            stretch,
            blob: FontBlob::new(Arc::new(Vec::<u8>::new()), 0),
        }
    }
}

impl FontSystem for CellFontSystem {
    fn register_font(&mut self, _data: Vec<u8>, _family_override: Option<&str>) -> Result<(), FontError> {
        // Web fonts are meaningless in a terminal; accept and ignore them.
        Ok(())
    }

    fn resolve(&mut self, query: &FontQuery<'_>) -> Result<ResolvedFont, FontError> {
        Ok(Self::face(query.style, query.weight, query.stretch))
    }

    fn families(&mut self) -> Vec<String> {
        vec!["monospace".to_string()]
    }

    fn vertical_grid_unit(&self) -> Option<f32> {
        // Layout runs on the character grid: one row is exactly one cell tall, so the layouter
        // snaps all vertical box metrics to whole cells. This is what stops sub-cell CSS spacing
        // (spacer rows, half-leading, fractional margins) from drifting the rasterizer's
        // per-element rounding into intermittent blank rows.
        Some(CELL_H)
    }

    fn shape(&mut self, text: &str, style: &TextStyle) -> ShapedText {
        if text.is_empty() {
            return ShapedText::empty();
        }

        // `max_width` of 0.0 is the layouter asking for min-content (`AvailableSpace::MinContent`),
        // not "unlimited" — filtering it out here reports the whole string as min-content, Taffy
        // concludes the text cannot shrink, and nothing on the page ever wraps.
        let max_cols = style.max_width.map(|w| (w / CELL_W).floor().max(1.0) as usize);

        let lines = wrap_cells(text, max_cols);
        if lines.is_empty() {
            return ShapedText::empty();
        }

        let face = Self::face(style.style, style.weight, style.stretch);
        let mut runs = Vec::with_capacity(lines.len());
        let mut widest = 0usize;

        for (i, line) in lines.iter().enumerate() {
            let cells = str_cells(line);
            widest = widest.max(cells);
            runs.push(ShapedRun {
                font: face.clone(),
                font_size: style.size,
                x: 0.0,
                baseline: i as f32 * CELL_H + ASCENT,
                width: cells as f32 * CELL_W,
                metrics: RunMetrics::default(),
                // No glyphs: the cell rasterizer re-derives characters from `Text::text`, the same
                // escape hatch the Pango/Parley rasterizers use to ignore pre-shaped runs.
                glyphs: Vec::new(),
            });
        }

        ShapedText {
            runs,
            width: widest as f32 * CELL_W,
            height: lines.len() as f32 * CELL_H,
            line_height: CELL_H,
            ascent: ASCENT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_greedily_at_the_column_limit() {
        assert_eq!(
            wrap_cells("the quick brown fox", Some(9)),
            vec!["the quick", "brown fox"]
        );
    }

    #[test]
    fn a_word_wider_than_the_line_overflows_rather_than_breaking() {
        // CSS `overflow-wrap: normal`: an over-long word is never split.
        assert_eq!(wrap_cells("abcdefgh", Some(3)), vec!["abcdefgh"]);
        assert_eq!(wrap_cells("hi abcdefgh", Some(3)), vec!["hi", "abcdefgh"]);
    }

    #[test]
    fn min_content_is_the_longest_word() {
        // The layouter passes max_width = 0.0 for AvailableSpace::MinContent. Treating that as
        // "unlimited" reports the whole string, Taffy decides the text can't shrink, and the page
        // stops wrapping entirely.
        let mut fs = CellFontSystem::new();
        let mut style = TextStyle::new("monospace", 16.0);
        style.max_width = Some(0.0);
        let (w, _) = fs.measure("a bb cccc dd", &style);
        assert_eq!(w, 4.0 * CELL_W, "min-content should be the longest word (\"cccc\")");
    }

    #[test]
    fn max_content_is_a_single_line() {
        let mut fs = CellFontSystem::new();
        let mut style = TextStyle::new("monospace", 16.0);
        style.max_width = Some(1_000_000_000.0);
        let (w, h) = fs.measure("a bb cccc dd", &style);
        assert_eq!(w, 12.0 * CELL_W);
        assert_eq!(h, CELL_H);
    }

    #[test]
    fn unwrapped_text_is_a_single_line() {
        assert_eq!(wrap_cells("a b c", None), vec!["a b c"]);
    }

    #[test]
    fn keeps_the_space_that_separates_inline_boxes() {
        // " by " between two elements must stay " by " — collapsing it to "by" makes the
        // neighbours abut.
        assert_eq!(wrap_cells(" by ", None), vec![" by "]);
        assert_eq!(wrap_cells("points ", None), vec!["points "]);
    }

    #[test]
    fn a_whitespace_only_node_measures_one_cell() {
        let mut fs = CellFontSystem::new();
        let (w, _) = fs.measure(" ", &TextStyle::new("monospace", 16.0));
        assert_eq!(w, CELL_W);
    }

    #[test]
    fn empty_text_measures_nothing() {
        assert!(wrap_cells("", None).is_empty());
        assert!(CellFontSystem::new()
            .shape("", &TextStyle::new("monospace", 16.0))
            .is_empty());
    }

    #[test]
    fn internal_whitespace_runs_collapse() {
        assert_eq!(wrap_cells("a   \n  b", None), vec!["a b"]);
    }

    #[test]
    fn wide_characters_count_as_two_cells() {
        // Each CJK char is 2 cells, so only two fit in 5 columns.
        assert_eq!(wrap_cells("日本 語", Some(5)), vec!["日本", "語"]);
    }

    #[test]
    fn measured_width_is_a_whole_number_of_cells() {
        let mut fs = CellFontSystem::new();
        let (w, h) = fs.measure("hello", &TextStyle::new("monospace", 16.0));
        assert_eq!(w, 5.0 * CELL_W);
        assert_eq!(h, CELL_H);
    }

    #[test]
    fn shaping_agrees_with_the_wrap_used_when_drawing() {
        let mut style = TextStyle::new("monospace", 16.0);
        style.max_width = Some(9.0 * CELL_W);
        let shaped = fs_shape(&mut style);
        assert_eq!(shaped.runs.len(), 2);
        assert_eq!(shaped.height, 2.0 * CELL_H);
    }

    fn fs_shape(style: &mut TextStyle) -> ShapedText {
        CellFontSystem::new().shape("the quick brown fox", style)
    }
}

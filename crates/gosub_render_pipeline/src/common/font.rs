#[derive(Debug, Clone)]
pub enum FontAlignment {
    /// Start of the line (left for LTR, right for RTL)
    Start,
    Center,
    /// End of the line (right for LTR, left for RTL)
    End,
    Justify,
}

#[derive(Debug, Clone)]
pub struct FontInfo {
    pub family: String,
    /// Font size in px
    pub size: f64,
    /// Font weight (100-900)
    pub weight: i32,
    /// Font width (100-900)
    pub width: i32,
    /// Font slant (0-1000)
    pub slant: i32,
    /// CSS line-height in px. `None` = `normal`: the font system uses the font's natural
    /// metrics; `Some` line boxes are exactly this tall (half-leading model).
    pub line_height: Option<f64>,
    /// Extra spacing between characters in px (CSS `letter-spacing`; 0 = `normal`)
    pub letter_spacing: f64,
    pub alignment: FontAlignment,
    pub underline: bool,
    pub line_through: bool,
}

impl FontInfo {
    /// The line height in px, with `normal` approximated as 1.2em for callers that
    /// need a number (form-control chrome, caret geometry).
    pub fn line_height_px(&self) -> f64 {
        self.line_height.unwrap_or(self.size * 1.2)
    }
}

use gosub_interface::layout::{Decoration, TextLayout as TLayout};
use gosub_shared::geo::Size;
use gosub_interface::font::Font as GsFont;

#[derive(Debug)]
pub struct TextLayout {
    pub text: String,
    // pub glyphs: Vec<Glyph>,
    pub font: Font,
    pub font_size: f32,
    pub size: Size,
    pub coords: Vec<i16>,
    pub decoration: Decoration,
}

impl TLayout for TextLayout {
    type Font = GsFont;

    fn text(&self) -> &str {
        self.text.as_str()
    }

    fn dbg_layout(&self) -> String {
        format!("TextLayout: {:?}", self)
    }

    fn font(&self) -> &Self::Font {
        &self.font
    }

    fn coords(&self) -> &[i16] {
        &self.coords
    }

    fn decorations(&self) -> &Decoration {
        &self.decoration
    }
}

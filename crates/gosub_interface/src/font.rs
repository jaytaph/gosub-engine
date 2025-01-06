pub trait Font {
    fn new(family: &str, size: f32) -> Self;
    fn family(&self) -> &str;
    fn size(&self) -> f32;
    fn weight(&self) -> FontWeight;
    fn style(&self) -> FontStyle;
    fn decoration(&self) -> FontDecoration;
}

#[derive(Copy, Debug, Clone, PartialEq)]
pub enum FontWeight {
    Thin,
    Light,
    Regular,
    Medium,
    Bold,
    ExtraBold,
}

#[derive(Copy, Debug, Clone, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontDecoration {
    pub underline: bool,
    pub strike_through: bool,
}

impl Default for FontDecoration {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDecoration {
    pub fn new() -> Self {
        FontDecoration {
            underline: false,
            strike_through: false,
        }
    }
    
    pub fn set_underline(&mut self, underline: bool) {
        self.underline = underline;
    }

    pub fn set_strike_through(&mut self, strike_through: bool) {
        self.strike_through = strike_through;
    }
}
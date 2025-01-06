use pango::{FontDescription, Style, Weight};
use gosub_interface::font::{Font as TFont, FontDecoration, FontStyle, FontWeight};

/// Implementation of the Font
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    /// Font family used (ie: "Arial", "Times New Roman", etc.)
    pub family: String,
    /// Font size (defined by height in points)
    pub size: f32,
    /// Font weight (ie: "normal", "bold", etc.)
    pub weight: FontWeight,
    /// Font style (ie: "normal", "italic", etc.)
    pub style: FontStyle,
    /// Font decoration (ie: "none", "underline", "line-through", etc.)
    pub decoration: FontDecoration,
}

impl TFont for Font {
    fn new(family: &str, size: f32) -> Self {
        Font {
            family: family.to_string(),
            size,
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
            decoration: FontDecoration::default(),
        }
    }

    fn family(&self) -> &str {
        self.family.as_str()
    }

    fn size(&self) -> f32 {
        self.size
    }

    fn weight(&self) -> FontWeight {
        self.weight
    }

    fn style(&self) -> FontStyle {
        self.style
    }

    fn decoration(&self) -> FontDecoration {
        self.decoration.clone()
    }
}

// Default implementation for Fonts
// https://granneman.com/webdev/coding/css/fonts-and-formatting/web-browser-font-defaults
//
// OS	        Browser	    Sans-serif	    Serif	            Mono
// Windows 	    Firefox 	Arial 	        Times New Roman 	Courier New
// Mac OS X 	Firefox 	Helvetica 	    Times 	            Courier
// Linux 	    Firefox 	sans-serif 	    serif 	            monospace

impl Default for Font {
    fn default() -> Self {
        Font {
            #[cfg(target_os = "windows")]
            family: "Arial".to_string(),
            #[cfg(target_os = "macos")]
            family: "Helvetica".to_string(),
            #[cfg(target_os = "linux")]
            family: "sans-serif".to_string(),

            size: 12.0,
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
            decoration: FontDecoration::default(),
        }
    }
}

impl Font {
    pub fn new(family: &str, size: f32) -> Self {
        Font {
            family: family.to_string(),
            size,
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
            decoration: FontDecoration::new(),
        }
    }

    pub fn set_family(&mut self, family: &str) {
        self.family = family.to_string();
    }

    pub fn set_size(&mut self, size: f32) {
        self.size = size;
    }

    pub fn set_weight(&mut self, weight: FontWeight) {
        self.weight = weight;
    }

    pub fn set_style(&mut self, style: FontStyle) {
        self.style = style;
    }

    pub fn set_decoration(&mut self, decoration: FontDecoration) {
        self.decoration = decoration;
    }

    pub fn get_font_description(&self) -> FontDescription {
        let mut font_desc = FontDescription::new();

        font_desc.set_family(&self.family);

        // Set the font size in Pango units (1 pt = 1024 Pango units)
        font_desc.set_size((self.size * pango::SCALE as f32) as i32);

        // Map your FontWeight to Pango's Weight
        let pango_weight = match self.weight {
            FontWeight::Thin => Weight::Thin,
            FontWeight::Light => Weight::Light,
            FontWeight::Regular => Weight::Normal,
            FontWeight::Medium => Weight::Medium,
            FontWeight::Bold => Weight::Bold,
            FontWeight::ExtraBold => Weight::Ultrabold,
        };
        font_desc.set_weight(pango_weight);

        let pango_style = match self.style {
            FontStyle::Normal => Style::Normal,
            FontStyle::Italic => Style::Italic,
            FontStyle::Oblique => Style::Oblique,
        };
        font_desc.set_style(pango_style);

        font_desc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_decoration() {
        let mut decoration = FontDecoration::new();
        assert_eq!(decoration.underline, false);
        assert_eq!(decoration.strike_through, false);

        decoration.set_underline(true);
        assert_eq!(decoration.underline, true);
        assert_eq!(decoration.strike_through, false);

        decoration.set_strike_through(true);
        assert_eq!(decoration.underline, true);
        assert_eq!(decoration.strike_through, true);
    }
}

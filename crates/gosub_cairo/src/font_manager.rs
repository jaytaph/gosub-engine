use pango::prelude::{FontFamilyExt, FontMapExt};

/// This manager keeps track of all loaded fonts and provides a way to check if a font family is available.
pub struct FontManager {
    font_map: pango::FontMap,
}

impl FontManager {
    pub fn new() -> Self {
        let font_map = pangocairo::FontMap::new();
        Self {
            font_map,
        }
    }

    pub fn has_font_family(&self, family: &str) -> bool {
        self.font_map.list_families().iter().any(|f| f.name() == family)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager() {
        let font_manager = FontManager::new();
        assert!(font_manager.has_font_family("Sans"));
        assert!(font_manager.has_font_family("Serif"));
        assert!(font_manager.has_font_family("Monospace"));

        assert!(!font_manager.has_font_family("Comic Sans"));
        assert!(!font_manager.has_font_family("NOTAVAILBLE"));
    }
}
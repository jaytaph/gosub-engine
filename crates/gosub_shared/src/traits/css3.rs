use crate::traits::ParserConfig;
use crate::types::Result;

/// Defines the origin of the stylesheet (or declaration)
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CssOrigin {
    /// Browser/user agent defined stylesheets
    UserAgent,
    /// Author defined stylesheets that are linked or embedded in the HTML files
    Author,
    /// User defined stylesheets that will override the author and user agent stylesheets (for instance, custom user styles or extensions)
    User,
}


/// The CssSystem trait is a trait that defines all things CSS3 that are used by other non-css3 crates. This is the main trait that
/// is used to parse CSS3 files. It contains sub elements like the Stylesheet trait that is used in for instance the Document trait.
pub trait CssSystem: Clone {
    type Stylesheet: CssStylesheet;

    /// Parses a string into a CSS3 stylesheet
    fn parse_str(str: &str, config: ParserConfig, origin: CssOrigin, source_url: &str) -> Result<Self::Stylesheet>;
}

pub trait CssStylesheet: PartialEq {
    /// Returns the origin of the stylesheet
    fn origin(&self) -> CssOrigin;

    /// Returns the source URL of the stylesheet
    fn location(&self) -> &str;
}
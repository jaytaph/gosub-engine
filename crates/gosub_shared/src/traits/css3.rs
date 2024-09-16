use crate::traits::ParserConfig;

/// Defines the origin of the stylesheet (or declaration)
#[derive(Debug, PartialEq, Clone)]
pub enum CssOrigin {
    /// Browser/user agent defined stylesheets
    UserAgent,
    /// Author defined stylesheets that are linked or embedded in the HTML files
    Author,
    /// User defined stylesheets that will override the author and user agent stylesheets (for instance, custom user styles or extensions)
    User,
}

pub trait CssSystem: Clone {
    type Stylesheet: CssStylesheet;

    fn parse_str(str: &str, config: ParserConfig, origin: (), source: String);
}


pub trait CssStylesheet: PartialEq {

}
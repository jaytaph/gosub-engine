//! Error results that can be returned from the css3 parser
use gosub_shared::byte_stream::Location;
use thiserror::Error;

/// Parser error that defines an error (message) on the given position
#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    /// Error message
    pub message: String,
    /// Location of the error
    pub location: Location,
}

/// Serious errors and errors from third-party libraries
#[derive(Debug, Error)]
pub enum Error {
    #[error("parse error: {0} at {1}")]
    Parse(String, Location),

    #[allow(dead_code)]
    #[error("incorrect value: {0} at {1}")]
    IncorrectValue(String, Location),

    #[error("css failure: {0}")]
    CssFailure(String),
}

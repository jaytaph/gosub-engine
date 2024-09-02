//! HTML5 tokenizer and parser
//!
//! The parser's job is to take a stream of bytes and turn it into a DOM tree. The parser is
//! implemented as a state machine and runs in the current thread.
use crate::parser::Html5Parser;
use gosub_shared::byte_stream::{ByteStream, Encoding};
use gosub_shared::document::DocumentHandle;
use crate::document::builder::DocumentBuilder;
use crate::document::document::DocumentImpl;

pub mod dom;
pub mod document;
pub mod errors;
pub mod node;
pub mod parser;
pub mod tokenizer;
pub mod writer;

/// Parses the given HTML string and returns a handle to the resulting DOM tree.
pub fn html_compile(html: &str) -> DocumentHandle<DocumentImpl> {
    let mut stream = ByteStream::new(Encoding::UTF8, None);
    stream.read_from_str(html, Some(Encoding::UTF8));
    stream.close();

    let handle = DocumentBuilder::new_document(None);
    let _ = Html5Parser::parse_document(&mut stream, handle.clone(), None);

    handle
}

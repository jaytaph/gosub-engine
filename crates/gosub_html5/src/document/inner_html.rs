//! Replacing an element's children by parsing markup - the `innerHTML` setter.

use gosub_interface::config::HasDocument;
use gosub_interface::document::Document;
use gosub_shared::byte_stream::{ByteStream, Encoding, Location};
use gosub_shared::node::NodeId;
use gosub_shared::types::Result;

use crate::parser::Html5Parser;

/// Parse `html` in the context of `target` and make the result its new children.
///
/// `parse_fragment` builds into a fresh `<html>` element it hangs off the document root, so
/// the parsed children are moved across and that scaffolding is thrown away afterwards.
pub fn set_inner_html<C: HasDocument>(document: &mut C::Document, target: NodeId, html: &str) -> Result<()> {
    for child in document.children(target).to_vec() {
        document.remove(child);
    }

    let mut stream = ByteStream::from_str(html, Encoding::UTF8);
    let before = document.children(document.root()).len();
    Html5Parser::<C>::parse_fragment(&mut stream, document, target, None, Location::default())?;

    let root = document.root();
    let scaffold: Vec<NodeId> = document.children(root)[before..].to_vec();
    for holder in scaffold {
        for child in document.children(holder).to_vec() {
            document.detach(child);
            document.attach(child, target, None);
        }
        document.remove(holder);
    }
    Ok(())
}

/// Serialise an element's children - what the `innerHTML` getter returns.
pub fn inner_html<C: HasDocument>(document: &C::Document, target: NodeId) -> String {
    document
        .children(target)
        .iter()
        .map(|&child| document.write_from_node(child))
        .collect()
}

//! Text helpers the DOM getters need.

use gosub_interface::document::Document as _;
use gosub_shared::node::NodeId;

use crate::Doc;

/// Concatenated data of every Text descendant of `id`, in tree order.
pub fn descendant_text(doc: &Doc, id: NodeId) -> String {
    let mut out = String::new();
    collect(doc, id, &mut out);
    out
}

fn collect(doc: &Doc, id: NodeId, out: &mut String) {
    if let Some(text) = doc.text_value(id) {
        out.push_str(text);
    }
    for &child in doc.children(id) {
        collect(doc, child, out);
    }
}

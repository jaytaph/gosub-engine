use std::collections::HashMap;

use url::Url;

use gosub_shared::traits::document::{Document, DocumentType};
use gosub_shared::traits::node::{Node, QuirksMode};
use crate::DocumentHandle;
use crate::node::HTML_NAMESPACE;

/// This struct will be used to create a fully initialized document or document fragment
pub struct DocumentBuilder {}

impl DocumentBuilder {
    /// Creates a new document with a document root node
    pub fn new_document<D: Document>(url: Option<Url>) -> DocumentHandle<D> {
        let doc = D::new(DocumentType::HTML, url, None);
        DocumentHandle::create(doc)
    }

    /// Creates a new document fragment with the context as the root node
    pub fn new_document_fragment<D>(context_node: &D::Node) -> DocumentHandle<D>
    where
        D: Document,
        D::Node: Node<Document=D>
    {
        let handle = context_node.handle();

        // Create a new document with an HTML node as the root node
        let fragment_root_node = D::new_element_node(
            handle.clone(),
            "html",
            Some(HTML_NAMESPACE),
            HashMap::new(),
            context_node.location().clone(),
        );
        let fragment_doc = D::new(DocumentType::HTML, None, Some(&fragment_root_node));
        let fragment_handle = DocumentHandle::create(fragment_doc);

        let context_doc_handle = context_node.handle();
        match context_doc_handle.get().quirks_mode() {
            QuirksMode::Quirks => {
                fragment_handle.get_mut().set_quirks_mode(QuirksMode::Quirks);
            }
            QuirksMode::LimitedQuirks => {
                fragment_handle.get_mut().set_quirks_mode(QuirksMode::LimitedQuirks);
            }
            _ => {}
        }

        fragment_handle
    }
}
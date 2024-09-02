use std::collections::HashMap;
use url::Url;
use crate::byte_stream::Location;
use crate::document::DocumentHandle;
use crate::node::NodeId;
use crate::traits::node::{Node, QuirksMode};

/// Type of the given document
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum DocumentType {
    /// HTML document
    HTML,
    /// Iframe source document
    IframeSrcDoc,
}

pub trait DocumentFragment: Sized {
    type Document: Document;

    /// Returns the document handle for this document
    fn handle(&self) -> DocumentHandle<Self::Document>;
}

pub trait Document: Sized {
    type Node: Node;

    // Creates a new doc with an optional document root node
    fn new(document_type: DocumentType, url: Option<Url>, root_node: Option<&Self::Node>) -> Self;

    // /// Creates a new document with an optional document root node
    // fn new_with_handle(document_type: DocumentType, url: Option<Url>, location: &Location, root_node: Option<&Self::Node>) -> DocumentHandle<Self>;

    /// Returns the document handle for this document
    fn handle(&self) -> DocumentHandle<Self>;

    /// Location of the document (URL, file path, etc.)
    fn url(&self) -> Option<Url>;

    fn set_quirks_mode(&mut self, quirks_mode: QuirksMode);
    fn quirks_mode(&self) -> QuirksMode;
    fn set_doctype(&mut self, doctype: DocumentType);
    fn doctype(&self) -> DocumentType;

    /// Return a node by its Node ID
    fn node_by_id(&self, node_id: NodeId) -> Option<&Self::Node>;
    fn get_node_by_id_mut(&mut self, node_id: NodeId) -> Option<&mut Self::Node>;

    /// Return the root node of the document
    fn get_root(&self) -> &Self::Node;
    fn get_root_mut(&mut self) -> &mut Self::Node;

    fn attach_node(&mut self, node_id: NodeId, parent_id: NodeId, position: Option<usize>);
    fn detach_node(&mut self, node_id: NodeId);
    fn relocate_node(&mut self, node_id: NodeId, parent_id: NodeId);

    /// Return the parent node from a given ID
    fn parent_node(&self, node: &Self::Node) -> Option<&Self::Node>;

    /// Removes a node from the document
    fn delete_node_by_id(&mut self, node_id: NodeId);

    /// Returns the next sibling of the reference node
    fn get_next_sibling(&self, node: NodeId) -> Option<NodeId>;

    // /// Returns the next node ID that will be used when registering a new node
    // fn peek_next_id(&self) -> NodeId;

    /// Register a new node
    fn register_node(&mut self, node: &Self::Node) -> NodeId;
    /// Register a new node at a specific position
    fn register_node_at(&mut self, node: &Self::Node, parent_id: NodeId, position: Option<usize>) -> NodeId;

    /// Node creation methods. The root node is needed in order to fetch the document handle (it can't be created from the document itself)
    fn new_document_node(handle: DocumentHandle<Self>, quirks_mode: QuirksMode, location: Location) -> Self::Node;
    fn new_doctype_node(handle: DocumentHandle<Self>, name: &str, public_id: Option<&str>, system_id: Option<&str>, location: Location) -> Self::Node;
    fn new_comment_node(handle: DocumentHandle<Self>, comment: &str, location: Location) -> Self::Node;
    fn new_text_node(handle: DocumentHandle<Self>, value: &str, location: Location) -> Self::Node;
    fn new_element_node(handle: DocumentHandle<Self>, name: &str, namespace: Option<&str>, attributes: HashMap<String, String>, location: Location) -> Self::Node;
}

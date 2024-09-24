use crate::doc::document::DocumentImpl;
use crate::node::data::comment::CommentData;
use crate::node::data::doctype::DocTypeData;
use crate::node::data::document::DocumentData;
use crate::node::data::element::ElementData;
use crate::node::data::text::TextData;
use core::fmt::Debug;
use std::collections::HashMap;
use gosub_shared::byte_stream::Location;
use gosub_shared::document::DocumentHandle;
use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::CssSystem;
use gosub_shared::traits::node::{Node, NodeData, NodeType, QuirksMode};

/// Implementation of the NodeDataType trait
#[derive(Debug, Clone, PartialEq)]
pub enum NodeDataTypeInternal<C: CssSystem> {
    /// Represents a document
    Document(DocumentData),
    // Represents a doctype
    DocType(DocTypeData),
    /// Represents a text
    Text(TextData),
    /// Represents a comment
    Comment(CommentData),
    /// Represents an element
    Element(ElementData<C>),
}

/// Node structure that resembles a DOM node
pub struct NodeImpl<C: CssSystem> {
    /// ID of the node, 0 is always the root / document node
    pub id: NodeId,
    /// parent of the node, if any
    pub parent: Option<NodeId>,
    /// any children of the node
    pub children: Vec<NodeId>,
    /// actual data of the node
    pub data: NodeDataTypeInternal<C>,
    /// Handle to the document in which this node resides
    pub document: DocumentHandle<DocumentImpl<C>, C>,
    // Returns true when the given node is registered into the document arena
    pub is_registered: bool,
    // Location of the node in the source code
    pub location: Location,
}

impl<C: CssSystem> Node<C> for NodeImpl<C> {
    type Document = DocumentImpl<C>;
    type DocumentData = DocumentData;
    type DocTypeData = DocTypeData;
    type TextData = TextData;
    type CommentData = CommentData;
    type ElementData = ElementData<C>;

    fn id(&self) -> NodeId {
        self.id
    }

    fn set_id(&mut self, id: NodeId) {
        self.id = id
    }

    fn location(&self) -> Location {
        self.location
    }

    fn parent_id(&self) -> Option<NodeId> {
        self.parent
    }

    fn set_parent(&mut self, parent_id: Option<NodeId>) {
        self.parent = parent_id;
    }

    fn is_root(&self) -> bool {
        self.parent_id().is_none()
    }

    fn children(&self) -> &[NodeId] {
        self.children.as_slice()
    }

    fn type_of(&self) -> NodeType {
        match self.data {
            NodeDataTypeInternal::Document(_) => NodeType::DocumentNode,
            NodeDataTypeInternal::DocType(_) => NodeType::DocTypeNode,
            NodeDataTypeInternal::Text(_) => NodeType::TextNode,
            NodeDataTypeInternal::Comment(_) => NodeType::CommentNode,
            NodeDataTypeInternal::Element(_) => NodeType::ElementNode,
        }
    }

    fn is_element_node(&self) -> bool {
        self.type_of() == NodeType::ElementNode
    }

    fn get_element_data(&self) -> Option<&Self::ElementData> {
        if let NodeDataTypeInternal::Element(data) = &self.data {
            return Some(data);
        }
        None
    }

    fn get_element_data_mut(&mut self) -> Option<&mut ElementData<C>> {
        if let NodeDataTypeInternal::Element(data) = &mut self.data {
            return Some(data);
        }
        None
    }

    fn is_text_node(&self) -> bool {
        matches!(self.data, NodeDataTypeInternal::Text(_))
    }

    fn get_text_data(&self) -> Option<&Self::TextData> {
        if let NodeDataTypeInternal::Text(data) = &self.data {
            return Some(data);
        }
        None
    }

    fn get_text_data_mut(&mut self) -> Option<&mut TextData> {
        if let NodeDataTypeInternal::Text(data) = &mut self.data {
            return Some(data);
        }
        None
    }

    fn get_comment_data(&self) -> Option<&Self::CommentData> {
        if let NodeDataTypeInternal::Comment(data) = &self.data {
            return Some(data);
        }
        None
    }

    fn get_doctype_data(&self) -> Option<&Self::DocTypeData> {
        if let NodeDataTypeInternal::DocType(data) = &self.data {
            return Some(data);
        }
        None
    }

    fn handle(&self) -> DocumentHandle<Self::Document, C> {
        self.document.clone()
    }

    fn remove(&mut self, node_id: NodeId) {
        self.children.retain(|x| x != &node_id);
    }

    fn insert(&mut self, node_id: NodeId, idx: usize) {
        self.children.insert(idx, node_id);
    }

    fn push(&mut self, node_id: NodeId) {
        self.children.push(node_id);
    }

    fn data(&self) -> NodeData<C, Self> {
        match self.data {
            NodeDataTypeInternal::Document(ref data) => NodeData::Document(data),
            NodeDataTypeInternal::DocType(ref data) => NodeData::DocType(data),
            NodeDataTypeInternal::Text(ref data) => NodeData::Text(data),
            NodeDataTypeInternal::Comment(ref data) => NodeData::Comment(data),
            NodeDataTypeInternal::Element(ref data) => NodeData::Element(data),
        }
    }
}

impl<C: CssSystem> PartialEq for NodeImpl<C> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id()
    }
}

impl<C: CssSystem> Debug for NodeImpl<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Node");
        debug.field("id", &self.id);
        debug.field("parent", &self.parent);
        debug.field("children", &self.children);
        // @todo: add element/doctype etc data
        debug.finish_non_exhaustive()
    }
}

impl<C: CssSystem> Clone for NodeImpl<C> {
    fn clone(&self) -> Self {
        NodeImpl {
            id: self.id,
            parent: self.parent,
            children: self.children.clone(),
            data: self.data.clone(),
            document: self.document.clone(),
            is_registered: self.is_registered,
            location: self.location,
        }
    }
}

impl<C: CssSystem> NodeImpl<C> {
    /// create a new `Node`
    #[must_use]
    pub fn new(
        document: DocumentHandle<DocumentImpl<C>, C>,
        location: Location,
        data: &NodeDataTypeInternal<C>,
    ) -> Self {
        let (id, parent, children, is_registered) = <_>::default();
        Self {
            id,
            parent,
            children,
            data: data.clone(),
            document: document.clone(),
            is_registered,
            location,
        }
    }

    /// Create a new document node
    #[must_use]
    pub fn new_document(document: DocumentHandle<DocumentImpl<C>, C>, location: Location, quirks_mode: QuirksMode) -> Self {
        Self::new(document, location, &NodeDataTypeInternal::Document(DocumentData::new(quirks_mode)))
    }

    #[must_use]
    pub fn new_doctype(
        document: DocumentHandle<DocumentImpl<C>, C>,
        location: Location,
        name: &str,
        pub_identifier: &str,
        sys_identifier: &str,
    ) -> Self {
        Self::new(
            document,
            location,
            &NodeDataTypeInternal::DocType(DocTypeData::new(name, pub_identifier, sys_identifier)),
        )
    }

    /// Create a new element node with the given name and attributes and namespace
    #[must_use]
    pub fn new_element(
        document: DocumentHandle<DocumentImpl<C>, C>,
        location: Location,
        name: &str,
        namespace: Option<&str>,
        attributes: HashMap<String, String>,
    ) -> Self {
        Self::new(
            document,
            location,
            &NodeDataTypeInternal::Element(ElementData::new(
                name,
                namespace,
                attributes,
                Default::default(),
            ))
        )
    }

    /// Creates a new comment node
    #[must_use]
    pub fn new_comment(document: DocumentHandle<DocumentImpl<C>, C>, location: Location, value: &str) -> Self {
        Self::new(
            document.clone(),
            location,
            &NodeDataTypeInternal::Comment(CommentData::with_value(value)),
        )
    }

    /// Creates a new text node
    #[must_use]
    pub fn new_text(document: DocumentHandle<DocumentImpl<C>, C>, location: Location, value: &str) -> Self {
        Self::new(
            document.clone(),
            location,
            &NodeDataTypeInternal::Text(TextData::with_value(value)),
        )
    }

    /// Returns true if this node is registered into an arena
    pub fn is_registered(&self) -> bool {
        self.is_registered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::elements::SPECIAL_HTML_ELEMENTS;
    use crate::node::elements::SPECIAL_MATHML_ELEMENTS;
    use crate::node::elements::SPECIAL_SVG_ELEMENTS;
    use crate::node::HTML_NAMESPACE;
    use crate::node::MATHML_NAMESPACE;
    use crate::node::SVG_NAMESPACE;
    use gosub_shared::traits::document::{Document, DocumentType};
    use std::collections::HashMap;

    #[test]
    fn new_document() {
        let document = Document::new(DocumentType::HTML, None, None);
        let node = NodeImpl::new_document(document.clone(), Location::default(), QuirksMode::NoQuirks);
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
        match &node.data {
            NodeData::Document(_) => (),
            _ => panic!(),
        }
    }

    #[test]
    fn new_element() {
        let mut attributes = HashMap::new();
        attributes.insert("id".to_string(), "test".to_string());

        let document = Document::shared(None);
        let node = NodeImpl::new_element(
            document.clone(),
            Location::default(),
            "div",
            Some(HTML_NAMESPACE),
            attributes.clone(),
        );
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());

        match &node.data {
            NodeData::Element(data) => {
                assert_eq!(data.name(), "div");
                assert!(data.attributes().contains_key("id"));
                assert_eq!(data.attributes().get("id").unwrap(), "test");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn new_comment() {
        let document = Document::new(DocumentType::HTML, None, None);
        let node = NodeImpl::new_comment(document.clone(), Location::default(), "test");
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
        let NodeData::Comment(CommentData { value, .. }) = &node.data else {
            panic!()
        };
        assert_eq!(value, "test");
    }

    #[test]
    fn new_text() {
        let document = Document::new(DocumentType::HTML, None, None);
        let node = NodeImpl::new_text(document.clone(), Location::default(), "test");
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
        let NodeData::Text(TextData { value }) = &node.data else {
            panic!()
        };
        assert_eq!(value, "test");
    }

    #[test]
    fn is_special() {
        let mut attributes = HashMap::new();
        attributes.insert("id".to_string(), "test".to_string());
        let document = Document::new(DocumentType::HTML, None, None);
        let node = NodeImpl::new_element(
            document.clone(),
            Location::default(),
            "div",
            Some(HTML_NAMESPACE),
            attributes,
        );
        assert!(node.is_special());
    }

    #[test]
    fn type_of() {
        let document = Document::new(DocumentType::HTML, None, None);
        let node = NodeImpl::new_document(document.clone(), Location::default(), QuirksMode::NoQuirks);
        assert_eq!(node.type_of(), NodeType::DocumentNode);
        let node = NodeImpl::new_text(document.clone(), Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::TextNode);
        let node = NodeImpl::new_comment_node(document.clone(), Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::CommentNode);
        let mut attributes = HashMap::new();
        attributes.insert("id".to_string(), "test".to_string());
        let node = NodeImpl::new_element(
            document.clone(),
            Location::default(),
            "div",
            Some(HTML_NAMESPACE),
            attributes,
        );
        assert_eq!(node.type_of(), NodeType::ElementNode);
    }

    #[test]
    fn special_html_elements() {
        let document = Document::new(DocumentType::HTML, None, None);

        for element in SPECIAL_HTML_ELEMENTS.iter() {
            let mut attributes = HashMap::new();
            attributes.insert("id".to_string(), "test".to_string());
            let node = NodeImpl::new_element(
                document.clone(),
                Location::default(),
                element,
                Some(HTML_NAMESPACE),
                attributes,
            );
            assert!(node.is_special());
        }
    }

    #[test]
    fn special_mathml_elements() {
        let document = Document::new(DocumentType::HTML, None, None);

        for element in SPECIAL_MATHML_ELEMENTS.iter() {
            let mut attributes = HashMap::new();
            attributes.insert("id".to_string(), "test".to_string());
            let node = NodeImpl::new_element(
                document.clone(),
                Location::default(),
                element,
                Some(MATHML_NAMESPACE),
                attributes.clone(),
            );

            assert!(node.is_special());
        }
    }

    #[test]
    fn special_svg_elements() {
        let document = Document::new(DocumentType::HTML, None, None);

        for element in SPECIAL_SVG_ELEMENTS.iter() {
            let mut attributes = HashMap::new();
            attributes.insert("id".to_string(), "test".to_string());
            let node = NodeImpl::new_element(
                document.clone(),
                Location::default(),
                element,
                Some(SVG_NAMESPACE),
                attributes,
            );
            assert!(node.is_special());
        }
    }

    #[test]
    fn type_of_node() {
        let document = Document::new(DocumentType::HTML, None, None);

        let node = NodeImpl::new_document(document.clone(), Location::default(), QuirksMode::NoQuirks);
        assert_eq!(node.type_of(), NodeType::DocumentNode);
        let node = NodeImpl::new_text(document.clone(), Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::TextNode);
        let node = NodeImpl::new_comment(document.clone(), Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::CommentNode);
        let mut attributes = HashMap::new();
        attributes.insert("id".to_string(), "test".to_string());
        let node = NodeImpl::new_element(
            document.clone(),
            Location::default(),
            "div",
            Some(HTML_NAMESPACE),
            attributes,
        );
        assert_eq!(node.type_of(), NodeType::ElementNode);
    }
}

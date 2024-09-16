use gosub_shared::traits::node::{Node, NodeType};
use crate::node::data::comment::CommentData;
use crate::node::data::doctype::DocTypeData;
use crate::node::data::document::DocumentData;
use crate::node::data::text::TextData;
use core::fmt::Debug;
use std::cell::{Ref, RefCell, RefMut};
use gosub_shared::byte_stream::Location;
use gosub_shared::document::DocumentHandle;
use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::CssSystem;
use crate::document::document::DocumentImpl;
use crate::node::data::element::ElementData;

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
    pub data: RefCell<NodeDataTypeInternal<C>>,
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
        self.id = id.clone()
    }

    fn location(&self) -> Location {
        self.location.clone()
    }

    fn parent_id(&self) -> Option<NodeId> {
        self.parent.clone()
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
        match *self.data.borrow() {
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

    fn get_element_data(&self) -> Option<Ref<Self::ElementData>> {
        let borrowed_data = self.data.borrow();

        if let NodeDataTypeInternal::Element(_) = *borrowed_data {
            return Some(Ref::map(borrowed_data, |d|
                if let NodeDataTypeInternal::Element(ref element_data) = d {
                    element_data
                } else {
                    unreachable!()
                }
            ));
        }
        None
    }

    fn get_element_data_mut(&self) -> Option<RefMut<ElementData<C>>> {
        let borrowed_data = self.data.borrow_mut();

        if let NodeDataTypeInternal::Element(_) = *borrowed_data {
            return Some(RefMut::map(borrowed_data, |d| {
                if let NodeDataTypeInternal::Element(ref mut element_data) = d {
                    element_data
                } else {
                    unreachable!()
                }
            }));
        }
        None
    }

    fn is_text_node(&self) -> bool {
        match *self.data.borrow() {
            NodeDataTypeInternal::Text(_) => true,
            _ => false,
        }
    }

    fn get_text_data(&self) -> Option<Ref<Self::TextData>> {
        let borrowed_data = self.data.borrow();

        if let NodeDataTypeInternal::Text(_) = *borrowed_data {
            return Some(Ref::map(borrowed_data, |d|
                if let NodeDataTypeInternal::Text(ref text_data) = d {
                    text_data
                } else {
                    unreachable!()
                }
            ));
        }
        None
    }

    fn get_text_data_mut(&self) -> Option<RefMut<TextData>> {
        let borrowed_data = self.data.borrow_mut();

        if let NodeDataTypeInternal::Text(_) = *borrowed_data {
            return Some(RefMut::map(borrowed_data, |d| {
                if let NodeDataTypeInternal::Text(ref mut text_data) = d {
                    text_data
                } else {
                    unreachable!()
                }
            }));
        }
        None
    }

    fn get_comment_data(&self) -> Option<Ref<Self::CommentData>> {
        let borrowed_data = self.data.borrow();

        if let NodeDataTypeInternal::Comment(_) = *borrowed_data {
            return Some(Ref::map(borrowed_data, |d|
                if let NodeDataTypeInternal::Comment(ref text_data) = d {
                    text_data
                } else {
                    unreachable!()
                }
            ));
        }
        None
    }

    fn get_doctype_data(&self) -> Option<Ref<Self::DocTypeData>> {
        let borrowed_data = self.data.borrow();

        if let NodeDataTypeInternal::DocType(_) = *borrowed_data {
            return Some(Ref::map(borrowed_data, |d|
                if let NodeDataTypeInternal::DocType(ref text_data) = d {
                    text_data
                } else {
                    unreachable!()
                }
            ));
        }
        None
    }

    fn handle(&self) -> DocumentHandle<Self::Document, C> {
        self.document.clone()
    }

    fn remove(&mut self, node_id: NodeId) {
        self.children = self.children.iter().filter(|&x| x != &node_id).cloned().collect();
    }

    fn insert(&mut self, node_id: NodeId, idx: usize) {
        self.children.insert(idx, node_id);
    }

    fn push(&mut self, node_id: NodeId) {
        self.children.push(node_id);
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
            location: self.location.clone(),
        }
    }
}

impl<C: CssSystem> NodeImpl<C> {
    /// create a new `Node`
    #[must_use]
    pub fn new(document: DocumentHandle<DocumentImpl<C>, C>, location: Location, data: &NodeDataTypeInternal<C>) -> Self {
        let (id, parent, children, is_registered) = <_>::default();
        Self {
            id,
            parent,
            children,
            data: data.clone().into(),
            document: document.clone(),
            is_registered,
            location,
        }
    }

    // /// Create a new document node
    // #[must_use]
    // pub fn new_document(document: DocumentHandle<DocumentImpl>, location: Location, quirks_mode: QuirksMode) -> Self {
    //     Self::new(document, location, &NodeDataTypeInternal::Document(DocumentData::new(quirks_mode)))
    // }
    //
    // #[must_use]
    // pub fn new_doctype(
    //     document: DocumentHandle<DocumentImpl>,
    //     location: Location,
    //     name: &str,
    //     pub_identifier: &str,
    //     sys_identifier: &str,
    // ) -> Self {
    //     Self::new(
    //         document,
    //         location,
    //         &NodeDataTypeInternal::DocType(DocTypeData::new(name, pub_identifier, sys_identifier)),
    //     )
    // }
    //
    // /// Create a new element node with the given name and attributes and namespace
    // #[must_use]
    // pub fn new_element(
    //     document: DocumentHandle<DocumentImpl>,
    //     location: Location,
    //     name: &str,
    //     namespace: Option<&str>,
    //     attributes: HashMap<String, String>,
    // ) -> Self {
    //     Self::new(
    //         document,
    //         location,
    //         &NodeDataTypeInternal::Element(ElementData::new(
    //             name,
    //             namespace,
    //             attributes,
    //             Default::default(),
    //         ))
    //     )
    // }
    //
    // /// Creates a new comment node
    // #[must_use]
    // pub fn new_comment(document: DocumentHandle<DocumentImpl>, location: Location, value: &str) -> Self {
    //     Self::new(
    //         document.clone(),
    //         location,
    //         &NodeDataTypeInternal::Comment(CommentData::with_value(value)),
    //     )
    // }
    //
    // /// Creates a new text node
    // #[must_use]
    // pub fn new_text(document: DocumentHandle<DocumentImpl>, location: Location, value: &str) -> Self {
    //     Self::new(
    //         document.clone(),
    //         location,
    //         &NodeDataTypeInternal::Text(TextData::with_value(value)),
    //     )
    // }


    /// Returns true if this node is registered into an arena
    pub fn is_registered(&self) -> bool {
        self.is_registered
    }

    // pub fn is_text(&self) -> bool {
    //     if let NodeDataTypeInternal::Text(_) = *self.data.borrow() {
    //         return true;
    //     }
    //
    //     false
    // }
    //
    // pub fn as_text(&self) -> &TextData {
    //     if let NodeDataTypeInternal::Text(text) = &self.data {
    //         return text;
    //     }
    //
    //     panic!("Node is not a text");
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::document::Document;

    #[test]
    fn new_document() {
        let document = Document::shared(None);
        let node = Node::new_document(&document, Location::default());
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
        assert_eq!(node.name, "".to_string());
        assert_eq!(node.namespace, None);
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
        let node = Node::new_element(
            &document,
            "div",
            attributes.clone(),
            HTML_NAMESPACE,
            Location::default(),
        );
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
        assert_eq!(node.name, "div".to_string());
        assert_eq!(node.namespace, Some(HTML_NAMESPACE.into()));
        let NodeData::Element(element) = &node.data else {
            panic!()
        };
        assert_eq!(element.name(), "div");
        assert!(element.attributes().contains_key("id"));
        assert_eq!(element.attributes().get("id").unwrap(), "test");
    }

    #[test]
    fn new_comment() {
        let document = Document::shared(None);
        let node = Node::new_comment(&document, Location::default(), "test");
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
        assert_eq!(node.name, "".to_string());
        assert_eq!(node.namespace, None);
        let NodeData::Comment(CommentData { value, .. }) = &node.data else {
            panic!()
        };
        assert_eq!(value, "test");
    }

    #[test]
    fn new_text() {
        let document = Document::shared(None);
        let node = Node::new_text(&document, Location::default(), "test");
        assert_eq!(node.id, NodeId::default());
        assert_eq!(node.parent, None);
        assert!(node.children.is_empty());
        assert_eq!(node.name, "".to_string());
        assert_eq!(node.namespace, None);
        let NodeData::Text(TextData { value }) = &node.data else {
            panic!()
        };
        assert_eq!(value, "test");
    }

    #[test]
    fn is_special() {
        let mut attributes = HashMap::new();
        attributes.insert("id".to_string(), "test".to_string());
        let document = Document::shared(None);
        let node = Node::new_element(
            &document,
            "div",
            attributes,
            HTML_NAMESPACE,
            Location::default(),
        );
        assert!(node.is_special());
    }

    #[test]
    fn type_of() {
        let document = Document::shared(None);
        let node = Node::new_document(&document, Location::default());
        assert_eq!(node.type_of(), NodeType::DocumentNode);
        let node = Node::new_text(&document, Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::TextNode);
        let node = Node::new_comment(&document, Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::CommentNode);
        let mut attributes = HashMap::new();
        attributes.insert("id".to_string(), "test".to_string());
        let node = Node::new_element(
            &document,
            "div",
            attributes,
            HTML_NAMESPACE,
            Location::default(),
        );
        assert_eq!(node.type_of(), NodeType::ElementNode);
    }

    #[test]
    fn special_html_elements() {
        let document = Document::shared(None);

        for element in SPECIAL_HTML_ELEMENTS.iter() {
            let mut attributes = HashMap::new();
            attributes.insert("id".to_string(), "test".to_string());
            let node = Node::new_element(
                &document,
                element,
                attributes,
                HTML_NAMESPACE,
                Location::default(),
            );
            assert!(node.is_special());
        }
    }

    #[test]
    fn special_mathml_elements() {
        let document = Document::shared(None);
        for element in SPECIAL_MATHML_ELEMENTS.iter() {
            let mut attributes = HashMap::new();
            attributes.insert("id".to_string(), "test".to_string());
            let node = Node::new_element(
                &document,
                element,
                attributes,
                MATHML_NAMESPACE,
                Location::default(),
            );
            assert!(node.is_special());
        }
    }

    #[test]
    fn special_svg_elements() {
        let document = Document::shared(None);
        for element in SPECIAL_SVG_ELEMENTS.iter() {
            let mut attributes = HashMap::new();
            attributes.insert("id".to_string(), "test".to_string());
            let node = Node::new_element(
                &document,
                element,
                attributes,
                SVG_NAMESPACE,
                Location::default(),
            );
            assert!(node.is_special());
        }
    }

    #[test]
    fn type_of_node() {
        let document = Document::shared(None);
        let node = Node::new_document(&document, Location::default());
        assert_eq!(node.type_of(), NodeType::DocumentNode);
        let node = Node::new_text(&document, Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::TextNode);
        let node = Node::new_comment(&document, Location::default(), "test");
        assert_eq!(node.type_of(), NodeType::CommentNode);
        let mut attributes = HashMap::new();
        attributes.insert("id".to_string(), "test".to_string());
        let node = Node::new_element(
            &document,
            "div",
            attributes,
            HTML_NAMESPACE,
            Location::default(),
        );
        assert_eq!(node.type_of(), NodeType::ElementNode);
    }
}

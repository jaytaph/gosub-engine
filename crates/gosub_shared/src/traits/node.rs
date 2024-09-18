use crate::byte_stream::Location;
use crate::document::DocumentHandle;
use crate::node::NodeId;
use crate::traits::css3::CssSystem;
use crate::traits::document::Document;
use crate::traits::document::DocumentFragment;
use std::collections::HashMap;

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum QuirksMode {
    Quirks,
    LimitedQuirks,
    NoQuirks,
}

/// Different types of nodes that all have their own data structures (NodeData)
#[derive(Debug, PartialEq)]
pub enum NodeType {
    DocumentNode,
    DocTypeNode,
    TextNode,
    CommentNode,
    ElementNode,
}

/// Different types of nodes
#[derive(Debug, PartialEq)]
pub enum NodeData<'a, C: CssSystem, N: Node<C>> {
    Document(&'a N::DocumentData),
    DocType(&'a N::DocTypeData),
    Text(&'a N::TextData),
    Comment(&'a N::CommentData),
    Element(&'a N::ElementData),
}

impl<C: CssSystem, N: Node<C>> Copy for NodeData<'_, C, N> {}

impl<C: CssSystem, N: Node<C>> Clone for NodeData<'_, C, N> {
    fn clone(&self) -> Self {
        *self
    }
}

pub trait DocumentDataType {
    fn quirks_mode(&self) -> QuirksMode;
    fn set_quirks_mode(&mut self, quirks_mode: QuirksMode);
}

pub trait DocTypeDataType {
    fn name(&self) -> &str;
    fn pub_identifier(&self) -> &str;
    fn sys_identifier(&self) -> &str;
}

pub trait TextDataType {
    fn value(&self) -> &str;

    fn string_value(&self) -> String;
    fn value_mut(&mut self) -> &mut String;
}

pub trait CommentDataType {
    fn value(&self) -> &str;
}

pub trait ElementDataType<C: CssSystem> {
    type Document: Document<C>;
    type DocumentFragment: DocumentFragment<C>;

    /// Returns the name of the element
    fn name(&self) -> &str;
    /// Returns the namespace
    fn namespace(&self) -> &str;
    /// Returns true if the namespace matches the element
    fn is_namespace(&self, namespace: &str) -> bool;
    /// Returns the classes of the element
    fn classes(&self) -> Vec<String>;
    /// Returns the active classes of the element
    fn active_classes(&self) -> Vec<String>;
    /// Returns the given attribute (or None when not found)
    fn attribute(&self, name: &str) -> Option<&String>;
    /// Returns all attributes of the element
    fn attributes(&self) -> &HashMap<String, String>;
    /// Returns mutable attributes of the element
    fn attributes_mut(&mut self) -> &mut HashMap<String, String>;

    fn matches_tag_and_attrs_without_order(&self, other_data: &Self) -> bool;
    fn is_mathml_integration_point(&self) -> bool;
    fn is_html_integration_point(&self) -> bool;

    /// Returns true if this is a "special" element node
    fn is_special(&self) -> bool;
    /// Add a class to the element
    fn add_class(&mut self, class: &str);
    // Return the template document of the element
    fn template_contents(&self) -> Option<&Self::DocumentFragment>;
    /// Returns true if the given node is a "formatting" node
    fn is_formatting(&self) -> bool;

    fn set_template_contents(&mut self, template_contents: Self::DocumentFragment);
}

pub trait Node<C: CssSystem>: Clone + PartialEq {
    type Document: Document<C>;
    type DocumentData: DocumentDataType;
    type DocTypeData: DocTypeDataType;
    type TextData: TextDataType;
    type CommentData: CommentDataType;
    type ElementData: ElementDataType<
        C,
        Document = Self::Document,
        DocumentFragment = <Self::Document as Document<C>>::Fragment,
    >;

    /// Return the ID of the node
    fn id(&self) -> NodeId;
    /// Sets the ID of the node
    fn set_id(&mut self, id: NodeId);
    /// Returns the location of the node
    fn location(&self) -> Location;
    /// Returns the ID of the parent node or None when the node is the root
    fn parent_id(&self) -> Option<NodeId>;
    /// Sets the parent of the node, or None when the node is the root
    fn set_parent(&mut self, parent_id: Option<NodeId>);

    /// Returns true when this node is the root node
    fn is_root(&self) -> bool;
    /// Returns the children of the node
    fn children(&self) -> &[NodeId];

    /// Returns the type of the node
    fn type_of(&self) -> NodeType;

    fn is_element_node(&self) -> bool;
    fn get_element_data(&self) -> Option<&Self::ElementData>;
    fn get_element_data_mut(&mut self) -> Option<&mut Self::ElementData>;

    fn is_text_node(&self) -> bool;
    fn get_text_data(&self) -> Option<&Self::TextData>;
    fn get_text_data_mut(&mut self) -> Option<&mut Self::TextData>;

    fn get_comment_data(&self) -> Option<&Self::CommentData>;
    fn get_doctype_data(&self) -> Option<&Self::DocTypeData>;

    /// Returns the document handle of the node
    fn handle(&self) -> DocumentHandle<Self::Document, C>;
    /// Removes a child node from the node
    fn remove(&mut self, node_id: NodeId);
    /// Inserts a child node to the node at a specific index
    fn insert(&mut self, node_id: NodeId, idx: usize);
    /// Pushes a child node to the node
    fn push(&mut self, node_id: NodeId);

    fn data(&self) -> NodeData<C, Self>;
}

use gosub_css3::stylesheet::CssStylesheet;
use gosub_shared::traits::document::{Document as OtherDocument, Document, DocumentType};
use crate::DocumentHandle;
use core::fmt::Debug;
use std::collections::HashMap;
use url::Url;

use gosub_shared::byte_stream::Location;
use gosub_shared::node::NodeId;
use gosub_shared::traits::node::Node;
use gosub_shared::traits::node::QuirksMode;
use crate::node::arena::NodeArena;
use crate::node::data::comment::CommentData;
use crate::node::data::doctype::DocTypeData;
use crate::node::data::document::DocumentData;
use crate::node::data::element::{ElementClass, ElementData};
use crate::node::data::text::TextData;
use crate::node::node::{NodeDataTypeInternal, NodeImpl};
use crate::node::visitor::Visitor;

/// according to HTML spec:
/// https://html.spec.whatwg.org/#global-attributes
pub(crate) fn is_valid_id_attribute_value(value: &str) -> bool {
    !(value.is_empty() || value.contains(|ref c| char::is_ascii_whitespace(c)))
}

/// Defines a document
#[derive(Debug)]
pub struct DocumentImpl {
    // pub handle: Weak<DocumentHandle<Self>>,

    /// URL of the given document (if any)
    pub url: Option<Url>,
    /// Holds and owns all nodes in the document
    pub(crate) arena: NodeArena<NodeImpl>,
    /// HTML elements with ID (e.g., <div id="myid">)
    named_id_elements: HashMap<String, NodeId>,
    /// Document type of this document
    pub doctype: DocumentType,
    /// Quirks mode of this document
    pub quirks_mode: QuirksMode,
    /// Loaded stylesheets as extracted from the document
    pub stylesheets: Vec<CssStylesheet>,
}

impl PartialEq for DocumentImpl {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
            && self.arena == other.arena
            && self.named_id_elements == other.named_id_elements
            && self.doctype == other.doctype
            && self.quirks_mode == other.quirks_mode
            && self.stylesheets == other.stylesheets
    }
}

impl Default for DocumentImpl {
    /// Returns a default document
    fn default() -> Self {
        Self::new(DocumentType::HTML, None, None)
    }
}

impl Document for DocumentImpl {
    type Node = NodeImpl;

    // fn new_with_handle(document_type: DocumentType, url: Option<Url>, location: &Location, root_node: Option<&Self::Node>) -> DocumentHandle<Self> {
    //     let mut doc = Self::new(document_type, url);
    //     let handle = DocumentHandle::create(doc);
    //
    //     if let Some(root_node) = root_node {
    //         handle.get().register_node(root_node);
    //     } else {
    //         let root_node = NodeImpl::new_document(handle.clone(), location.clone());
    //         handle.get().register_node(&root_node);
    //     }
    //
    //     handle
    // }

    /// Creates a new document without a doc handle
    #[must_use]
    fn new(document_type: DocumentType, url: Option<Url>, root_node: Option<&Self::Node>) -> Self {
        let mut doc = Self {
            url,
            arena: NodeArena::new(),
            named_id_elements: HashMap::new(),
            doctype: document_type,
            quirks_mode: QuirksMode::NoQuirks,
            stylesheets: Vec::new(),
        };

        if let Some(root_node) = root_node {
            doc.register_node(root_node);
        }

        doc
    }

    fn handle(&self) -> DocumentHandle<DocumentImpl> {
        todo!("handle() implementation")
        // self.handle.upgrade().expect("failure").get().unwrap()
    }


    /// Returns the URL of the document, or "" when no location is set
    fn url(&self) -> Option<Url> {
        match self.url {
            Some(ref url) => Some(url.clone()),
            None => None,
        }
    }

    // // Returns a shared reference-counted handle for the document
    // fn new_with_dochandle(document_type: DocumentType, location: Option<Url>) -> DocumentHandle<DocumentImpl> {
    //     let doc = Self::new(document_type, location);
    //     DocumentHandle::<DocumentImpl>(Rc::new(RefCell::new(doc)))
    // }

    fn set_quirks_mode(&mut self, quirks_mode: QuirksMode) {
        self.quirks_mode = quirks_mode;
    }

    fn quirks_mode(&self) -> QuirksMode {
        self.quirks_mode
    }

    fn set_doctype(&mut self, doctype: DocumentType) {
        self.doctype = doctype;
    }

    fn doctype(&self) -> DocumentType {
        self.doctype
    }

    /// Fetches a node by id or returns None when no node with this ID is found
    fn node_by_id(&self, node_id: NodeId) -> Option<&Self::Node> {
        self.arena.node(node_id)
    }

    /// Fetches a mutable node by id or returns None when no node with this ID is found
    fn get_node_by_id_mut(&mut self, node_id: NodeId) -> Option<&mut Self::Node> {
        self.arena.node_mut(node_id)
    }

    /// returns the root node
    fn get_root(&self) -> &Self::Node {
        self.arena
            .node(NodeId::root())
            .expect("Root node not found !?")
    }

    /// returns the root node
    fn get_root_mut(&mut self) -> &mut Self::Node {
        self.arena
            .node_mut(NodeId::root())
            .expect("Root node not found !?")
    }

    fn attach_node(&mut self, node_id: NodeId, parent_id: NodeId, position: Option<usize>) {
        //check if any children of node have parent as child
        if parent_id == node_id || self.has_node_id_recursive(node_id, parent_id) {
            return
        }

        if let Some(parent_node) = self.get_node_by_id_mut(parent_id) {
            // Make sure position can never be larger than the number of children in the parent
            if let Some(mut position) = position {
                if position > parent_node.children().len() {
                    position = parent_node.children().len();
                }
                parent_node.insert(node_id, position);
            } else {
                // No position given, add to end of the children list
                parent_node.push(node_id);
            }
        }

        let node = self.arena.node_mut(node_id).unwrap();
        node.parent = Some(parent_id);
    }

    fn detach_node(&mut self, node_id: NodeId) {
        let parent = self.node_by_id(node_id).expect("node not found").parent_id();

        if let Some(parent_id) = parent {
            let parent_node = self
                .get_node_by_id_mut(parent_id)
                .expect("parent node not found");
            parent_node.remove(node_id);

            let node = self.get_node_by_id_mut(node_id).expect("node not found");
            node.set_parent(None);
        }
    }

    /// Relocates a node to another parent node
    fn relocate_node(&mut self, node_id: NodeId, parent_id: NodeId) {
        let node = self.arena.node_mut(node_id).unwrap();
        assert!(node.is_registered, "Node is not registered to the arena");

        if node.parent.is_some() && node.parent.unwrap() == parent_id {
            // Nothing to do when we want to relocate to its own parent
            return;
        }

        self.detach_node(node_id);
        self.attach_node(node_id, parent_id, None);
    }

    /// Returns the parent node of the given node, or None when no parent is found
    fn parent_node(&self, node: &Self::Node) -> Option<&Self::Node> {
        match node.parent_id() {
            Some(parent_node_id) => self.node_by_id(parent_node_id),
            None => None,
        }
    }

    /// Removes a node by id from the arena. Note that this does not check the nodelist itself to see
    /// if the node is still available as a child or parent in the tree.
    fn delete_node_by_id(&mut self, node_id: NodeId) {
        let node = self.arena.node(node_id).unwrap();
        let parent_id = node.parent_id().clone();

        match parent_id {
            Some(parent_id) => {
                let parent = self.get_node_by_id_mut(parent_id).unwrap();
                parent.remove(node_id);
            }
            None => {}
        }

        self.arena.delete_node(node_id);
    }

    /// Retrieves the next sibling NodeId (to the right) of the reference_node or None.
    fn get_next_sibling(&self, reference_node: NodeId) -> Option<NodeId> {
        let node = self.node_by_id(reference_node)?;
        let parent = self.node_by_id(node.parent_id()?)?;

        let idx = parent
            .children()
            .iter()
            .position(|&child_id| child_id == reference_node)?;

        let next_idx = idx + 1;
        if parent.children().len() > next_idx {
            return Some(parent.children()[next_idx]);
        }

        None
    }

    /// Register a node
    fn register_node(&mut self, _node: &Self::Node) -> NodeId {
        todo!("register_node() not implemented");
    }

    /// Inserts a node to the parent node at the given position in the children (or none
    /// to add at the end). Will automatically register the node if not done so already
    fn register_node_at(&mut self, node: &Self::Node, parent_id: NodeId, position: Option<usize>) -> NodeId {
        let node_id = self.register_node(node);
        self.attach_node(node_id, parent_id, position);

        node_id
    }

    /// Creates a new document node
    fn new_document_node(handle: DocumentHandle<Self>, quirks_mode: QuirksMode, location: Location) -> Self::Node {
        NodeImpl::new(
            handle.clone(),
            location,
            &NodeDataTypeInternal::Document(DocumentData::new(quirks_mode))
        )
    }

    fn new_doctype_node(handle: DocumentHandle<Self>, name: &str, public_id: Option<&str>, system_id: Option<&str>, location: Location) -> Self::Node {
        NodeImpl::new(
            handle.clone(),
            location,
            &NodeDataTypeInternal::DocType(DocTypeData::new(name, public_id.unwrap_or(""), system_id.unwrap_or("")))
        )
    }

    /// Creates a new comment node
    fn new_comment_node(handle: DocumentHandle<Self>, comment: &str, location: Location) -> Self::Node {
        NodeImpl::new(
            handle.clone(),
            location,
            &NodeDataTypeInternal::Comment(CommentData::with_value(comment))
        )
    }

    /// Creates a new text node
    fn new_text_node(handle: DocumentHandle<Self>, value: &str, location: Location) -> Self::Node {
        NodeImpl::new(
            handle.clone(),
            location,
            &NodeDataTypeInternal::Text(TextData::with_value(value))
        )
    }

    /// Creates a new element node
    fn new_element_node(handle: DocumentHandle<Self>, name: &str, namespace: Option<&str>, attributes: HashMap<String, String>, location: Location) -> Self::Node {
        NodeImpl::new(
            handle.clone(),
            location,
            &NodeDataTypeInternal::Element(ElementData::new(name, namespace, attributes, ElementClass::default()))
        )
    }
}

impl DocumentImpl {

    /// Fetches a node by named id (string) or returns None when no node with this ID is found
    pub fn get_node_by_named_id<D>(&self, named_id: &str) -> Option<&D::Node>
    where
        D: Document<Node=NodeImpl>
    {
        let node_id = self.named_id_elements.get(named_id)?;
        self.arena.node(*node_id)
    }

    /// Fetches a mutable node by named id (string) or returns None when no node with this ID is found
    pub fn get_node_by_named_id_mut<D>(&mut self, named_id: &str) -> Option<&mut D::Node>
    where
        D: Document<Node=NodeImpl>
    {
        let node_id = self.named_id_elements.get(named_id)?;
        self.arena.node_mut(*node_id)
    }

    // pub fn count_nodes(&self) -> usize {
    //     self.arena.count_nodes()
    // }

    pub fn has_node_id_recursive(&self, parent_id: NodeId, target_node_id: NodeId) -> bool {
        let parent = self.arena.node(parent_id).cloned();
        if parent.is_none() {
            return false;
        }

        let parent = parent.unwrap();
        for child_node_id in parent.children() {
            if *child_node_id == target_node_id {
                return true;
            }
            if self.has_node_id_recursive(*child_node_id, target_node_id) {
                return true;
            }
        }

        false
    }

    pub fn peek_next_id(&self) -> NodeId {
        self.arena.peek_next_id()
    }

    /// Check if a given node's children contain a certain tag name
    pub fn contains_child_tag(&self, node_id: NodeId, tag: &str) -> bool {
        if let Some(node) = self.node_by_id(node_id) {
            for child_id in &node.children().to_vec() {
                if let Some(child) = self.node_by_id(*child_id) {
                    if let Some(data) = child.get_element_data() {
                        return data.name == tag;
                    }
                }
            }
        }

        false
    }

    pub fn nodes<D>(&self) -> &HashMap<NodeId, D::Node>
    where
        D: Document<Node=NodeImpl>
    {
        self.arena.nodes()
    }
}

// Walk the document tree with the given visitor
pub fn walk_document_tree<D: Document>(handle: DocumentHandle<D>, visitor: &mut Box<dyn Visitor<D::Node>>) {
    let binding = handle.get();
    let root = binding.get_root();
    internal_visit(handle.clone(), root, visitor);
}

fn internal_visit<D: Document>(handle: DocumentHandle<D>, node: &D::Node, visitor: &mut Box<dyn Visitor<D::Node>>) {
    visitor.document_enter(&node);

    let binding = handle.get();
    for child_id in node.children() {
        let child = binding.node_by_id(*child_id).unwrap();
        internal_visit(handle.clone(), child, visitor);
    }

    // Leave node
    visitor.document_leave(&node);
}

/// Constructs an iterator from a given DocumentHandle.
/// WARNING: mutations in the document would be reflected
/// in the iterator. It's advised to consume the entire iterator
/// before mutating the document again.
pub struct TreeIterator<D: Clone + Document> {
    current_node_id: Option<NodeId>,
    node_stack: Vec<NodeId>,
    document: DocumentHandle<D>,
}

impl<D: Document + Clone> TreeIterator<D> {
    #[must_use]
    pub fn new(doc: DocumentHandle<D>) -> Self {
        Self {
            current_node_id: None,
            document: doc.clone(),
            node_stack: vec![doc.get().get_root().id()],
        }
    }
}

impl<D: Document + Clone> Iterator for TreeIterator<D> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        self.current_node_id = self.node_stack.pop();

        if let Some(current_node_id) = self.current_node_id {
            let doc_read = self.document.get();

            if let Some(sibling_id) = self.document.get().get_next_sibling(current_node_id) {
                self.node_stack.push(sibling_id);
            }

            if let Some(current_node) = doc_read.node_by_id(current_node_id) {
                if let Some(&child_id) = current_node.children().first() {
                    self.node_stack.push(child_id);
                }
            }
        }

        self.current_node_id
    }
}

#[cfg(test)]
mod tests {
    use crate::node::{NodeTrait, NodeType, HTML_NAMESPACE};
    use crate::parser::document::{DocumentBuilder, DocumentTaskQueue, TreeIterator};
    use crate::parser::query::Query;
    use crate::parser::tree_builder::TreeBuilder;
    use crate::parser::{Node, NodeData, NodeId};
    use gosub_shared::byte_stream::Location;
    use std::collections::HashMap;

    #[test]
    fn relocate() {
        let mut document = DocumentBuilder::new_document(None);

        let parent = Node::new_element(
            &document,
            "parent",
            HashMap::new(),
            HTML_NAMESPACE,
            Location::default(),
        );
        let node1 = Node::new_element(
            &document,
            "div1",
            HashMap::new(),
            HTML_NAMESPACE,
            Location::default(),
        );
        let node2 = Node::new_element(
            &document,
            "div2",
            HashMap::new(),
            HTML_NAMESPACE,
            Location::default(),
        );
        let node3 = Node::new_element(
            &document,
            "div3",
            HashMap::new(),
            HTML_NAMESPACE,
            Location::default(),
        );
        let node3_1 = Node::new_element(
            &document,
            "div3_1",
            HashMap::new(),
            HTML_NAMESPACE,
            Location::default(),
        );

        let parent_id = document
            .get_mut()
            .add_node(parent, NodeId::from(0usize), None);
        let node1_id = document.get_mut().add_node(node1, parent_id, None);
        let node2_id = document.get_mut().add_node(node2, parent_id, None);
        let node3_id = document.get_mut().add_node(node3, parent_id, None);
        let node3_1_id = document.get_mut().add_node(node3_1, node3_id, None);

        assert_eq!(
            format!("{}", document),
            r#"└─ Document
   └─ <parent>
      ├─ <div1>
      ├─ <div2>
      └─ <div3>
         └─ <div3_1>
"#
        );

        document.get_mut().relocate(node3_1_id, node1_id);
        assert_eq!(
            format!("{}", document),
            r#"└─ Document
   └─ <parent>
      ├─ <div1>
      │  └─ <div3_1>
      ├─ <div2>
      └─ <div3>
"#
        );

        document.get_mut().relocate(node1_id, node2_id);
        assert_eq!(
            format!("{}", document),
            r#"└─ Document
   └─ <parent>
      ├─ <div2>
      │  └─ <div1>
      │     └─ <div3_1>
      └─ <div3>
"#
        );
    }

    #[test]
    fn duplicate_named_id_elements() {
        let mut document = DocumentBuilder::new_document(None);

        let div_1 = document.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_2 = document.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        // when adding duplicate IDs, our current implementation will prevent duplicates.
        let mut res = document.insert_attribute("id", "myid", div_1, Location::default());
        assert!(res.is_ok());

        res = document.insert_attribute("id", "myid", div_2, Location::default());
        assert!(res.is_err());
        if let Err(err) = res {
            assert_eq!(
                err.to_string(),
                "document task error: ID 'myid' already exists in DOM"
            );
        }

        assert_eq!(
            document.get().get_node_by_named_id("myid").unwrap().id,
            div_1
        );

        // when div_1's ID changes, "myid" should be removed from the DOM
        res = document.insert_attribute("id", "newid", div_1, Location::default());
        assert!(res.is_ok());
        assert!(document.get().get_node_by_named_id("myid").is_none());
        assert_eq!(
            document.get().get_node_by_named_id("newid").unwrap().id,
            div_1
        );
    }

    #[test]
    fn verify_node_ids_in_element_data() {
        let mut document = DocumentBuilder::new_document(None);

        let node1 = Node::new_element(
            &document,
            "div",
            HashMap::new(),
            HTML_NAMESPACE,
            Location::default(),
        );
        let node2 = Node::new_element(
            &document,
            "div",
            HashMap::new(),
            HTML_NAMESPACE,
            Location::default(),
        );

        document
            .get_mut()
            .add_node(node1, NodeId::from(0usize), None);
        document
            .get_mut()
            .add_node(node2, NodeId::from(0usize), None);

        let doc_ptr = document.get();

        let get_node1 = doc_ptr.get_node_by_id(NodeId::from(1usize)).unwrap();
        let get_node2 = doc_ptr.get_node_by_id(NodeId::from(2usize)).unwrap();

        let NodeData::Element(element1) = &get_node1.data else {
            panic!()
        };

        assert_eq!(element1.node_id(), NodeId::from(1usize));

        let NodeData::Element(element2) = &get_node2.data else {
            panic!()
        };

        assert_eq!(element2.node_id(), NodeId::from(2usize));
    }

    #[test]
    fn document_task_queue() {
        let document = DocumentBuilder::new_document(None);

        // Using task queue to create the following structure initially:
        // <div>
        //   <p>
        //     <!-- comment inside p -->
        //     hey
        //   </p>
        //   <!-- comment inside div -->
        // </div>

        // then flush the queue and use it again to add an attribute to <p>:
        // <p id="myid">hey</p>
        let mut task_queue = DocumentTaskQueue::new(&document);

        // NOTE: only elements return the ID
        let div_id = task_queue.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        assert_eq!(div_id, NodeId::from(1usize));

        let p_id =
            task_queue.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());
        assert_eq!(p_id, NodeId::from(2usize));

        task_queue.create_comment("comment inside p", p_id, Location::default());
        task_queue.create_text("hey", p_id, Location::default());
        task_queue.create_comment("comment inside div", div_id, Location::default());

        // at this point, the DOM should have NO nodes (besides root)
        assert_eq!(document.get().arena.count_nodes(), 1);

        // validate our queue is loaded
        assert!(!task_queue.is_empty());
        let errors = task_queue.flush();
        assert!(errors.is_empty());

        // validate queue is empty
        assert!(task_queue.is_empty());

        // DOM should now have all our nodes
        assert_eq!(document.get().arena.count_nodes(), 6);

        // NOTE: these checks are scoped separately since this is using an
        // immutable borrow, and we make a mutable borrow after (to insert the attribute).
        // We need this immutable borrow to die off before making a new mutable borrow
        // (and again an immutable borrow for validation afterward)
        {
            // validate DOM is correctly laid out
            let doc_read = document.get();
            let root = doc_read.get_root(); // <!DOCTYPE html>
            let root_children = &root.children;

            // div child
            let div_child = doc_read.get_node_by_id(root_children[0]).unwrap();
            assert_eq!(div_child.type_of(), NodeType::ElementNode);
            assert_eq!(div_child.name, "div");
            let div_children = &div_child.children;

            // p child
            let p_child = doc_read.get_node_by_id(div_children[0]).unwrap();
            assert_eq!(p_child.type_of(), NodeType::ElementNode);
            assert_eq!(p_child.name, "p");
            let p_children = &p_child.children;

            // comment inside p
            let p_comment = doc_read.get_node_by_id(p_children[0]).unwrap();
            assert_eq!(p_comment.type_of(), NodeType::CommentNode);
            let NodeData::Comment(p_comment_data) = &p_comment.data else {
                panic!()
            };
            assert_eq!(p_comment_data.value, "comment inside p");

            // body inside p
            let p_body = doc_read.get_node_by_id(p_children[1]).unwrap();
            assert_eq!(p_body.type_of(), NodeType::TextNode);
            let NodeData::Text(p_body_data) = &p_body.data else {
                panic!()
            };
            assert_eq!(p_body_data.value, "hey");

            // comment inside div
            let div_comment = doc_read.get_node_by_id(div_children[1]).unwrap();
            assert_eq!(div_comment.type_of(), NodeType::CommentNode);
            let NodeData::Comment(div_comment_data) = &div_comment.data else {
                panic!()
            };
            assert_eq!(div_comment_data.value, "comment inside div");
        }

        // use task queue again to add an ID attribute
        // NOTE: inserting attribute in task queue always succeeds
        // since it doesn't touch DOM until flush
        let _ = task_queue.insert_attribute("id", "myid", p_id, Location::default());
        let errors = task_queue.flush();
        assert!(errors.is_empty());

        let doc_read = document.get();
        // validate ID is searchable in dom
        assert_eq!(*doc_read.named_id_elements.get("myid").unwrap(), p_id);

        // validate attribute is applied to underlying element
        let p_node = doc_read.get_node_by_id(p_id).unwrap();
        let NodeData::Element(p_element) = &p_node.data else {
            panic!()
        };
        assert_eq!(p_element.attributes().get("id").unwrap(), "myid");
    }

    #[test]
    fn task_queue_insert_attribute_failues() {
        let document = DocumentBuilder::new_document(None);

        let mut task_queue = DocumentTaskQueue::new(&document);
        let div_id = task_queue.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        task_queue.create_comment("content", div_id, Location::default()); // this is NodeId::from(2)
        task_queue.flush();

        // NOTE: inserting attribute in task queue always succeeds
        // since it doesn't touch DOM until flush
        let _ =
            task_queue.insert_attribute("id", "myid", NodeId::from(1usize), Location::default());
        let _ =
            task_queue.insert_attribute("id", "myid", NodeId::from(1usize), Location::default());
        let _ =
            task_queue.insert_attribute("id", "otherid", NodeId::from(2usize), Location::default());
        let _ = task_queue.insert_attribute(
            "id",
            "dummyid",
            NodeId::from(42usize),
            Location::default(),
        );
        let _ =
            task_queue.insert_attribute("id", "my id", NodeId::from(1usize), Location::default());
        let _ = task_queue.insert_attribute("id", "123", NodeId::from(1usize), Location::default());
        let _ = task_queue.insert_attribute("id", "", NodeId::from(1usize), Location::default());
        let errors = task_queue.flush();
        for error in &errors {
            println!("{}", error);
        }
        assert_eq!(errors.len(), 5);
        assert_eq!(
            errors[0],
            "document task error: ID 'myid' already exists in DOM",
        );
        assert_eq!(
            errors[1],
            "document task error: Node ID 2 is not an element",
        );
        assert_eq!(errors[2], "document task error: Node ID 42 not found");
        assert_eq!(
            errors[3],
            "document task error: Attribute value 'my id' did not pass validation",
        );
        assert_eq!(
            errors[4],
            "document task error: Attribute value '' did not pass validation",
        );

        // validate that invalid changes did not apply to DOM
        let doc_read = document.get();
        assert!(!doc_read.named_id_elements.contains_key("my id"));
        assert!(!doc_read.named_id_elements.contains_key(""));
    }

    // this is basically a replica of document_task_queue() test
    // but using tree builder directly instead of the task queue
    #[test]
    fn document_tree_builder() {
        let mut document = DocumentBuilder::new_document(None);

        // Using tree builder to create the following structure:
        // <div>
        //   <p id="myid">
        //     <!-- comment inside p -->
        //     hey
        //   </p>
        //   <!-- comment inside div -->
        // </div>

        // NOTE: only elements return the ID
        let div_id = document.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        assert_eq!(div_id, NodeId::from(1usize));

        let p_id = document.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());
        assert_eq!(p_id, NodeId::from(2usize));

        document.create_comment("comment inside p", p_id, Location::default());
        document.create_text("hey", p_id, Location::default());
        document.create_comment("comment inside div", div_id, Location::default());

        let res = document.insert_attribute("id", "myid", p_id, Location::default());
        assert!(res.is_ok());

        // DOM should now have all our nodes
        assert_eq!(document.get().arena.count_nodes(), 6);

        // validate DOM is correctly laid out
        let doc_read = document.get();
        let root = doc_read.get_root(); // <!DOCTYPE html>
        let root_children = &root.children;

        // div child
        let div_child = doc_read.get_node_by_id(root_children[0]).unwrap();
        assert_eq!(div_child.type_of(), NodeType::ElementNode);
        assert_eq!(div_child.name, "div");
        let div_children = &div_child.children;

        // p child
        let p_child = doc_read.get_node_by_id(div_children[0]).unwrap();
        assert_eq!(p_child.type_of(), NodeType::ElementNode);
        assert_eq!(p_child.name, "p");
        let p_children = &p_child.children;

        // comment inside p
        let p_comment = doc_read.get_node_by_id(p_children[0]).unwrap();
        assert_eq!(p_comment.type_of(), NodeType::CommentNode);
        let NodeData::Comment(p_comment_data) = &p_comment.data else {
            panic!()
        };
        assert_eq!(p_comment_data.value, "comment inside p");

        // body inside p
        let p_body = doc_read.get_node_by_id(p_children[1]).unwrap();
        assert_eq!(p_body.type_of(), NodeType::TextNode);
        let NodeData::Text(p_body_data) = &p_body.data else {
            panic!()
        };
        assert_eq!(p_body_data.value, "hey");

        // comment inside div
        let div_comment = doc_read.get_node_by_id(div_children[1]).unwrap();
        assert_eq!(div_comment.type_of(), NodeType::CommentNode);
        let NodeData::Comment(div_comment_data) = &div_comment.data else {
            panic!()
        };
        assert_eq!(div_comment_data.value, "comment inside div");

        // validate ID is searchable in dom
        assert_eq!(*doc_read.named_id_elements.get("myid").unwrap(), p_id);

        // validate attribute is applied to underlying element
        let p_node = doc_read.get_node_by_id(p_id).unwrap();
        let NodeData::Element(p_element) = &p_node.data else {
            panic!()
        };
        assert_eq!(p_element.attributes().get("id").unwrap(), "myid");
    }

    #[test]
    fn insert_generic_attribute() {
        let mut doc = DocumentBuilder::new_document(None);
        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let res = doc.insert_attribute("key", "value", div_id, Location::default());
        assert!(res.is_ok());
        let doc_read = doc.get();
        let NodeData::Element(element) = &doc_read.get_node_by_id(div_id).unwrap().data else {
            panic!()
        };
        assert_eq!(element.attributes().get("key").unwrap(), "value");
    }

    #[test]
    fn task_queue_insert_generic_attribute() {
        let doc = DocumentBuilder::new_document(None);
        let mut task_queue = DocumentTaskQueue::new(&doc);
        let div_id = task_queue.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let _ = task_queue.insert_attribute("key", "value", div_id, Location::default());
        let errors = task_queue.flush();
        assert!(errors.is_empty());
        let doc_read = doc.get();
        let NodeData::Element(element) = &doc_read.get_node_by_id(div_id).unwrap().data else {
            panic!()
        };
        assert_eq!(element.attributes().get("key").unwrap(), "value");
    }

    #[test]
    fn insert_class_attribute() {
        let mut doc = DocumentBuilder::new_document(None);
        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let res = doc.insert_attribute("class", "one two three", div_id, Location::default());
        assert!(res.is_ok());
        let doc_read = doc.get();
        let NodeData::Element(element) = &doc_read.get_node_by_id(div_id).unwrap().data else {
            panic!()
        };
        assert!(element.classes().contains("one"));
        assert!(element.classes().contains("two"));
        assert!(element.classes().contains("three"));
    }

    #[test]
    fn task_queue_insert_class_attribute() {
        let doc = DocumentBuilder::new_document(None);
        let mut task_queue = DocumentTaskQueue::new(&doc);
        let div_id = task_queue.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let _ = task_queue.insert_attribute("class", "one two three", div_id, Location::default());
        let errors = task_queue.flush();
        assert!(errors.is_empty());
        let doc_read = doc.get();
        let NodeData::Element(element) = &doc_read.get_node_by_id(div_id).unwrap().data else {
            panic!()
        };
        assert!(element.classes().contains("one"));
        assert!(element.classes().contains("two"));
        assert!(element.classes().contains("three"));
    }

    #[test]
    fn uninitialized_query() {
        let doc = DocumentBuilder::new_document(None);

        let query = Query::new();
        let found_ids = doc.query(&query);
        if let Err(err) = found_ids {
            assert_eq!(
                err.to_string(),
                "query: generic error: Query predicate is uninitialized"
            );
        } else {
            panic!()
        }
    }

    #[test]
    fn single_query_equals_tag_find_first() {
        // <div>
        //     <div>
        //         <p>
        //     <p>
        // <div>
        //     <p>
        // <p>
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let _ = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let _ = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let query = Query::new().equals_tag("p").find_first();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 1);
        assert_eq!(found_ids, [p_id]);
    }

    #[test]
    fn single_query_equals_tag_find_all() {
        // <div>
        //     <div>
        //         <p>
        //     <p>
        // <div>
        //     <p>
        // <p>
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let p_id_2 = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let p_id_3 = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let p_id_4 = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let query = Query::new().equals_tag("p").find_all();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 4);
        assert_eq!(found_ids, [p_id, p_id_2, p_id_3, p_id_4]);
    }

    #[test]
    fn single_query_equals_id() {
        // <div>
        //     <div>
        //         <p>
        //     <p id="myid">
        // <div>
        //     <p>
        // <p>
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let p_id_2 = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());
        let res = doc.insert_attribute("id", "myid", p_id_2, Location::default());
        assert!(res.is_ok());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let _ = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let _ = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let query = Query::new().equals_id("myid").find_first();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 1);
        assert_eq!(found_ids, [p_id_2]);
    }

    #[test]
    fn single_query_contains_class_find_first() {
        // <div>
        //     <div>
        //         <p class="one two">
        //     <p class="one">
        // <div>
        //     <p class="two three">
        // <p class="three">
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let mut res = doc.insert_attribute("class", "one two", p_id, Location::default());
        assert!(res.is_ok());
        let p_id_2 = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());
        res = doc.insert_attribute("class", "one", p_id_2, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("id", "myid", p_id_2, Location::default());
        assert!(res.is_ok());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let p_id_3 = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());
        res = doc.insert_attribute("class", "two three", p_id_3, Location::default());
        assert!(res.is_ok());

        let p_id_4 = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        res = doc.insert_attribute("class", "three", p_id_4, Location::default());
        assert!(res.is_ok());

        let query = Query::new().contains_class("two").find_first();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 1);
        assert_eq!(found_ids, [p_id]);
    }

    #[test]
    fn single_query_contains_class_find_all() {
        // <div>
        //     <div>
        //         <p class="one two">
        //     <p class="one">
        // <div>
        //     <p class="two three">
        // <p class="three">
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let mut res = doc.insert_attribute("class", "one two", p_id, Location::default());
        assert!(res.is_ok());
        let p_id_2 = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());
        res = doc.insert_attribute("class", "one", p_id_2, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("id", "myid", p_id_2, Location::default());
        assert!(res.is_ok());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let p_id_3 = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());
        res = doc.insert_attribute("class", "two three", p_id_3, Location::default());
        assert!(res.is_ok());

        let p_id_4 = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        res = doc.insert_attribute("class", "three", p_id_4, Location::default());
        assert!(res.is_ok());

        let query = Query::new().contains_class("two").find_all();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 2);
        assert_eq!(found_ids, [p_id, p_id_3]);
    }

    #[test]
    fn single_query_contains_attribute_find_first() {
        // <div>
        //     <div id="myid" style="somestyle">
        //         <p title="hey">
        //     <p>
        // <div style="otherstyle" id="otherid">
        //     <p>
        // <p title="yo" style="cat">
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let mut res = doc.insert_attribute("id", "myid", div_id_2, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("style", "somestyle", div_id_2, Location::default());
        assert!(res.is_ok());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        res = doc.insert_attribute("title", "key", p_id, Location::default());
        assert!(res.is_ok());
        let _ = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        res = doc.insert_attribute("style", "otherstyle", div_id_3, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("id", "otherid", div_id_3, Location::default());
        assert!(res.is_ok());
        let _ = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let p_id_4 = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        res = doc.insert_attribute("title", "yo", p_id_4, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("style", "cat", p_id_4, Location::default());
        assert!(res.is_ok());

        let query = Query::new().contains_attribute("style").find_first();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 1);
        assert_eq!(found_ids, [div_id_2]);
    }

    #[test]
    fn single_query_contains_attribute_find_all() {
        // <div>
        //     <div id="myid" style="somestyle">
        //         <p title="hey">
        //     <p>
        // <div style="otherstyle" id="otherid">
        //     <p>
        // <p title="yo" style="cat">
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let mut res = doc.insert_attribute("id", "myid", div_id_2, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("style", "somestyle", div_id_2, Location::default());
        assert!(res.is_ok());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        res = doc.insert_attribute("title", "key", p_id, Location::default());
        assert!(res.is_ok());
        let _ = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        res = doc.insert_attribute("style", "otherstyle", div_id_3, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("id", "otherid", div_id_3, Location::default());
        assert!(res.is_ok());
        let _ = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let p_id_4 = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        res = doc.insert_attribute("title", "yo", p_id_4, Location::default());
        assert!(res.is_ok());
        res = doc.insert_attribute("style", "cat", p_id_4, Location::default());
        assert!(res.is_ok());

        let query = Query::new().contains_attribute("style").find_all();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 3);
        assert_eq!(found_ids, [div_id_2, div_id_3, p_id_4]);
    }

    #[test]
    fn single_query_contains_child_find_first() {
        // <div>
        //     <div>
        //         <p>
        //     <p>
        // <div>
        //     <p>
        // <p>
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let _ = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let _ = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let query = Query::new().contains_child_tag("p").find_first();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 1);
        assert_eq!(found_ids, [NodeId::root()]);
    }

    #[test]
    fn single_query_contains_child_find_all() {
        // <div>
        //     <div>
        //         <p>
        //     <p>
        // <div>
        //     <p>
        // <p>
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let _ = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let _ = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let query = Query::new().contains_child_tag("p").find_all();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 4);
        assert_eq!(found_ids, [NodeId::root(), div_id, div_id_2, div_id_3]);
    }

    #[test]
    fn single_query_has_parent_find_first() {
        // <div>
        //     <div>
        //         <p>
        //     <p>
        // <div>
        //     <p>
        // <p>
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let _ = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let _ = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let _ = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let query = Query::new().has_parent_tag("div").find_first();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 1);
        assert_eq!(found_ids, [div_id_2]);
    }

    #[test]
    fn single_query_has_parent_find_all() {
        // <div>
        //     <div>
        //         <p>
        //     <p>
        // <div>
        //     <p>
        // <p>
        let mut doc = DocumentBuilder::new_document(None);

        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let p_id_2 = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());

        let div_id_3 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let p_id_3 = doc.create_element("p", div_id_3, None, HTML_NAMESPACE, Location::default());

        let _ = doc.create_element(
            "p",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let query = Query::new().has_parent_tag("div").find_all();
        let found_ids = doc.query(&query).unwrap();
        assert_eq!(found_ids.len(), 4);
        assert_eq!(found_ids, [div_id_2, p_id, p_id_2, p_id_3]);
    }

    #[test]
    fn tree_iterator() {
        let mut doc = DocumentBuilder::new_document(None);

        // <div>
        //     <div>
        //         <p>first p tag
        //         <p>second p tag
        //     <p>third p tag
        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        let div_id_2 = doc.create_element("div", div_id, None, HTML_NAMESPACE, Location::default());
        let p_id = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let text_id = doc.create_text("first p tag", p_id, Location::default());
        let p_id_2 = doc.create_element("p", div_id_2, None, HTML_NAMESPACE, Location::default());
        let text_id_2 = doc.create_text("second p tag", p_id_2, Location::default());
        let p_id_3 = doc.create_element("p", div_id, None, HTML_NAMESPACE, Location::default());
        let text_id_3 = doc.create_text("third p tag", p_id_3, Location::default());

        let tree_iterator = TreeIterator::new(&doc);

        let expected_order = vec![
            NodeId::root(),
            div_id,
            div_id_2,
            p_id,
            text_id,
            p_id_2,
            text_id_2,
            p_id_3,
            text_id_3,
        ];

        let mut traversed_nodes = Vec::new();
        for current_node_id in tree_iterator {
            traversed_nodes.push(current_node_id);
        }

        assert_eq!(expected_order, traversed_nodes);
    }

    #[test]
    fn tree_iterator_mutation() {
        let mut doc = DocumentBuilder::new_document(None);
        let div_id = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );

        let mut tree_iterator = TreeIterator::new(&doc);
        let mut current_node_id;

        current_node_id = tree_iterator.next();
        assert_eq!(current_node_id.unwrap(), NodeId::root());

        // we mutate the tree while the iterator is still "open"
        let div_id_2 = doc.create_element(
            "div",
            NodeId::root(),
            None,
            HTML_NAMESPACE,
            Location::default(),
        );
        current_node_id = tree_iterator.next();
        assert_eq!(current_node_id.unwrap(), div_id);

        // and find this node on next iteration
        current_node_id = tree_iterator.next();
        assert_eq!(current_node_id.unwrap(), div_id_2);
    }
}

use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::CssSystem;
use gosub_shared::traits::node::Node;
use std::collections::HashMap;
use std::marker::PhantomData;

/// The node arena is the single source for nodes in a document (or fragment).
#[derive(Debug, Clone)]
pub struct NodeArena<N: Node<C>, C: CssSystem> {
    /// Current nodes stored as <id, node>
    nodes: HashMap<NodeId, N>,
    /// Next node ID to use
    next_id: NodeId,

    _marker: PhantomData<C>,
}

impl<C: CssSystem, N: Node<C>> NodeArena<N, C> {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl<C: CssSystem, N: Node<C>> PartialEq for NodeArena<N, C> {
    fn eq(&self, other: &Self) -> bool {
        if self.next_id != other.next_id {
            return false;
        }

        self.nodes == other.nodes
    }
}

impl<N: Node<C>, C: CssSystem> NodeArena<N, C> {
    /// Creates a new NodeArena
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            next_id: NodeId::default(),
            _marker: PhantomData,
        }
    }

    /// Peek what the next node ID is without incrementing the internal counter.
    /// Used by DocumentTaskQueue for create_element() tasks.
    pub(crate) fn peek_next_id(&self) -> NodeId {
        self.next_id
    }

    /// Gets the node with the given id
    pub fn node(&self, node_id: NodeId) -> Option<&N> {
        self.nodes.get(&node_id)
    }

    /// Get the node with the given id as a mutable reference
    pub fn node_mut(&mut self, node_id: NodeId) -> Option<&mut N> {
        self.nodes.get_mut(&node_id)
    }

    pub fn delete_node(&mut self, node_id: NodeId) {
        self.nodes.remove(&node_id);
    }

    /// Registered an unregistered node into the arena
    pub fn register_node(&mut self, mut node: N) -> NodeId {
        let id = self.next_id;
        self.next_id = id.next();

        node.set_id(id);

        self.nodes.insert(id, node);
        id
    }

    pub fn nodes(&self) -> &HashMap<NodeId, N> {
        &self.nodes
    }
}

impl<N: Node<C>, C: CssSystem> Default for NodeArena<N, C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::HTML_NAMESPACE;
    use gosub_shared::byte_stream::Location;
    use gosub_shared::traits::document::Document;
    use gosub_shared::traits::document::DocumentType;
    use crate::node::nodeimpl::NodeImpl;
    use gosub_shared::document::DocumentHandle;

    #[test]
    fn register_node() {
        let doc = DocumentHandle::create(Document::new(DocumentType::HTML, None, None));
        let node = NodeImpl::new_element(doc.clone(), Location::default(), "test", Some(HTML_NAMESPACE), HashMap::new());
        let mut document = doc.get_mut();
        let id = document.arena.register_node(node);

        assert_eq!(document.arena.nodes.len(), 1);
        assert_eq!(document.arena.next_id, 1usize.into());
        assert_eq!(id, NodeId::default());
    }

    #[test]
    #[should_panic]
    fn register_node_twice() {
        let doc = DocumentHandle::create(Document::new(DocumentType::HTML, None, None));
        let node = NodeImpl::new_element(doc.clone(), Location::default(), "test", Some(HTML_NAMESPACE), HashMap::new());

        let mut document = doc.get_mut();
        document.arena.register_node(node);

        let node = document.node_by_id(NodeId(0)).unwrap().to_owned();
        document.arena.register_node(node);
    }

    #[test]
    fn get_node() {
        let doc = DocumentHandle::create(Document::new(DocumentType::HTML, None, None));
        let node = NodeImpl::new_element(doc.clone(), Location::default(), "test", Some(HTML_NAMESPACE), HashMap::new());

        let mut document = doc.get_mut();
        let id = document.arena.register_node(node);
        let node = document.arena.get_node(id);
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "test");
    }

    #[test]
    fn get_node_mut() {
        let doc = DocumentHandle::create(Document::new(DocumentType::HTML, None, None));
        let node = NodeImpl::new_element(doc.clone(), Location::default(), "test", Some(HTML_NAMESPACE), HashMap::new());

        let mut document = doc.get_mut();
        let node_id = document.arena.register_node(node);
        let node = document.arena.get_node_mut(node_id);
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "test");
    }

    #[test]
    fn register_node_through_document() {
        let doc = DocumentHandle::create(Document::new(DocumentType::HTML, None, None));

        let parent = NodeImpl::new_element(doc.clone(), Location::default(), "parent", Some(HTML_NAMESPACE), HashMap::new());
        let child = NodeImpl::new_element(doc.clone(), Location::default(), "child", Some(HTML_NAMESPACE), HashMap::new());

        let mut document = doc.get_mut();
        let parent_id = document.arena.register_node(parent);
        let child_id = document.add_node(child, parent_id, None);

        let parent = document.node_by_id(parent_id).unwrap();
        assert!(parent.is_some());
        assert_eq!(parent.unwrap().children.len(), 1);
        assert_eq!(parent.unwrap().children[0], child_id);

        let child = document.node_by_id(child_id);
        assert!(child.is_some());
        assert_eq!(child.unwrap().parent, Some(parent_id));
    }
}

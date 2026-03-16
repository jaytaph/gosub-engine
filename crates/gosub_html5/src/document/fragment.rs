use core::fmt;
use core::fmt::Debug;

use crate::node::arena::NodeArena;
use gosub_interface::config::{HasDocumentFragment, HasDocument};
use gosub_interface::document::DocumentFragment;
use gosub_interface::node::NodeId;

/// Defines a document fragment which can be attached to for instance a <template> element
pub struct DocumentFragmentImpl<C: HasDocumentFragment> {
    /// Node elements inside this fragment
    arena: NodeArena<C>,
    /// Document handle of the parent
    /// Host node on which this fragment is attached
    host: NodeId,
}

impl<C: HasDocumentFragment> PartialEq for DocumentFragmentImpl<C> {
    fn eq(&self, other: &Self) -> bool {
        self.host == other.host && self.arena == other.arena
    }
}

impl<C: HasDocumentFragment> Clone for DocumentFragmentImpl<C> {
    /// Clones the document fragment
    fn clone(&self) -> Self {
        Self {
            arena: self.arena.clone(),
            host: self.host,
        }
    }
}

impl<C: HasDocumentFragment> Debug for DocumentFragmentImpl<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DocumentFragment")
    }
}

impl<C: HasDocumentFragment> DocumentFragmentImpl<C> {
    /// Creates a new document fragment and attaches it to "host" node inside "handle"
    #[must_use]
    pub(crate) fn new(host: NodeId) -> Self {
        Self {
            arena: NodeArena::new(),
            host,
        }
    }
}

impl<C: HasDocument> DocumentFragment<C> for DocumentFragmentImpl<C> {
    fn new(node_id: NodeId) -> Self {
        Self::new(node_id)
    }
}

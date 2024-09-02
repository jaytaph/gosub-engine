use crate::DocumentHandle;
use core::fmt;
use core::fmt::Debug;

use gosub_shared::node::NodeId;
use gosub_shared::traits::document::DocumentFragment;
use crate::document::document::DocumentImpl;
use crate::node::arena::NodeArena;
use crate::node::node::NodeImpl;

/// Defines a document fragment which can be attached to for instance a <template> element
#[derive(PartialEq)]
pub struct DocumentFragmentImpl {
    /// Node elements inside this fragment
    arena: NodeArena<NodeImpl>,
    /// Document handle of the parent
    pub handle: DocumentHandle<DocumentImpl>,
    /// Host node on which this fragment is attached
    host: NodeId,
}

impl Clone for DocumentFragmentImpl {
    /// Clones the document fragment
    fn clone(&self) -> Self {
        Self {
            arena: self.arena.clone(),
            handle: self.handle.clone(),
            host: self.host,
        }
    }
}

impl Debug for DocumentFragmentImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DocumentFragment")
    }
}

impl DocumentFragmentImpl {
    /// Creates a new document fragment and attaches it to "host" node inside "handle"
    #[must_use]
    pub(crate) fn new(handle: DocumentHandle<DocumentImpl>, host: NodeId) -> Self {
        Self {
            arena: NodeArena::new(),
            handle,
            host,
        }
    }
}

impl DocumentFragment for DocumentFragmentImpl {
    type Document = DocumentImpl;

    /// Returns the document handle for this document
    fn handle(&self) -> DocumentHandle<Self::Document> {
        self.handle.clone()
    }
}
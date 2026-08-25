//! The `ValidityState` object.
//!
//! Each getter re-reads the document, so the object stays live the way the real one does:
//! holding on to `input.validity` and then changing the value reports the new state.

use gosub_engine::validity;
use gosub_shared::node::NodeId;
use rquickjs::class::Trace;
use rquickjs::JsLifetime;

use crate::{DocHandle, DomConfig};

#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename = "ValidityState")]
pub struct DomValidity {
    #[qjs(skip_trace)]
    doc: DocHandle,
    #[qjs(skip_trace)]
    id: NodeId,
}

impl DomValidity {
    pub(crate) fn new(doc: DocHandle, id: NodeId) -> Self {
        Self { doc, id }
    }

    fn flags(&self) -> validity::Validity {
        validity::validity::<DomConfig>(&self.doc.borrow(), self.id)
    }
}

#[rquickjs::methods(rename_all = "camelCase")]
impl DomValidity {
    #[qjs(get)]
    pub fn value_missing(&self) -> bool {
        self.flags().value_missing
    }

    #[qjs(get)]
    pub fn type_mismatch(&self) -> bool {
        self.flags().type_mismatch
    }

    #[qjs(get)]
    pub fn pattern_mismatch(&self) -> bool {
        self.flags().pattern_mismatch
    }

    #[qjs(get)]
    pub fn too_long(&self) -> bool {
        self.flags().too_long
    }

    #[qjs(get)]
    pub fn too_short(&self) -> bool {
        self.flags().too_short
    }

    #[qjs(get)]
    pub fn range_underflow(&self) -> bool {
        self.flags().range_underflow
    }

    #[qjs(get)]
    pub fn range_overflow(&self) -> bool {
        self.flags().range_overflow
    }

    #[qjs(get)]
    pub fn step_mismatch(&self) -> bool {
        self.flags().step_mismatch
    }

    #[qjs(get)]
    pub fn bad_input(&self) -> bool {
        self.flags().bad_input
    }

    #[qjs(get)]
    pub fn custom_error(&self) -> bool {
        self.flags().custom_error
    }

    #[qjs(get)]
    pub fn valid(&self) -> bool {
        self.flags().valid()
    }
}

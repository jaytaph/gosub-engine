//! The document surface the form-control value rules need.
//!
//! Those rules have to give the same answer on both sides of the render pipeline: the engine
//! holds the real document, the layouter and painter hold a trait object over it. This is the
//! narrow view both can offer, so the rules can be written once against it rather than
//! reimplemented on each side - which is exactly how `<input type=date value="not a date">`
//! ended up painting its raw attribute while `.value` reported the empty string.

use crate::document::ControlEditState;
use gosub_shared::node::NodeId;

pub trait ControlHost {
    fn tag_name(&self, id: NodeId) -> Option<String>;
    fn attribute(&self, id: NodeId, name: &str) -> Option<String>;
    fn children(&self, id: NodeId) -> Vec<NodeId>;
    fn text_value(&self, id: NodeId) -> Option<String>;
    fn control_edit_state(&self, id: NodeId) -> Option<ControlEditState>;
}

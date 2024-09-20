use gosub_shared::errors::{CssResult};
use crate::node::Node;
use crate::Css3;

impl Css3<'_> {
    pub fn parse_at_rule_starting_style_block(&mut self) -> CssResult<Node> {
        log::trace!("parse_at_rule_starting_style_block");
        todo!();
    }
}

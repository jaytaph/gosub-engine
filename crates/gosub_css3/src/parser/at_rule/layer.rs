use crate::node::{Node, NodeType};
use crate::Css3;
use gosub_shared::types::Result;

impl Css3<'_> {
    #[allow(dead_code)]
    fn parse_at_rule_layer_list(&mut self) -> Result<Node> {
        let _children: Vec<Node> = Vec::new();

        todo!();
    }

    fn parse_layer_query(&mut self) -> Result<Node> {
        let _children: Vec<Node> = Vec::new();
        todo!();
    }

    pub fn parse_at_rule_layer_prelude(&mut self) -> Result<Node> {
        log::trace!("parse_at_rule_layer_prelude");

        let loc = self.tokenizer.current_location();

        self.consume_whitespace_comments();

        let mut layers = vec![];

        while !self.tokenizer.eof() {
            let layer = self.parse_layer_query()?;
            layers.push(layer);

            self.consume_whitespace_comments();

            let t = self.consume_any()?;
            if !t.is_comma() {
                self.tokenizer.reconsume();
                break;
            }
        }

        Ok(Node::new(NodeType::LayerList { layers }, loc))
    }
}

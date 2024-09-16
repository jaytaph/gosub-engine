use crate::node::{Node, NodeType};
use crate::tokenizer::TokenType;
use crate::Css3;
use gosub_shared::types::Result;
use crate::errors::Error;

impl Css3<'_> {
    pub fn parse_combinator(&mut self) -> Result<Node> {
        log::trace!("parse_combinator");
        let t = self.consume_any()?;

        let name = match t.token_type {
            TokenType::Whitespace(_) => " ".to_string(),
            TokenType::Delim('+') => t.to_string(),
            TokenType::Delim('>') => t.to_string(),
            TokenType::Delim('~') => t.to_string(),
            TokenType::Delim('/') => {
                let tn1 = self.tokenizer.lookahead(1);
                let tn2 = self.tokenizer.lookahead(2);
                if tn1.token_type == TokenType::Ident("deep".to_string())
                    && tn2.token_type == TokenType::Delim('/')
                {
                    "/deep/".to_string()
                } else {
                    return Err(Error::Parse(
                        format!("Unexpected token {:?}", tn1),
                        self.tokenizer.current_location(),
                    ).into());
                }
            }
            _ => {
                return Err(Error::Parse(
                    format!("Unexpected token {:?}", t),
                    self.tokenizer.current_location(),
                ).into());
            }
        };

        self.consume_whitespace_comments();

        Ok(Node::new(NodeType::Combinator { value: name }, t.location))
    }
}

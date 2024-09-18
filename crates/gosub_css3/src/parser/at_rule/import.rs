use crate::node::{Node, NodeType};
use crate::tokenizer::TokenType;
use crate::{Css3, Error};
use gosub_shared::types::Result;

impl Css3<'_> {
    pub fn parse_at_rule_import_prelude(&mut self) -> Result<Node> {
        log::trace!("parse_at_rule_import");

        let mut children = Vec::new();

        let loc = self.tokenizer.current_location();

        let t = self.consume_any()?;
        match t.token_type {
            TokenType::QuotedString(value) => {
                children.push(Node::new(NodeType::String { value }, loc));
            }
            TokenType::Url(url) => {
                children.push(Node::new(NodeType::Url { url }, loc));
            }
            TokenType::Function(name) if name.eq_ignore_ascii_case("url") => {
                self.tokenizer.reconsume();
                children.push(self.parse_url()?);
            }
            _ => {
                return Err(Error::Parse(
                    "Expected string or url()".to_string().to_string(),
                    t.location,
                )
                .into());
            }
        }

        self.consume_whitespace_comments();

        let t = self.tokenizer.lookahead_sc(0);
        match t.token_type {
            TokenType::Ident(value) if value.eq_ignore_ascii_case("layer") => {
                children.push(Node::new(NodeType::Ident { value }, t.location));
            }
            TokenType::Function(name) if name.eq_ignore_ascii_case("layer") => {
                children.push(self.parse_function()?);
            }
            _ => {}
        }

        self.consume_whitespace_comments();

        let t = self.tokenizer.lookahead_sc(0);
        match t.token_type {
            TokenType::Function(name) if name.eq_ignore_ascii_case("supports") => {
                children.push(self.parse_function()?);
            }
            _ => {}
        }

        self.consume_whitespace_comments();
        // let nt = self.tokenizer.lookahead_sc(0);
        // match nt.token_type {
        //     TokenType::Ident(_) => {
        //         self.tokenizer.reconsume();
        //         let list = self.parse_media_query_list()?;
        //         children.push(list);
        //     }
        //     TokenType::LParen => {
        //         self.tokenizer.reconsume();
        //         let list = self.parse_media_query_list()?;
        //         children.push(list);
        //     }
        //     _ => {}
        // }

        Ok(Node::new(NodeType::ImportList { children }, loc))
    }
}

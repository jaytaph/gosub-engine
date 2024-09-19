use crate::render_tree::{RenderNodeData, RenderTree};
use gosub_render_backend::layout::{Layout, Layouter};
use gosub_render_backend::{NodeDesc, Point, Size};
use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::{CssPropertyMap, CssSystem};
use gosub_shared::traits::document::Document;

impl<L: Layouter, D: Document<C>, C: CssSystem> RenderTree<L, D, C> {
    pub fn desc(&self) -> NodeDesc {
        self.desc_node(self.root)
    }

    fn desc_node(&self, node: NodeId) -> NodeDesc {
        let Some(node) = self.get_node(node) else {
            return NodeDesc {
                id: 0,
                name: "<unknown>".into(),
                children: vec![],
                attributes: vec![],
                properties: vec![],
                text: None,
                pos: Point::ZERO,
                size: Size::ZERO,
            };
        };

        let attributes = if let RenderNodeData::Element = &node.data {
            // we need to get the attributes from the document, not from the render tree

            todo!("attributes")

            // e.attributes
            //     .iter()
            //     .map(|(k, v)| (k.clone(), v.clone()))
            //     .collect()
        } else {
            vec![]
        };

        let (name, text) = if let RenderNodeData::Text(t) = &node.data {
            ("#text".into(), Some(t.text.clone()))
        } else {
            (node.name.clone(), None)
        };

        NodeDesc {
            id: node.id.into(),
            name,
            children: node
                .children
                .iter()
                .map(|child| self.desc_node(*child))
                .collect(),
            attributes,
            properties: node
                .properties
                .iter()
                .map(|(k, v)| (k.to_owned(), format!("{v:?}")))
                .collect(),
            text,
            pos: node.layout.rel_pos(),
            size: node.layout.size(),
        }
    }
}

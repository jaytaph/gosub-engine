use crate::stylesheet::CssValue;
use gosub_shared::traits::css3::CssSystem;
use gosub_shared::traits::node::{ElementDataType, Node};

// Probably this shouldn't quite be in gosub_css3
#[allow(dead_code)]
pub fn resolve_attr<N: Node<C>, C: CssSystem>(values: &[CssValue], node: &N) -> Vec<CssValue> {
    let Some(attr_name) = values.first().map(|v| v.to_string()) else {
        return vec![];
    };

    let ty = values.get(1).cloned();

    let Some(data) = node.get_element_data() else {
        return vec![];
    };

    let Some(attr_value) = data.attribute(&attr_name) else {
        let _default_value = values.get(2).cloned();

        if let Some(ty) = ty {
            return vec![ty];
        }

        return vec![];
    };

    let Ok(value) = CssValue::parse_str(attr_value) else {
        return vec![];
    };

    vec![value]
}

use crate::matcher::styling::{match_selector, CssProperties, CssProperty, DeclarationProperty};
use crate::Css3;
use gosub_shared::document::DocumentHandle;
use gosub_shared::errors::CssResult;
use gosub_shared::node::NodeId;
use gosub_shared::traits::css3::{CssOrigin, CssSystem};
use gosub_shared::traits::document::Document;
use gosub_shared::traits::node::{ElementDataType, Node, TextDataType};
use gosub_shared::traits::ParserConfig;
use log::warn;

use crate::functions::attr::resolve_attr;
use crate::functions::calc::resolve_calc;
use crate::functions::var::resolve_var;
use crate::matcher::property_definitions::get_css_definitions;
use crate::matcher::shorthands::FixList;
use crate::stylesheet::{CssDeclaration, CssValue, Specificity};

#[derive(Debug, Clone)]
pub struct Css3System;

impl CssSystem for Css3System {
    type Stylesheet = crate::stylesheet::CssStylesheet;

    type PropertyMap = CssProperties;

    type Property = CssProperty;

    fn parse_str(str: &str, config: ParserConfig, origin: CssOrigin, url: &str) -> CssResult<Self::Stylesheet> {
        Css3::parse_str(str, config, origin, url)
    }

    fn properties_from_node<D: Document<Self>>(
        node: &D::Node,
        sheets: &[Self::Stylesheet],
        handle: DocumentHandle<D, Self>,
        id: NodeId,
    ) -> Option<Self::PropertyMap> {
        let mut css_map_entry = CssProperties::new();

        if node_is_unrenderable::<D, Self>(node) {
            return None;
        }

        let definitions = get_css_definitions();

        let mut fix_list = FixList::new();

        for sheet in sheets {
            for rule in &sheet.rules {
                for selector in rule.selectors().iter() {
                    let (matched, specificity) = match_selector(DocumentHandle::clone(&handle), id, selector);

                    if !matched {
                        continue;
                    }

                    // Selector matched, so we add all declared values to the map
                    for declaration in rule.declarations().iter() {
                        // Step 1: find the property in our CSS definition list
                        let Some(definition) = definitions.find_property(&declaration.property) else {
                            // If not found, we skip this declaration
                            warn!("Definition is not found for property {:?}", declaration.property);
                            continue;
                        };

                        let value = resolve_functions(&declaration.value, node, handle.clone());

                        // Check if the declaration matches the definition and return the "expanded" order
                        let res = definition.matches_and_shorthands(&value, &mut fix_list);
                        if !res {
                            warn!("Declaration does not match definition: {:?}", declaration);
                            continue;
                        }

                        // create property for the given values
                        let property_name = declaration.property.clone();
                        let decl = CssDeclaration {
                            property: property_name.to_string(),
                            value,
                            important: declaration.important,
                        };

                        add_property_to_map(&mut css_map_entry, sheet, specificity.clone(), &decl);
                    }
                }
            }
        }

        fix_list.resolve_nested(definitions);

        fix_list.apply(&mut css_map_entry);

        Some(css_map_entry)
    }
}

/* TODO
fn resolve_inheritance(&mut self, node_id: NodeId, inherit_props: &Vec<(String, CssValue)>) {
    let Some(current_node) = self.get_node(node_id) else {
        return;
    };

    for prop in inherit_props {
        current_node
            .properties
            .entry(prop.0.clone())
            .or_insert_with(|| {
                let mut p = CssProperty::new(prop.0.as_str());

                p.inherited = prop.1.clone();

                p
            });
    }

    let mut inherit_props = inherit_props.clone();

    'props: for (name, prop) in &mut current_node.properties.iter_mut() {
        prop.compute_value();

        let value = prop.actual.clone();

        if prop_is_inherit(name) {
            for (k, v) in &mut inherit_props {
                if k == name {
                    *v = value;
                    continue 'props;
                }
            }

            inherit_props.push((name.clone(), value));
        }
    }

    let Some(children) = self.get_children(node_id) else {
        return;
    };

    for child in children.clone() {
        self.resolve_inheritance(child, &inherit_props);
    }
}



 */
pub fn add_property_to_map(
    css_map_entry: &mut CssProperties,
    sheet: &crate::stylesheet::CssStylesheet,
    specificity: Specificity,
    declaration: &CssDeclaration,
) {
    let property_name = declaration.property.clone();
    // let entry = CssProperty::new(property_name.as_str());

    // If the property is a shorthand css property, we need fetch the individual properties
    // It's possible that need to recurse here as these individual properties can be shorthand as well
    // if entry.is_shorthand() {
    //     for property_name in entry.get_props_from_shorthand() {
    //         let decl = CssDeclaration {
    //             property: property_name.to_string(),
    //             value: declaration.value.clone(),
    //             important: declaration.important,
    //         };
    //
    //         add_property_to_map(css_map_entry, sheet, selector, &decl);
    //     }
    // }
    //
    let declaration = DeclarationProperty {
        // @todo: this seems wrong. We only get the first values from the declared values
        value: declaration.value.first().unwrap().clone(),
        origin: sheet.origin,
        important: declaration.important,
        location: sheet.url.clone(),
        specificity,
    };

    if let std::collections::hash_map::Entry::Vacant(e) = css_map_entry.properties.entry(property_name.clone()) {
        // Generate new property in the css map
        let mut entry = CssProperty::new(property_name.as_str());
        entry.declared.push(declaration);
        e.insert(entry);
    } else {
        // Just add the declaration to the existing property
        let entry = css_map_entry.properties.get_mut(&property_name).unwrap();
        entry.declared.push(declaration);
    }
}

pub fn node_is_unrenderable<D: Document<C>, C: CssSystem>(node: &D::Node) -> bool {
    // There are more elements that are not renderable, but for now we only remove the most common ones

    const REMOVABLE_ELEMENTS: [&str; 6] = ["head", "script", "style", "svg", "noscript", "title"];

    if let Some(element_data) = node.get_element_data() {
        if REMOVABLE_ELEMENTS.contains(&element_data.name()) {
            return true;
        }
    }

    if let Some(text_data) = &node.get_text_data() {
        if text_data.value().chars().all(|c| c.is_whitespace()) {
            return true;
        }
    }

    false
}

pub fn resolve_functions<D: Document<C>, C: CssSystem>(
    value: &[CssValue],
    node: &D::Node,
    handle: DocumentHandle<D, C>,
) -> Vec<CssValue> {
    let mut result = Vec::with_capacity(value.len()); //TODO: we could give it a &mut Vec and reuse the allocation

    for val in value {
        match val {
            CssValue::Function(func, values) => {
                let resolved = match func.as_str() {
                    "calc" => resolve_calc(values),
                    "attr" => resolve_attr(values, node),
                    "var" => resolve_var(values, &*handle.get(), node),
                    _ => vec![val.clone()],
                };

                result.extend(resolved);
            }
            _ => result.push(val.clone()),
        }
    }

    result
}

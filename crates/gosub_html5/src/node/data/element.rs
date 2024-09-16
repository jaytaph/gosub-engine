use core::fmt::{Debug, Formatter};
use std::collections::HashMap;
use std::fmt;
use gosub_shared::traits::css3::CssSystem;
use gosub_shared::traits::node::ElementDataType;
use crate::node::elements::{FORMATTING_HTML_ELEMENTS, SPECIAL_HTML_ELEMENTS, SPECIAL_MATHML_ELEMENTS, SPECIAL_SVG_ELEMENTS};
use crate::node::{HTML_NAMESPACE, MATHML_NAMESPACE, SVG_NAMESPACE};
use crate::document::document::DocumentImpl;
use crate::document::fragment::DocumentFragmentImpl;

#[derive(Debug, Clone, PartialEq)]
pub struct ElementClass {
    /// a map of classes applied to an HTML element.
    /// key = name, value = is_active
    /// the is_active is used to toggle a class (JavaScript API)
    class_map: HashMap<String, bool>,
}

impl Default for ElementClass {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementClass {
    /// Initialise a new (empty) ElementClass
    #[must_use]
    pub fn new() -> Self {
        Self {
            class_map: HashMap::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &bool)> {
        self.class_map.iter()
    }

    /// Count the number of classes (active or inactive)
    /// assigned to an element
    pub fn len(&self) -> usize {
        self.class_map.len()
    }

    /// Check if any classes are present
    pub fn is_empty(&self) -> bool {
        self.class_map.is_empty()
    }

    /// Check if class name exists
    pub fn contains(&self, name: &str) -> bool {
        self.class_map.contains_key(name)
    }

    /// Add a new class (if already exists, does nothing)
    pub fn add(&mut self, name: &str) {
        // by default, adding a new class will be active.
        // however, map.insert will update a key if it exists,
        // and we don't want to overwrite an inactive class to make it active unintentionally,
        // so we ignore this operation if the class already exists
        if !self.contains(name) {
            self.class_map.insert(name.to_owned(), true);
        }
    }

    /// Remove a class (does nothing if not exists)
    pub fn remove(&mut self, name: &str) {
        self.class_map.remove(name);
    }

    /// Toggle a class active/inactive. Does nothing if class doesn't exist
    pub fn toggle(&mut self, name: &str) {
        if let Some(is_active) = self.class_map.get_mut(name) {
            *is_active = !*is_active;
        }
    }

    /// Set explicitly if a class is active or not. Does nothing if class doesn't exist
    pub fn set_active(&mut self, name: &str, is_active: bool) {
        if let Some(is_active_item) = self.class_map.get_mut(name) {
            *is_active_item = is_active;
        }
    }

    /// Check if a class is active. Returns false if class doesn't exist
    pub fn is_active(&self, name: &str) -> bool {
        if let Some(is_active) = self.class_map.get(name) {
            return *is_active;
        }

        false
    }
}

/// Initialize a class from a class string
/// with space-delimited class names
impl From<&str> for ElementClass {
    fn from(class_string: &str) -> Self {
        let class_map_local = class_string
            .split_whitespace()
            .map(|class| (class.to_owned(), true))
            .collect::<HashMap<String, bool>>();

        ElementClass {
            class_map: class_map_local,
        }
    }
}


/// Data structure for element nodes
#[derive(PartialEq, Clone)]
pub struct ElementData<C: CssSystem> {
    // /// Numerical ID of the node this data is attached to
    // pub node_id: NodeId,
    /// Name of the element (e.g., div)
    pub name: String,
    /// Namespace of the element
    pub namespace: Option<String>,
    /// Element's attributes stored as key-value pairs.
    /// Note that it is NOT RECOMMENDED to modify this
    /// attribute map directly and instead use TreeBuilder.insert_attribute
    /// to keep attributes in sync with the DOM.
    pub attributes: HashMap<String, String>,
    /// CSS classes
    pub classes: ElementClass,
    // Only used for <script> elements
    pub force_async: bool,
    // Template contents (when it's a template element)
    pub template_contents: Option<DocumentFragmentImpl<C>>,

}

impl<C: CssSystem> Debug for ElementData<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ElementData");
        debug.field("name", &self.name);
        debug.field("attributes", &self.attributes);
        debug.field("classes", &self.classes);
        debug.finish()
    }
}

impl<C: CssSystem> ElementDataType<C> for ElementData<C> {
    type Document = DocumentImpl<C>;
    type DocumentFragment = DocumentFragmentImpl<C>;

    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn namespace(&self) -> &str {
        match self.namespace {
            Some(ref namespace) => namespace.as_str(),
            None => HTML_NAMESPACE,
        }
    }

    fn is_namespace(&self, namespace: &str) -> bool {
        self.name == namespace
    }

    fn classes(&self) -> Vec<String> {
        self.classes.iter().map(|(name, _)| name.clone()).collect()
    }

    fn active_classes(&self) -> Vec<String> {
        self.classes.iter().filter(|(_, &active)| active).map(|(name, _)| name.clone()).collect()
    }

    fn attribute(&self, name: &str) -> Option<&String> {
        self.attributes.get(name)
    }

    fn attributes(&self) -> &HashMap<String, String> {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.attributes
    }

    /// This will only compare against the tag, namespace and data same except element data.
    /// for element data compare against the tag, namespace and attributes without order.
    /// Both nodes could still have other parents and children.
    fn matches_tag_and_attrs_without_order(&self, other_data: &ElementData<C>) -> bool {
        if self.name != other_data.name || self.namespace != other_data.namespace {
            return false;
        }

        if self.name != other_data.name {
            return false;
        }

        if self.namespace != other_data.namespace {
            return false;
        }

        return self.attributes.eq(&other_data.attributes);
    }

    /// Returns true if the given node is a mathml integration point
    /// See: https://html.spec.whatwg.org/multipage/parsing.html#mathml-text-integration-point
    fn is_mathml_integration_point(&self) -> bool {
        let namespace = self.namespace.clone().unwrap_or_default();

        namespace == MATHML_NAMESPACE
            && ["mi", "mo", "mn", "ms", "mtext"].contains(&self.name.as_str())
    }

    // fn set_attributes(&mut self, attributes: &HashMap<String, String>) {
    //     self.attributes = attributes.to_owned();
    // }

    /// Returns true if the given node is a html integration point
    /// See: https://html.spec.whatwg.org/multipage/parsing.html#html-integration-point
    fn is_html_integration_point(&self) -> bool {
        match self.namespace {
            Some(ref namespace) => {
                if namespace == MATHML_NAMESPACE && self.name == "annotation-xml" {
                    if let Some(value) = self.attributes().get("encoding") {
                        if value.eq_ignore_ascii_case("text/html") {
                            return true;
                        }
                        if value.eq_ignore_ascii_case("application/xhtml+xml") {
                            return true;
                        }
                    }

                    return false;
                }

                namespace == SVG_NAMESPACE
                    && ["foreignObject", "desc", "title"].contains(&self.name.as_str())
            }
            None => return false,
        }
    }

    /// Returns true if the given node is "special" node based on the namespace and name
    fn is_special(&self) -> bool {
        if self.namespace == Some(HTML_NAMESPACE.into())
            && SPECIAL_HTML_ELEMENTS.contains(&self.name())
        {
            return true;
        }
        if self.namespace == Some(MATHML_NAMESPACE.into())
            && SPECIAL_MATHML_ELEMENTS.contains(&self.name())
        {
            return true;
        }
        if self.namespace == Some(SVG_NAMESPACE.into())
            && SPECIAL_SVG_ELEMENTS.contains(&self.name())
        {
            return true;
        }

        false
    }
    fn add_class(&mut self, class_name: &str) {
        self.classes.add(class_name);
    }

    fn template_contents(&self) -> Option<&Self::DocumentFragment> {
        match &self.template_contents {
            Some(fragment) => Some(fragment),
            None => None,
        }
    }

    /// Returns true if the given node is a "formatting" node
    fn is_formatting(&self) -> bool {
        self.namespace == Some(HTML_NAMESPACE.into())
            && FORMATTING_HTML_ELEMENTS.contains(&self.name.as_str())
    }

    fn set_template_contents(&mut self, template_contents: Self::DocumentFragment) {
        self.template_contents = Some(template_contents);
    }
}

impl<C: CssSystem> ElementData<C> {
    pub(crate) fn new(
        name: &str,
        namespace: Option<&str>,
        attributes: HashMap<String, String>,
        classes: ElementClass,
    ) -> Self {
        let (force_async, template_contents) = <_>::default();
        Self {
            name: name.into(),
            namespace: Some(namespace.unwrap_or(HTML_NAMESPACE).into()),
            attributes,
            classes,
            force_async,
            template_contents,
        }
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_empty() {
        let mut classes = ElementClass::new();
        assert!(classes.is_empty());
        classes.add("one");
        assert!(!classes.is_empty());
    }

    #[test]
    fn count_classes() {
        let mut classes = ElementClass::new();
        classes.add("one");
        classes.add("two");
        assert_eq!(classes.len(), 2);
    }

    #[test]
    fn contains_nonexistent_class() {
        let classes = ElementClass::new();
        assert!(!classes.contains("nope"));
    }

    #[test]
    fn contains_valid_class() {
        let mut classes = ElementClass::new();
        classes.add("yep");
        assert!(classes.contains("yep"));
    }

    #[test]
    fn add_class() {
        let mut classes = ElementClass::new();
        classes.add("yep");
        assert!(classes.is_active("yep"));

        classes.set_active("yep", false);
        classes.add("yep"); // should be ignored
        assert!(!classes.is_active("yep"));
    }

    #[test]
    fn remove_class() {
        let mut classes = ElementClass::new();
        classes.add("yep");
        classes.remove("yep");
        assert!(!classes.contains("yep"));
    }

    #[test]
    fn toggle_class() {
        let mut classes = ElementClass::new();
        classes.add("yep");
        assert!(classes.is_active("yep"));
        classes.toggle("yep");
        assert!(!classes.is_active("yep"));
        classes.toggle("yep");
        assert!(classes.is_active("yep"));
    }
}

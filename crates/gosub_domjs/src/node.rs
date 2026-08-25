//! The JS `Node` wrapper.
//!
//! One class covers every node type and dispatches the element-specific properties on tag
//! name. A real binding needs the interface hierarchy (`HTMLOptionElement` and friends) so
//! that `instanceof` and prototype-chain tests work; this is enough to drive the engine.

use cow_utils::CowUtils;
use gosub_interface::document::Document as _;
use gosub_interface::node::NodeType;
use gosub_shared::byte_stream::Location;
use gosub_shared::node::NodeId;
use rquickjs::class::Trace;
use rquickjs::{Coerced, Ctx, Exception, JsLifetime, Result, Value};

use crate::validity::DomValidity;
use crate::{event, select, text, wrap, wrap_opt, DocHandle, DomConfig};
use gosub_engine::{edit, form, validity};
use gosub_interface::document::ControlEditState;

#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename = "Node")]
pub struct GosubNode {
    #[qjs(skip_trace)]
    doc: DocHandle,
    #[qjs(skip_trace)]
    pub(crate) id: NodeId,
}

impl GosubNode {
    pub(crate) fn new(doc: DocHandle, id: NodeId) -> Self {
        Self { doc, id }
    }

    fn attr(&self, name: &str) -> String {
        self.doc
            .borrow()
            .attribute(self.id, name)
            .map(str::to_string)
            .unwrap_or_default()
    }

    fn set_attr(&self, name: &str, value: &str) {
        self.doc.borrow_mut().set_attribute(self.id, name, value);
    }

    fn bool_attr(&self, name: &str) -> bool {
        self.doc.borrow().attribute(self.id, name).is_some()
    }

    /// A boolean content attribute is set by presence, so clearing it removes it entirely.
    fn set_bool_attr(&self, name: &str, on: bool) {
        let mut doc = self.doc.borrow_mut();
        if on {
            doc.set_attribute(self.id, name, "");
        } else {
            doc.remove_attribute(self.id, name);
        }
    }

    fn long_attr(&self, name: &str, default: i32) -> i32 {
        self.doc
            .borrow()
            .attribute(self.id, name)
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(default)
    }

    /// The `<select>` an `<option>` belongs to, through an `<optgroup>` if there is one.
    fn owner_select(&self) -> Option<NodeId> {
        let doc = self.doc.borrow();
        if doc.tag_name(self.id) != Some("option") {
            return None;
        }
        let mut current = doc.parent(self.id)?;
        loop {
            match doc.tag_name(current) {
                Some("select") => return Some(current),
                Some("optgroup") => current = doc.parent(current)?,
                _ => return None,
            }
        }
    }

    /// Every `<option>` of a `<select>`, in tree order.
    fn option_ids(&self) -> Vec<NodeId> {
        let doc = self.doc.borrow();
        if doc.tag_name(self.id) != Some("select") {
            return Vec::new();
        }
        select::descendants(&doc, self.id)
            .into_iter()
            .filter(|&id| doc.tag_name(id) == Some("option"))
            .collect()
    }

    /// Setting `select.value` picks the first option with that value, or clears the choice.
    fn select_by_value(&self, value: &str) {
        let doc = self.doc.borrow();
        let chosen = self
            .option_ids()
            .into_iter()
            .find(|&option| form::option_value::<DomConfig>(&doc, option) == value);
        doc.set_selected_option(self.id, chosen);
    }

    /// The document has no namespace support for attributes, so a namespaced attribute is
    /// parked under a key no HTML attribute name can produce (they cannot contain spaces).
    /// That keeps `setAttributeNS` out of the reflection path, which is what the tests check.
    fn ns_key(namespace: &str, name: &str) -> String {
        format!("{namespace} {name}")
    }
}

#[rquickjs::methods(rename_all = "camelCase")]
impl GosubNode {
    // ── node ───────────────────────────────────────────────────────────────

    #[qjs(get)]
    pub fn node_type(&self) -> u32 {
        match self.doc.borrow().node_type(self.id) {
            NodeType::ElementNode => 1,
            NodeType::TextNode => 3,
            NodeType::CommentNode => 8,
            NodeType::DocumentNode => 9,
            NodeType::DocTypeNode => 10,
        }
    }

    #[qjs(get)]
    pub fn node_name(&self) -> String {
        let doc = self.doc.borrow();
        match doc.tag_name(self.id) {
            Some(tag) => tag.cow_to_ascii_uppercase().into_owned(),
            None => match doc.node_type(self.id) {
                NodeType::TextNode => "#text".to_string(),
                NodeType::CommentNode => "#comment".to_string(),
                _ => "#document".to_string(),
            },
        }
    }

    #[qjs(get)]
    pub fn tag_name(&self) -> Option<String> {
        self.doc
            .borrow()
            .tag_name(self.id)
            .map(|tag| tag.cow_to_ascii_uppercase().into_owned())
    }

    #[qjs(get)]
    pub fn local_name(&self) -> Option<String> {
        self.doc.borrow().tag_name(self.id).map(str::to_string)
    }

    // ── tree ───────────────────────────────────────────────────────────────

    #[qjs(get)]
    pub fn parent_node<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let parent = self.doc.borrow().parent(self.id);
        wrap_opt(&ctx, &self.doc, parent)
    }

    #[qjs(get)]
    pub fn parent_element<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let parent = self
            .doc
            .borrow()
            .parent(self.id)
            .filter(|&p| self.doc.borrow().tag_name(p).is_some());
        wrap_opt(&ctx, &self.doc, parent)
    }

    #[qjs(get)]
    pub fn child_nodes<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let children = self.doc.borrow().children(self.id).to_vec();
        wrap_list(&ctx, &self.doc, &children)
    }

    #[qjs(get)]
    pub fn children<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let doc = self.doc.borrow();
        let children: Vec<NodeId> = doc
            .children(self.id)
            .iter()
            .copied()
            .filter(|&c| doc.tag_name(c).is_some())
            .collect();
        drop(doc);
        wrap_list(&ctx, &self.doc, &children)
    }

    #[qjs(get)]
    pub fn first_child<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let first = self.doc.borrow().children(self.id).first().copied();
        wrap_opt(&ctx, &self.doc, first)
    }

    pub fn append_child<'js>(&self, ctx: Ctx<'js>, child: rquickjs::Class<'js, GosubNode>) -> Result<Value<'js>> {
        let child_id = child.borrow().id;
        {
            let mut doc = self.doc.borrow_mut();
            doc.detach(child_id);
            doc.attach(child_id, self.id, None);
        }
        if self.doc.borrow().parent(child_id) != Some(self.id) {
            // `attach_node` refuses to build a cycle instead of throwing HierarchyRequestError.
            return Err(Exception::throw_message(&ctx, "appendChild would create a cycle"));
        }
        wrap(&ctx, &self.doc, child_id)
    }

    pub fn remove_child<'js>(&self, ctx: Ctx<'js>, child: rquickjs::Class<'js, GosubNode>) -> Result<Value<'js>> {
        let child_id = child.borrow().id;
        if self.doc.borrow().parent(child_id) != Some(self.id) {
            return Err(Exception::throw_message(&ctx, "NotFoundError: node is not a child"));
        }
        self.doc.borrow_mut().detach(child_id);
        wrap(&ctx, &self.doc, child_id)
    }

    pub fn remove(&self) {
        self.doc.borrow_mut().detach(self.id);
    }

    /// A deep clone copies the subtree; a shallow one copies just this node.
    pub fn clone_node<'js>(&self, ctx: Ctx<'js>, deep: rquickjs::prelude::Opt<Coerced<bool>>) -> Result<Value<'js>> {
        let deep = deep.0.map(|d| d.0).unwrap_or(false);
        let copy = {
            let mut doc = self.doc.borrow_mut();
            if deep {
                doc.clone_node(self.id)
            } else {
                doc.duplicate_node(self.id)
            }
        };
        wrap(&ctx, &self.doc, copy)
    }

    pub fn matches(&self, ctx: Ctx<'_>, selector: String) -> Result<bool> {
        let compound = select::parse(&selector).map_err(|e| Exception::throw_message(&ctx, &e))?;
        Ok(select::matches(&self.doc.borrow(), self.id, &compound))
    }

    pub fn replace_children<'js>(&self, nodes: rquickjs::prelude::Rest<rquickjs::Class<'js, GosubNode>>) -> Result<()> {
        let existing = self.doc.borrow().children(self.id).to_vec();
        {
            let mut doc = self.doc.borrow_mut();
            for child in existing {
                doc.detach(child);
            }
        }
        for node in nodes.0 {
            let id = node.borrow().id;
            let mut doc = self.doc.borrow_mut();
            doc.detach(id);
            doc.attach(id, self.id, None);
        }
        Ok(())
    }

    pub fn insert_before<'js>(
        &self,
        ctx: Ctx<'js>,
        node: rquickjs::Class<'js, GosubNode>,
        reference: rquickjs::prelude::Opt<rquickjs::Class<'js, GosubNode>>,
    ) -> Result<Value<'js>> {
        let new_id = node.borrow().id;
        let position = reference.0.and_then(|r| {
            let target = r.borrow().id;
            self.doc.borrow().children(self.id).iter().position(|&c| c == target)
        });
        {
            let mut doc = self.doc.borrow_mut();
            doc.detach(new_id);
            doc.attach(new_id, self.id, position);
        }
        wrap(&ctx, &self.doc, new_id)
    }

    pub fn has_child_nodes(&self) -> bool {
        !self.doc.borrow().children(self.id).is_empty()
    }

    // ── attributes ─────────────────────────────────────────────────────────

    pub fn get_attribute(&self, name: String) -> Option<String> {
        self.doc
            .borrow()
            .attribute(self.id, &name.cow_to_ascii_lowercase())
            .map(str::to_string)
    }

    pub fn set_attribute(&self, name: String, value: String) {
        self.doc
            .borrow_mut()
            .set_attribute(self.id, &name.cow_to_ascii_lowercase(), &value);
    }

    pub fn remove_attribute(&self, name: String) {
        self.doc
            .borrow_mut()
            .remove_attribute(self.id, &name.cow_to_ascii_lowercase());
    }

    pub fn has_attribute(&self, name: String) -> bool {
        self.doc
            .borrow()
            .attribute(self.id, &name.cow_to_ascii_lowercase())
            .is_some()
    }

    #[qjs(rename = "setAttributeNS")]
    pub fn set_attribute_ns(&self, namespace: Option<String>, name: String, value: String) {
        match namespace {
            None => self.set_attribute(name, value),
            Some(ns) => self
                .doc
                .borrow_mut()
                .set_attribute(self.id, &Self::ns_key(&ns, &name), &value),
        }
    }

    #[qjs(rename = "getAttributeNS")]
    pub fn get_attribute_ns(&self, namespace: Option<String>, name: String) -> Option<String> {
        match namespace {
            None => self.get_attribute(name),
            Some(ns) => self
                .doc
                .borrow()
                .attribute(self.id, &Self::ns_key(&ns, &name))
                .map(str::to_string),
        }
    }

    // ── reflected content attributes ───────────────────────────────────────
    //
    // Every one of these is a getter/setter pair. A getter on its own is worse than no
    // binding at all: assigning to it is a silent no-op in a page script, so a test carries
    // on and fails somewhere far away from the line that actually did nothing.

    #[qjs(get)]
    pub fn id(&self) -> String {
        self.attr("id")
    }

    #[qjs(set, rename = "id")]
    pub fn set_id(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("id", &value);
    }

    #[qjs(get)]
    pub fn class_name(&self) -> String {
        self.attr("class")
    }

    #[qjs(set, rename = "className")]
    pub fn set_class_name(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("class", &value);
    }

    #[qjs(get)]
    pub fn name(&self) -> String {
        self.attr("name")
    }

    #[qjs(set, rename = "name")]
    pub fn set_name(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("name", &value);
    }

    #[qjs(get)]
    pub fn placeholder(&self) -> String {
        self.attr("placeholder")
    }

    #[qjs(set, rename = "placeholder")]
    pub fn set_placeholder(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("placeholder", &value);
    }

    #[qjs(get)]
    pub fn pattern(&self) -> String {
        self.attr("pattern")
    }

    #[qjs(set, rename = "pattern")]
    pub fn set_pattern(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("pattern", &value);
    }

    #[qjs(get)]
    pub fn min(&self) -> String {
        self.attr("min")
    }

    #[qjs(set, rename = "min")]
    pub fn set_min(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("min", &value);
    }

    #[qjs(get)]
    pub fn max(&self) -> String {
        self.attr("max")
    }

    #[qjs(set, rename = "max")]
    pub fn set_max(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("max", &value);
    }

    #[qjs(get)]
    pub fn step(&self) -> String {
        self.attr("step")
    }

    #[qjs(set, rename = "step")]
    pub fn set_step(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("step", &value);
    }

    #[qjs(get)]
    pub fn accept(&self) -> String {
        self.attr("accept")
    }

    #[qjs(set, rename = "accept")]
    pub fn set_accept(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("accept", &value);
    }

    #[qjs(get)]
    pub fn autocomplete(&self) -> String {
        self.attr("autocomplete")
    }

    #[qjs(set, rename = "autocomplete")]
    pub fn set_autocomplete(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("autocomplete", &value);
    }

    #[qjs(get)]
    pub fn action(&self) -> String {
        self.attr("action")
    }

    #[qjs(set, rename = "action")]
    pub fn set_action(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("action", &value);
    }

    #[qjs(get)]
    pub fn method(&self) -> String {
        self.attr("method")
    }

    #[qjs(set, rename = "method")]
    pub fn set_method(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("method", &value);
    }

    /// `HTMLLabelElement.htmlFor` reflects the `for` attribute.
    #[qjs(get, rename = "htmlFor")]
    pub fn html_for(&self) -> String {
        self.attr("for")
    }

    #[qjs(set, rename = "htmlFor")]
    pub fn set_html_for(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("for", &value);
    }

    #[qjs(get)]
    pub fn disabled(&self) -> bool {
        self.bool_attr("disabled")
    }

    #[qjs(set, rename = "disabled")]
    pub fn set_disabled(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("disabled", on);
    }

    #[qjs(get)]
    pub fn required(&self) -> bool {
        self.bool_attr("required")
    }

    #[qjs(set, rename = "required")]
    pub fn set_required(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("required", on);
    }

    #[qjs(get, rename = "readOnly")]
    pub fn read_only(&self) -> bool {
        self.bool_attr("readonly")
    }

    #[qjs(set, rename = "readOnly")]
    pub fn set_read_only(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("readonly", on);
    }

    #[qjs(get)]
    pub fn multiple(&self) -> bool {
        self.bool_attr("multiple")
    }

    #[qjs(set, rename = "multiple")]
    pub fn set_multiple(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("multiple", on);
    }

    #[qjs(get)]
    pub fn autofocus(&self) -> bool {
        self.bool_attr("autofocus")
    }

    #[qjs(set, rename = "autofocus")]
    pub fn set_autofocus(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("autofocus", on);
    }

    #[qjs(get, rename = "noValidate")]
    pub fn no_validate(&self) -> bool {
        self.bool_attr("novalidate")
    }

    #[qjs(set, rename = "noValidate")]
    pub fn set_no_validate(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("novalidate", on);
    }

    #[qjs(get, rename = "maxLength")]
    pub fn max_length(&self) -> i32 {
        self.long_attr("maxlength", -1)
    }

    #[qjs(set, rename = "maxLength")]
    pub fn set_max_length(&self, value: Coerced<i32>) {
        let value = value.0;
        self.set_attr("maxlength", &value.to_string());
    }

    #[qjs(get, rename = "minLength")]
    pub fn min_length(&self) -> i32 {
        self.long_attr("minlength", -1)
    }

    #[qjs(set, rename = "minLength")]
    pub fn set_min_length(&self, value: Coerced<i32>) {
        let value = value.0;
        self.set_attr("minlength", &value.to_string());
    }

    /// Defaults differ by element: 20 for an `<input>`, 0 (meaning "one row") for a `<select>`.
    #[qjs(get)]
    pub fn size(&self) -> i32 {
        let default = if self.doc.borrow().tag_name(self.id) == Some("select") {
            0
        } else {
            20
        };
        self.long_attr("size", default)
    }

    #[qjs(set, rename = "size")]
    pub fn set_size(&self, value: Coerced<i32>) {
        let value = value.0;
        self.set_attr("size", &value.to_string());
    }

    #[qjs(get)]
    pub fn rows(&self) -> i32 {
        self.long_attr("rows", 2)
    }

    #[qjs(set, rename = "rows")]
    pub fn set_rows(&self, value: Coerced<i32>) {
        let value = value.0;
        self.set_attr("rows", &value.to_string());
    }

    #[qjs(get)]
    pub fn cols(&self) -> i32 {
        self.long_attr("cols", 20)
    }

    #[qjs(set, rename = "cols")]
    pub fn set_cols(&self, value: Coerced<i32>) {
        let value = value.0;
        self.set_attr("cols", &value.to_string());
    }

    /// `HTMLInputElement.type` / `HTMLTextAreaElement.type`.
    #[qjs(get, rename = "type")]
    pub fn control_type(&self) -> Option<String> {
        let doc = self.doc.borrow();
        match doc.tag_name(self.id) {
            Some("textarea") => Some("textarea".to_string()),
            Some("input") => Some(
                doc.attribute(self.id, "type")
                    .map(|t| t.cow_to_ascii_lowercase().into_owned())
                    .unwrap_or_else(|| "text".to_string()),
            ),
            Some("button") => Some(
                doc.attribute(self.id, "type")
                    .map(|t| t.cow_to_ascii_lowercase().into_owned())
                    .unwrap_or_else(|| "submit".to_string()),
            ),
            _ => None,
        }
    }

    /// A `<textarea>`'s type is fixed; only `<input>` and `<button>` take a new one.
    #[qjs(set, rename = "type")]
    pub fn set_control_type(&self, value: Coerced<String>) {
        let value = value.0;
        if matches!(self.doc.borrow().tag_name(self.id), Some("textarea")) {
            return;
        }
        self.set_attr("type", &value);
    }

    // ── live control state ─────────────────────────────────────────────────

    /// The control's current value. Which state that reads depends on the element and, for
    /// an `<input>`, on its value mode - both decided by engine code, not here.
    #[qjs(get)]
    pub fn value(&self) -> Option<String> {
        let doc = self.doc.borrow();
        match doc.tag_name(self.id)? {
            "option" => Some(form::option_value::<DomConfig>(&doc, self.id)),
            "button" => Some(doc.attribute(self.id, "value").unwrap_or_default().to_string()),
            "select" => Some(
                doc.selected_option(self.id)
                    .map(|option| form::option_value::<DomConfig>(&doc, option))
                    .unwrap_or_default(),
            ),
            _ => match edit::value_mode::<DomConfig>(&doc, self.id)? {
                // The IDL value is sanitized; what the painter shows is not.
                edit::ValueMode::Value => Some(edit::api_value::<DomConfig>(&doc, self.id)),
                edit::ValueMode::Default => Some(doc.attribute(self.id, "value").unwrap_or_default().to_string()),
                edit::ValueMode::DefaultOn => Some(doc.attribute(self.id, "value").unwrap_or("on").to_string()),
                edit::ValueMode::Filename => Some(String::new()),
            },
        }
    }

    #[qjs(set, rename = "value")]
    pub fn set_value(&self, value: Coerced<String>) {
        let value = value.0;
        let tag = self.doc.borrow().tag_name(self.id).map(str::to_string);
        match tag.as_deref() {
            Some("option" | "button") => self.set_attr("value", &value),
            Some("select") => self.select_by_value(&value),
            _ => {
                let mode = edit::value_mode::<DomConfig>(&self.doc.borrow(), self.id);
                match mode {
                    // Setting the value moves the text entry cursor to the end.
                    Some(edit::ValueMode::Value) => {
                        let doc = self.doc.borrow();
                        let value = edit::sanitize_value::<DomConfig>(&doc, self.id, &value);
                        let caret = value.chars().count();
                        doc.set_control_edit_state(self.id, Some(ControlEditState::new(value, caret)));
                    }
                    Some(edit::ValueMode::Default | edit::ValueMode::DefaultOn) => self.set_attr("value", &value),
                    _ => {}
                }
            }
        }
    }

    /// Live checkedness. The `checked` attribute is only the default - see `defaultChecked`.
    #[qjs(get)]
    pub fn checked(&self) -> bool {
        self.doc.borrow().is_checked(self.id)
    }

    #[qjs(set, rename = "checked")]
    pub fn set_checked(&self, on: Coerced<bool>) {
        let on = on.0;
        self.doc.borrow().set_checked(self.id, Some(on));
    }

    #[qjs(get, rename = "defaultChecked")]
    pub fn default_checked(&self) -> bool {
        self.bool_attr("checked")
    }

    #[qjs(set, rename = "defaultChecked")]
    pub fn set_default_checked(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("checked", on);
    }

    /// The markup value: an `<input>`'s `value` attribute, a `<textarea>`'s child text.
    #[qjs(get, rename = "defaultValue")]
    pub fn default_value(&self) -> String {
        edit::initial_value::<DomConfig>(&self.doc.borrow(), self.id)
    }

    #[qjs(set, rename = "defaultValue")]
    pub fn set_default_value(&self, value: Coerced<String>) {
        let value = value.0;
        if self.doc.borrow().tag_name(self.id) == Some("textarea") {
            self.set_text_content(Coerced(value));
            return;
        }
        self.set_attr("value", &value);
    }

    /// Live selectedness of an `<option>`; `defaultSelected` is the attribute.
    #[qjs(get)]
    pub fn selected(&self) -> bool {
        let doc = self.doc.borrow();
        match self.owner_select() {
            Some(select) => doc.selected_option(select) == Some(self.id),
            None => doc.attribute(self.id, "selected").is_some(),
        }
    }

    #[qjs(set, rename = "selected")]
    pub fn set_selected(&self, on: Coerced<bool>) {
        let on = on.0;
        let Some(select) = self.owner_select() else {
            return;
        };
        let doc = self.doc.borrow();
        if on {
            doc.set_selected_option(select, Some(self.id));
        } else if doc.selected_option(select) == Some(self.id) {
            doc.set_selected_option(select, None);
        }
    }

    #[qjs(get, rename = "defaultSelected")]
    pub fn default_selected(&self) -> bool {
        self.bool_attr("selected")
    }

    #[qjs(set, rename = "defaultSelected")]
    pub fn set_default_selected(&self, on: Coerced<bool>) {
        let on = on.0;
        self.set_bool_attr("selected", on);
    }

    #[qjs(get, rename = "selectedIndex")]
    pub fn selected_index(&self) -> i32 {
        let doc = self.doc.borrow();
        let Some(chosen) = doc.selected_option(self.id) else {
            return -1;
        };
        drop(doc);
        self.option_ids()
            .iter()
            .position(|&option| option == chosen)
            .map_or(-1, |index| index as i32)
    }

    #[qjs(set, rename = "selectedIndex")]
    pub fn set_selected_index(&self, index: i32) {
        let options = self.option_ids();
        let chosen = usize::try_from(index).ok().and_then(|i| options.get(i).copied());
        self.doc.borrow().set_selected_option(self.id, chosen);
    }

    #[qjs(get)]
    pub fn options<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        wrap_list(&ctx, &self.doc, &self.option_ids())
    }

    #[qjs(get, rename = "selectedOptions")]
    pub fn selected_options<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let chosen: Vec<NodeId> = self.doc.borrow().selected_option(self.id).into_iter().collect();
        wrap_list(&ctx, &self.doc, &chosen)
    }

    /// `HTMLOptionElement.text`: the option's descendant text, stripped and collapsed.
    #[qjs(get)]
    pub fn text(&self) -> Option<String> {
        let doc = self.doc.borrow();
        (doc.tag_name(self.id) == Some("option")).then(|| form::option_text::<DomConfig>(&doc, self.id))
    }

    #[qjs(set, rename = "text")]
    pub fn set_text(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_text_content(Coerced(value));
    }

    /// `HTMLOptionElement.label`: the `label` attribute if present, else the option's text.
    #[qjs(get)]
    pub fn label(&self) -> Option<String> {
        let doc = self.doc.borrow();
        if doc.tag_name(self.id) != Some("option") {
            return None;
        }
        match doc.attribute(self.id, "label") {
            Some(label) => Some(label.to_string()),
            None => Some(form::option_text::<DomConfig>(&doc, self.id)),
        }
    }

    #[qjs(set, rename = "label")]
    pub fn set_label(&self, value: Coerced<String>) {
        let value = value.0;
        self.set_attr("label", &value);
    }

    #[qjs(get, rename = "textLength")]
    pub fn text_length(&self) -> u32 {
        self.value().unwrap_or_default().chars().count() as u32
    }

    /// The `<form>` this control belongs to - its `form` attribute's target, else the
    /// nearest ancestor form.
    #[qjs(get)]
    pub fn form<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let owner = form::form_owner::<DomConfig>(&self.doc.borrow(), self.id);
        wrap_opt(&ctx, &self.doc, owner)
    }

    // ── content ────────────────────────────────────────────────────────────

    #[qjs(get)]
    pub fn text_content(&self) -> String {
        let doc = self.doc.borrow();
        text::descendant_text(&doc, self.id)
    }

    #[qjs(set, rename = "textContent")]
    pub fn set_text_content(&self, value: Coerced<String>) {
        let value = value.0;
        let children = self.doc.borrow().children(self.id).to_vec();
        let mut doc = self.doc.borrow_mut();
        for child in children {
            doc.remove(child);
        }
        if !value.is_empty() {
            let text = doc.create_text(&value, Location::default());
            doc.attach(text, self.id, None);
        }
    }

    #[qjs(get, rename = "outerHTML")]
    pub fn outer_html(&self) -> String {
        self.doc.borrow().write_from_node(self.id)
    }

    // ── queries ────────────────────────────────────────────────────────────

    pub fn query_selector<'js>(&self, ctx: Ctx<'js>, selector: String) -> Result<Value<'js>> {
        let found = crate::document::first_match(&self.doc.borrow(), self.id, &selector)
            .map_err(|e| Exception::throw_message(&ctx, &e))?;
        wrap_opt(&ctx, &self.doc, found)
    }

    #[qjs(rename = "getElementsByTagName")]
    pub fn get_elements_by_tag_name<'js>(&self, ctx: Ctx<'js>, name: String) -> Result<Value<'js>> {
        let doc = self.doc.borrow();
        let found: Vec<NodeId> = select::descendants(&doc, self.id)
            .into_iter()
            .filter(|&id| doc.tag_name(id).is_some_and(|tag| tag.eq_ignore_ascii_case(&name)))
            .collect();
        drop(doc);
        wrap_list(&ctx, &self.doc, &found)
    }

    // ── constraint validation ──────────────────────────────────────────────

    #[qjs(get, rename = "willValidate")]
    pub fn will_validate(&self) -> bool {
        validity::will_validate::<DomConfig>(&self.doc.borrow(), self.id)
    }

    #[qjs(get)]
    pub fn validity<'js>(&self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let state = DomValidity::new(self.doc.clone(), self.id);
        Ok(rquickjs::Class::instance(ctx, state)?.into_value())
    }

    #[qjs(get, rename = "validationMessage")]
    pub fn validation_message(&self) -> String {
        validity::validation_message::<DomConfig>(&self.doc.borrow(), self.id)
    }

    /// Validating fires an `invalid` event at every control that fails - at this element,
    /// or at each of a `<form>`'s controls when asked of a form.
    #[qjs(rename = "checkValidity")]
    pub fn check_validity<'js>(&self, ctx: Ctx<'js>) -> Result<bool> {
        let failing = {
            let doc = self.doc.borrow();
            if doc.tag_name(self.id) == Some("form") {
                validity::invalid_controls::<DomConfig>(&doc, self.id)
            } else if validity::check_validity::<DomConfig>(&doc, self.id) {
                Vec::new()
            } else {
                vec![self.id]
            }
        };
        for id in &failing {
            let event = rquickjs::Class::instance(ctx.clone(), event::DomEvent::synthetic("invalid", false, true))?;
            event::dispatch(&ctx, &self.doc, *id, event)?;
        }
        Ok(failing.is_empty())
    }

    /// Without a UI to show the message in, reporting is the same answer as checking.
    #[qjs(rename = "reportValidity")]
    pub fn report_validity<'js>(&self, ctx: Ctx<'js>) -> Result<bool> {
        self.check_validity(ctx)
    }

    #[qjs(rename = "setCustomValidity")]
    pub fn set_custom_validity(&self, message: Coerced<String>) {
        self.doc.borrow().set_custom_validity(self.id, &message.0);
    }

    // ── events ─────────────────────────────────────────────────────────────

    #[qjs(rename = "addEventListener")]
    pub fn add_event_listener<'js>(
        &self,
        ctx: Ctx<'js>,
        event_type: String,
        callback: rquickjs::Function<'js>,
        options: rquickjs::prelude::Opt<Value<'js>>,
    ) -> Result<()> {
        event::add(&ctx, u64::from(self.id), event_type, callback, options)
    }

    #[qjs(rename = "removeEventListener")]
    pub fn remove_event_listener<'js>(
        &self,
        ctx: Ctx<'js>,
        event_type: String,
        callback: rquickjs::Function<'js>,
        options: rquickjs::prelude::Opt<Value<'js>>,
    ) -> Result<()> {
        event::remove(&ctx, u64::from(self.id), &event_type, &callback, options)
    }

    #[qjs(rename = "dispatchEvent")]
    pub fn dispatch_event<'js>(
        &self,
        ctx: Ctx<'js>,
        event: rquickjs::Class<'js, event::DomEvent<'js>>,
    ) -> Result<bool> {
        event::dispatch(&ctx, &self.doc, self.id, event)
    }

    /// Fires a click event. There is no activation behaviour behind it yet - a checkbox does
    /// not toggle and a submit button does not submit, because those live in engine code
    /// this crate cannot reach.
    pub fn click<'js>(&self, ctx: Ctx<'js>) -> Result<()> {
        let event = rquickjs::Class::instance(ctx.clone(), event::DomEvent::synthetic("click", true, true))?;
        event::dispatch(&ctx, &self.doc, self.id, event)?;
        Ok(())
    }

    #[qjs(rename = "toString")]
    pub fn to_string_js(&self) -> String {
        format!("[object Node {}]", self.node_name())
    }
}

/// A plain JS array; a real `NodeList`/`HTMLCollection` is live and has `item()`.
pub(crate) fn wrap_list<'js>(ctx: &Ctx<'js>, doc: &DocHandle, ids: &[NodeId]) -> Result<Value<'js>> {
    let array = rquickjs::Array::new(ctx.clone())?;
    for (index, &id) in ids.iter().enumerate() {
        array.set(index, wrap(ctx, doc, id)?)?;
    }
    Ok(array.into_value())
}

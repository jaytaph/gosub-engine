//! Form submission and reset: the form data set (with what the user typed/toggled/picked), its
//! `application/x-www-form-urlencoded` encoding, and the request a submit turns into.

use crate::engine::edit;
use crate::html::{DomConfiguration, EngineDocument};
use cow_utils::CowUtils;
use gosub_interface::document::Document as _;
use gosub_shared::node::NodeId;
use url::Url;

/// What a form submit navigates to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub url: Url,
    pub post: bool,
    /// urlencoded body for POST; GET carries the data in `url`'s query.
    pub body: Option<String>,
}

/// Elements that can have a form owner at all.
const FORM_ASSOCIATED: [&str; 9] = [
    "button", "fieldset", "input", "img", "label", "object", "output", "select", "textarea",
];

/// Listed elements: the ones that honour the `form` content attribute and take part in the
/// form data set. `<img>` and `<label>` are form-associated but not listed.
const LISTED: [&str; 7] = ["button", "fieldset", "input", "object", "output", "select", "textarea"];

/// The `<form>` that owns `id`.
///
/// Three cases, in order: an association the parser made (which outlives tree moves that
/// carry the element along with its form), then the `form` content attribute, then the
/// nearest ancestor form. Everything but the first is a live read of the tree, so an id
/// changing anywhere gives a different answer next time - which is what the spec's "reset
/// the form owner" achieves by re-running on every mutation.
pub fn form_owner<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<NodeId> {
    let tag = doc.tag_name(id)?;
    if !FORM_ASSOCIATED.contains(&tag) {
        return None;
    }
    if let Some(owner) = doc.parser_form_owner(id) {
        return Some(owner);
    }
    if LISTED.contains(&tag) {
        if let Some(target) = doc.attribute(id, "form") {
            // A detached control ignores its form attribute and falls back to its ancestors.
            if is_connected(doc, id) {
                // Only a form counts, and only the first element in tree order with that id:
                // a `<span>` with the same id earlier in the document means no owner at all.
                return first_by_id(doc, target).filter(|&node| doc.tag_name(node) == Some("form"));
            }
        }
    }
    nearest_ancestor_form(doc, id)
}

/// What `HTMLLabelElement.form` returns: **not** the label's own owner, but the form owner
/// of the control it labels.
pub fn label_form<C: DomConfiguration>(doc: &EngineDocument<C>, label: NodeId) -> Option<NodeId> {
    let control = crate::engine::focus::label_control(doc, label)?;
    if !LISTED.contains(&doc.tag_name(control)?) {
        return None;
    }
    form_owner(doc, control)
}

fn nearest_ancestor_form<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<NodeId> {
    let mut current = doc.parent(id)?;
    loop {
        if doc.tag_name(current) == Some("form") {
            return Some(current);
        }
        current = doc.parent(current)?;
    }
}

/// Whether `id` hangs off the document rather than a detached subtree.
fn is_connected<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    let mut current = id;
    while let Some(parent) = doc.parent(current) {
        current = parent;
    }
    current == doc.root()
}

/// The first element in tree order whose `id` attribute is exactly `target`. An empty
/// string is not an ID, so it never matches anything.
fn first_by_id<C: DomConfiguration>(doc: &EngineDocument<C>, target: &str) -> Option<NodeId> {
    first_by_id_within(doc, doc.root(), target)
}

/// The same search, but scoped to one tree: an element in a detached subtree can only see
/// the ids in that subtree, not the document's.
pub fn first_by_id_within<C: DomConfiguration>(doc: &EngineDocument<C>, root: NodeId, target: &str) -> Option<NodeId> {
    if target.is_empty() {
        return None;
    }
    let mut stack: Vec<NodeId> = doc.children(root).iter().rev().copied().collect();
    while let Some(node) = stack.pop() {
        stack.extend(doc.children(node).iter().rev());
        if doc.attribute(node, "id") == Some(target) {
            return Some(node);
        }
    }
    None
}

/// Every element `form` owns, in tree order. Not the same as its descendants: a control can
/// point at a form it sits outside of, and a control inside a form can point somewhere else.
pub fn owned_elements<C: DomConfiguration>(doc: &EngineDocument<C>, form: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = doc.children(doc.root()).iter().rev().copied().collect();
    while let Some(node) = stack.pop() {
        stack.extend(doc.children(node).iter().rev());
        if form_owner(doc, node) == Some(form) {
            out.push(node);
        }
    }
    out
}

fn input_type<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> String {
    doc.attribute(id, "type")
        .map(|t| t.cow_to_ascii_lowercase().into_owned())
        .unwrap_or_else(|| "text".to_string())
}

/// `Some(is_reset)` when `id` is an enabled submit or reset button.
pub fn button_kind<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<bool> {
    if edit::is_disabled(doc, id) {
        return None;
    }
    let ty = match doc.tag_name(id) {
        Some("button") => doc
            .attribute(id, "type")
            .map(|t| t.cow_to_ascii_lowercase().into_owned())
            .unwrap_or_else(|| "submit".to_string()),
        Some("input") => input_type(doc, id),
        _ => return None,
    };
    match ty.as_str() {
        "submit" | "image" => Some(false),
        "reset" => Some(true),
        _ => None,
    }
}

/// The form data set: `(name, value)` of every successful control in `form`, in tree order.
/// `submitter` is the button that triggered the submit (other buttons don't contribute).
pub fn data_set<C: DomConfiguration>(
    doc: &EngineDocument<C>,
    form: NodeId,
    submitter: Option<NodeId>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in owned_elements(doc, form) {
        let (Some(tag), Some(name)) = (doc.tag_name(id), doc.attribute(id, "name")) else {
            continue;
        };
        if name.is_empty() || edit::is_disabled(doc, id) {
            continue;
        }
        let value = match tag {
            "input" => match input_type(doc, id).as_str() {
                "checkbox" | "radio" => {
                    if !doc.is_checked(id) {
                        continue;
                    }
                    doc.attribute(id, "value").unwrap_or("on").to_string()
                }
                "submit" | "reset" | "button" | "image" => {
                    if Some(id) != submitter {
                        continue;
                    }
                    doc.attribute(id, "value").unwrap_or_default().to_string()
                }
                "file" => continue,
                _ => live_value(doc, id),
            },
            "textarea" => live_value(doc, id),
            "select" => {
                let Some(opt) = doc.selected_option(id) else {
                    continue;
                };
                option_value(doc, opt)
            }
            "button" => {
                if Some(id) != submitter {
                    continue;
                }
                doc.attribute(id, "value").unwrap_or_default().to_string()
            }
            _ => continue,
        };
        out.push((name.to_string(), value));
    }
    out
}

/// What a control currently holds: what the user typed, else its markup value.
pub fn live_value<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> String {
    doc.control_edit_state(id)
        .map(|s| s.value)
        .unwrap_or_else(|| edit::initial_value(doc, id))
}

/// An `<option>`'s text: the text of every descendant, stripped and collapsed. Descendant,
/// not child - `<option>a<b>x</b></option>` reads as `"ax"` - and inner whitespace runs
/// collapse to one space, so `" a  b "` reads as `"a b"`.
pub fn option_text<C: DomConfiguration>(doc: &EngineDocument<C>, opt: NodeId) -> String {
    let mut raw = String::new();
    collect_text(doc, opt, &mut raw);
    strip_and_collapse(&raw)
}

fn collect_text<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, out: &mut String) {
    if let Some(text) = doc.text_value(id) {
        out.push_str(text);
    }
    for &child in doc.children(id) {
        collect_text(doc, child, out);
    }
}

/// Infra's "strip and collapse ASCII whitespace".
fn strip_and_collapse(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;
    for ch in input.chars() {
        if ch.is_ascii_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    out
}

/// An option's submission value: its `value` attribute, else its text.
pub fn option_value<C: DomConfiguration>(doc: &EngineDocument<C>, opt: NodeId) -> String {
    match doc.attribute(opt, "value") {
        Some(value) => value.to_string(),
        None => option_text(doc, opt),
    }
}

/// The request for submitting `form` via `submitter`, resolved against `base`. Only
/// urlencoded GET/POST; other methods/enctypes fall back to that.
pub fn submission<C: DomConfiguration>(
    doc: &EngineDocument<C>,
    form: NodeId,
    submitter: Option<NodeId>,
    base: &Url,
) -> Option<Submission> {
    // formaction/formmethod on the submitter override the form's.
    let attr = |name: &str, form_name: &str| {
        submitter
            .and_then(|s| doc.attribute(s, name))
            .or_else(|| doc.attribute(form, form_name))
    };
    let action = attr("formaction", "action").unwrap_or("");
    let mut url = if action.trim().is_empty() {
        base.clone()
    } else {
        base.join(action.trim()).ok()?
    };
    let post = attr("formmethod", "method").is_some_and(|m| m.eq_ignore_ascii_case("post"));

    let mut encoded = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in data_set(doc, form, submitter) {
        encoded.append_pair(&k, &v);
    }
    let encoded = encoded.finish();

    if post {
        return Some(Submission {
            url,
            post: true,
            body: Some(encoded),
        });
    }
    url.set_query(if encoded.is_empty() { None } else { Some(&encoded) });
    url.set_fragment(None);
    Some(Submission {
        url,
        post: false,
        body: None,
    })
}

/// Reset `form`: forget everything typed, toggled or picked in it, so every control falls
/// back to its markup value again.
pub fn reset<C: DomConfiguration>(doc: &EngineDocument<C>, form: NodeId) {
    for id in controls(doc, form) {
        doc.set_control_edit_state(id, None);
        doc.set_checked(id, None);
        doc.set_selected_option(id, None);
    }
}

/// The controls of `form` whose live state a reset should forget.
pub fn controls<C: DomConfiguration>(doc: &EngineDocument<C>, form: NodeId) -> Vec<NodeId> {
    owned_elements(doc, form)
        .into_iter()
        .filter(|&id| matches!(doc.tag_name(id), Some("input" | "textarea" | "select")))
        .collect()
}

/// The button an Enter key in a text field submits through: the form's first submit button.
/// Without one, implicit submission still happens when the form has a single text field.
pub fn default_submitter<C: DomConfiguration>(doc: &EngineDocument<C>, form: NodeId) -> Option<Option<NodeId>> {
    let mut text_fields = 0;
    for id in owned_elements(doc, form) {
        if button_kind(doc, id) == Some(false) {
            return Some(Some(id));
        }
        if edit::text_entry_kind(doc, id) == Some(false) {
            text_fields += 1;
        }
    }
    (text_fields == 1).then_some(None)
}

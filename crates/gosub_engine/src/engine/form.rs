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

/// The `<form>` a control belongs to: its `form` attribute's target, else the nearest ancestor.
pub fn form_owner<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<NodeId> {
    if let Some(target) = doc.attribute(id, "form").and_then(|f| doc.node_by_named_id(f)) {
        if doc.tag_name(target) == Some("form") {
            return Some(target);
        }
    }
    let mut cur = doc.parent(id)?;
    loop {
        if doc.tag_name(cur) == Some("form") {
            return Some(cur);
        }
        cur = doc.parent(cur)?;
    }
}

fn input_type<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> String {
    doc.attribute(id, "type")
        .map(|t| t.cow_to_ascii_lowercase().into_owned())
        .unwrap_or_else(|| "text".to_string())
}

/// `Some(is_reset)` when `id` is an enabled submit or reset button.
pub fn button_kind<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<bool> {
    if doc.attribute(id, "disabled").is_some() {
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
    let mut stack: Vec<NodeId> = doc.children(form).iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        stack.extend(doc.children(id).iter().rev());
        let (Some(tag), Some(name)) = (doc.tag_name(id), doc.attribute(id, "name")) else {
            continue;
        };
        if name.is_empty() || doc.attribute(id, "disabled").is_some() {
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

/// The controls of `form` whose live state a reset should forget.
pub fn controls<C: DomConfiguration>(doc: &EngineDocument<C>, form: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = doc.children(form).iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        stack.extend(doc.children(id).iter().rev());
        if matches!(doc.tag_name(id), Some("input" | "textarea" | "select")) {
            out.push(id);
        }
    }
    out
}

/// The button an Enter key in a text field submits through: the form's first submit button.
/// Without one, implicit submission still happens when the form has a single text field.
pub fn default_submitter<C: DomConfiguration>(doc: &EngineDocument<C>, form: NodeId) -> Option<Option<NodeId>> {
    let mut text_fields = 0;
    let mut stack: Vec<NodeId> = doc.children(form).iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        stack.extend(doc.children(id).iter().rev());
        if button_kind(doc, id) == Some(false) {
            return Some(Some(id));
        }
        if edit::text_entry_kind(doc, id) == Some(false) {
            text_fields += 1;
        }
    }
    (text_fields == 1).then_some(None)
}

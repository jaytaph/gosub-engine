//! Constraint validation: whether a control is a candidate, which constraints it fails, and
//! the message a UA would show for that failure.
//!
//! Everything here is a pure read of the document, so `willValidate`/`validity` report the
//! same thing whether they are asked by the engine, a test, or (eventually) script.

use crate::engine::edit;
use crate::html::{DomConfiguration, EngineDocument};
use cow_utils::CowUtils;
use gosub_interface::document::Document as _;
use gosub_shared::node::NodeId;

/// The constraint-validation flags of one control - the `ValidityState` of the spec.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Validity {
    pub value_missing: bool,
    pub type_mismatch: bool,
    pub pattern_mismatch: bool,
    pub too_long: bool,
    pub too_short: bool,
    pub range_underflow: bool,
    pub range_overflow: bool,
    pub step_mismatch: bool,
    /// The control holds input the UA could not convert. Nothing produces this yet: the
    /// engine has no type-specific editors that can hold an unconvertible value.
    pub bad_input: bool,
    pub custom_error: bool,
}

impl Validity {
    pub fn valid(&self) -> bool {
        *self == Validity::default()
    }
}

fn input_type<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> String {
    doc.attribute(id, "type")
        .map(|t| t.cow_to_ascii_lowercase().into_owned())
        .unwrap_or_else(|| "text".to_string())
}

/// Whether `id` is a candidate for constraint validation.
///
/// Barred: inputs in the hidden/reset/button states, buttons that do not submit, anything
/// disabled or read-only, and anything inside a `<datalist>`.
pub fn will_validate<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    let Some(tag) = doc.tag_name(id) else {
        return false;
    };
    if !matches!(tag, "input" | "select" | "textarea" | "button") {
        return false;
    }
    if edit::is_disabled(doc, id) {
        return false;
    }
    if matches!(tag, "input" | "textarea") && doc.attribute(id, "readonly").is_some() {
        return false;
    }
    if has_ancestor(doc, id, "datalist") {
        return false;
    }
    match tag {
        "input" => !matches!(input_type(doc, id).as_str(), "hidden" | "reset" | "button"),
        "button" => doc
            .attribute(id, "type")
            .is_none_or(|t| !matches!(t.cow_to_ascii_lowercase().as_ref(), "reset" | "button")),
        _ => true,
    }
}

fn has_ancestor<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, tag: &str) -> bool {
    let mut current = id;
    while let Some(parent) = doc.parent(current) {
        if doc.tag_name(parent) == Some(tag) {
            return true;
        }
        current = parent;
    }
    false
}

/// Which constraints `id` currently fails.
pub fn validity<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Validity {
    let mut flags = Validity {
        custom_error: doc.custom_validity(id).is_some_and(|m| !m.is_empty()),
        ..Validity::default()
    };
    let Some(tag) = doc.tag_name(id) else {
        return flags;
    };

    let value = edit::api_value(doc, id);
    let required = doc.attribute(id, "required").is_some();
    let ty = input_type(doc, id);

    flags.value_missing = required && is_missing(doc, id, tag, &ty, &value);

    if tag == "input" {
        flags.type_mismatch = type_mismatch(doc, id, &ty, &value);
        flags.pattern_mismatch = pattern_mismatch(doc, id, &ty, &value);
        let (under, over, step) = numeric_flags(doc, id, &ty, &value);
        flags.range_underflow = under;
        flags.range_overflow = over;
        flags.step_mismatch = step;
    }
    if tag == "input" || tag == "textarea" {
        // Length constraints only apply once the user (or script) has changed the value.
        let dirty = doc.control_edit_state(id).is_some();
        let length = value.chars().count() as i64;
        if let Some(max) = length_attr(doc, id, "maxlength") {
            flags.too_long = dirty && length > max;
        }
        if let Some(min) = length_attr(doc, id, "minlength") {
            flags.too_short = dirty && length > 0 && length < min;
        }
    }
    flags
}

fn is_missing<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, tag: &str, ty: &str, value: &str) -> bool {
    match tag {
        "select" => match doc.selected_option(id) {
            // A placeholder label option counts as no selection.
            Some(option) => crate::engine::form::option_value(doc, option).is_empty(),
            None => true,
        },
        "textarea" => value.is_empty(),
        "input" => match ty {
            "checkbox" => !doc.is_checked(id),
            "radio" => !radio_group_checked(doc, id),
            "hidden" | "range" | "color" | "submit" | "reset" | "button" | "image" => false,
            _ => value.is_empty(),
        },
        _ => false,
    }
}

/// A required radio is satisfied when any member of its group is checked.
fn radio_group_checked<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    let Some(name) = doc.attribute(id, "name") else {
        return doc.is_checked(id);
    };
    let scope = crate::engine::form::form_owner(doc, id).unwrap_or_else(|| doc.root());
    let mut stack: Vec<NodeId> = doc.children(scope).iter().rev().copied().collect();
    while let Some(node) = stack.pop() {
        stack.extend(doc.children(node).iter().rev());
        if doc.tag_name(node) == Some("input")
            && input_type(doc, node) == "radio"
            && doc.attribute(node, "name") == Some(name)
            && doc.is_checked(node)
        {
            return true;
        }
    }
    false
}

fn type_mismatch<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, ty: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    match ty {
        "email" if doc.attribute(id, "multiple").is_some() => value.split(',').any(|part| !is_email(part.trim())),
        "email" => !is_email(value),
        "url" => url::Url::parse(value).is_err(),
        _ => false,
    }
}

/// The spec's email production, which is deliberately narrower than RFC 5322.
fn is_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() || domain.len() > 253 {
        return false;
    }
    let local_ok = local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+/=?^_`{|}~.-".contains(c));
    let label_ok = |label: &str| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    local_ok && domain.split('.').all(label_ok)
}

fn pattern_mismatch<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, ty: &str, value: &str) -> bool {
    if value.is_empty() || !matches!(ty, "text" | "search" | "url" | "tel" | "email" | "password") {
        return false;
    }
    let Some(pattern) = doc.attribute(id, "pattern") else {
        return false;
    };
    // The pattern must match the whole value. A pattern the engine cannot compile is
    // ignored, which is what the spec says to do with one that fails to parse.
    let Ok(regex) = regex::Regex::new(&format!("^(?:{pattern})$")) else {
        return false;
    };
    if doc.attribute(id, "multiple").is_some() && ty == "email" {
        return value.split(',').any(|part| !regex.is_match(part.trim()));
    }
    !regex.is_match(value)
}

fn length_attr<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, name: &str) -> Option<i64> {
    doc.attribute(id, name)?.trim().parse::<i64>().ok().filter(|&n| n >= 0)
}

/// `(underflow, overflow, step mismatch)` for the numeric input types.
fn numeric_flags<C: DomConfiguration>(
    doc: &EngineDocument<C>,
    id: NodeId,
    ty: &str,
    value: &str,
) -> (bool, bool, bool) {
    if !matches!(ty, "number" | "range") {
        return (false, false, false);
    }
    let Some(number) = edit::parse_number(value) else {
        return (false, false, false);
    };
    let min = doc.attribute(id, "min").and_then(edit::parse_number);
    let max = doc.attribute(id, "max").and_then(edit::parse_number);

    let step_attr = doc.attribute(id, "step").map(str::trim);
    let step_mismatch = match step_attr {
        Some(s) if s.eq_ignore_ascii_case("any") => false,
        _ => {
            let step = step_attr
                .and_then(edit::parse_number)
                .filter(|s| *s > 0.0)
                .unwrap_or(1.0);
            let base = min.unwrap_or(0.0);
            let offset = (number - base) / step;
            (offset - offset.round()).abs() * step > 1e-9
        }
    };
    (
        min.is_some_and(|m| number < m),
        max.is_some_and(|m| number > m),
        step_mismatch,
    )
}

/// The message a UA shows for the first failing constraint. Empty when the control is valid
/// or barred from validation.
pub fn validation_message<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> String {
    if !will_validate(doc, id) {
        return String::new();
    }
    let flags = validity(doc, id);
    if flags.custom_error {
        return doc.custom_validity(id).unwrap_or_default();
    }
    let message = match flags {
        f if f.value_missing => "Please fill out this field.",
        f if f.type_mismatch => "Please enter a value of the correct type.",
        f if f.pattern_mismatch => "Please match the requested format.",
        f if f.too_long => "Please shorten this text.",
        f if f.too_short => "Please lengthen this text.",
        f if f.range_underflow => "Value must be greater than or equal to the minimum.",
        f if f.range_overflow => "Value must be less than or equal to the maximum.",
        f if f.step_mismatch => "Please enter a valid value.",
        f if f.bad_input => "Please enter a valid value.",
        _ => "",
    };
    message.to_string()
}

/// Whether `id` satisfies its constraints - the answer `checkValidity()` gives.
pub fn check_validity<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    !will_validate(doc, id) || validity(doc, id).valid()
}

/// Every control a `<form>` owns that currently fails validation, in tree order. Validating
/// a form means validating these, and the caller fires an `invalid` event at each.
pub fn invalid_controls<C: DomConfiguration>(doc: &EngineDocument<C>, form: NodeId) -> Vec<NodeId> {
    crate::engine::form::owned_elements(doc, form)
        .into_iter()
        .filter(|&id| will_validate(doc, id) && !validity(doc, id).valid())
        .collect()
}

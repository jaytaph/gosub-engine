//! What value a form control holds.
//!
//! This lives next to the DOM, not in the engine, because everyone needs the same answer:
//! the layouter sizes a field from it, the painter draws it, the engine submits it and
//! script reads it. When these drifted apart, `<input type=date value="not a date">` painted
//! its raw attribute while `.value` reported the empty string.

use cow_utils::CowUtils;
use gosub_interface::control::ControlHost;
use gosub_interface::document::ControlEditState;
use gosub_shared::node::NodeId;

use crate::temporal;

/// How a control's `value` behaves: the spec gives every `<input>` type one of these modes,
/// and they disagree about whether the value is live state or just the content attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueMode {
    /// Live editable state; the `value` attribute is only the default.
    Value,
    /// The `value` attribute itself.
    Default,
    /// The `value` attribute, or `"on"` when it is absent.
    DefaultOn,
    /// The selected file's name - always empty until uploads exist.
    Filename,
}

/// The value mode of `id`, or `None` when it is not a control with a value at all.
pub fn value_mode<H: ControlHost + ?Sized>(doc: &H, id: NodeId) -> Option<ValueMode> {
    match doc.tag_name(id)?.as_str() {
        "textarea" => Some(ValueMode::Value),
        "input" => Some(
            match doc
                .attribute(id, "type")
                .map(|t| t.cow_to_ascii_lowercase().into_owned())
                .as_deref()
            {
                Some("checkbox" | "radio") => ValueMode::DefaultOn,
                Some("file") => ValueMode::Filename,
                Some("hidden" | "submit" | "image" | "reset" | "button") => ValueMode::Default,
                _ => ValueMode::Value,
            },
        ),
        _ => None,
    }
}

/// The markup value: the `value` attribute, or a `<textarea>`'s text content minus the one
/// leading newline HTML allows after the start tag.
pub fn markup_value<H: ControlHost + ?Sized>(doc: &H, id: NodeId) -> String {
    if doc.tag_name(id).as_deref() == Some("textarea") {
        let mut out = String::new();
        for child in doc.children(id) {
            if let Some(t) = doc.text_value(child) {
                out.push_str(&t);
            }
        }
        return out.strip_prefix('\n').unwrap_or(&out).to_string();
    }
    doc.attribute(id, "value").unwrap_or_default()
}

/// The value sanitization algorithm: what a control does to a value on its way in.
pub fn sanitize_value<H: ControlHost + ?Sized>(doc: &H, id: NodeId, raw: &str) -> String {
    // A textarea keeps its line breaks but normalises them: CRLF and a lone CR both become
    // LF, so assigning "a\r\nb" to a control already holding "a\nb" changes nothing.
    if doc.tag_name(id).as_deref() == Some("textarea") {
        return normalize_newlines(raw);
    }
    if doc.tag_name(id).as_deref() != Some("input") {
        return raw.to_string();
    }
    let ty = doc
        .attribute(id, "type")
        .map(|t| t.cow_to_ascii_lowercase().into_owned())
        .unwrap_or_else(|| "text".to_string());

    // Every single-line control drops line breaks, whatever else its type does.
    let stripped: String = raw.chars().filter(|c| !matches!(c, '\n' | '\r')).collect();
    match ty.as_str() {
        "url" | "email" => stripped.trim().to_string(),
        "color" => {
            if is_simple_color(&stripped) {
                stripped.cow_to_ascii_lowercase().into_owned()
            } else {
                "#000000".to_string()
            }
        }
        "number" => match parse_number(&stripped) {
            Some(_) => stripped,
            None => String::new(),
        },
        "range" => {
            let min = doc.attribute(id, "min").and_then(|v| parse_number(&v)).unwrap_or(0.0);
            let max = doc.attribute(id, "max").and_then(|v| parse_number(&v)).unwrap_or(100.0);
            let value = parse_number(&stripped).unwrap_or((min + max) / 2.0);
            value.clamp(min, max.max(min)).to_string()
        }
        // A date or time value survives only if it is a conforming string of its own format.
        _ => match temporal::Kind::of(&ty) {
            Some(kind) if !temporal::is_valid(kind, &stripped) => String::new(),
            _ => stripped,
        },
    }
}

/// CRLF and lone CR both collapse to LF.
fn normalize_newlines(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            chars.next_if_eq(&'\n');
            out.push('\n');
            continue;
        }
        out.push(ch);
    }
    out
}

/// A "simple colour": `#` followed by exactly six ASCII hex digits.
fn is_simple_color(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('#') else {
        return false;
    };
    digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit())
}
/// The spec's "rules for parsing floating-point number values": no whitespace, no infinities
/// and no NaN, so `" 1"`, `"inf"` and `"1px"` are all failures.
pub fn parse_number(value: &str) -> Option<f64> {
    if value.is_empty() || value.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    if value
        .chars()
        .any(|c| c.is_ascii_alphabetic() && !matches!(c, 'e' | 'E'))
    {
        return None;
    }
    value.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// The value the control holds right now: what has been typed into it, else its markup value
/// sanitized for its type. **The** answer - nothing should compute its own.
pub fn current_value<H: ControlHost + ?Sized>(doc: &H, id: NodeId) -> String {
    match doc.control_edit_state(id) {
        Some(state) => state.value,
        None => sanitize_value(doc, id, &markup_value(doc, id)),
    }
}

/// Where a freshly created edit state puts its cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretStart {
    /// Where focus would leave it: the end of a single-line field, the top of a textarea.
    Focus,
    /// The beginning - what the IDL reports for a control nobody has touched.
    Idl,
}

/// The control's edit state, created from [`current_value`] when it has none yet.
///
/// The two starts are not a disagreement: clicking into a single-line field leaves the caret
/// at the end, while `selectionStart` on an untouched control is 0 whatever a click would
/// later do.
pub fn edit_state<H: ControlHost + ?Sized>(doc: &H, id: NodeId, start: CaretStart) -> ControlEditState {
    if let Some(state) = doc.control_edit_state(id) {
        return state;
    }
    let value = sanitize_value(doc, id, &markup_value(doc, id));
    let caret = match start {
        CaretStart::Idl => 0,
        CaretStart::Focus if doc.tag_name(id).as_deref() == Some("textarea") => 0,
        CaretStart::Focus => value.chars().count(),
    };
    ControlEditState::new(value, caret)
}

/// The real document offers the host view directly.
impl<C: gosub_interface::config::HasDocument<Document = Self>> ControlHost
    for crate::document::document_impl::DocumentImpl<C>
{
    fn tag_name(&self, id: NodeId) -> Option<String> {
        gosub_interface::document::Document::tag_name(self, id).map(str::to_string)
    }

    fn attribute(&self, id: NodeId, name: &str) -> Option<String> {
        gosub_interface::document::Document::attribute(self, id, name).map(str::to_string)
    }

    fn children(&self, id: NodeId) -> Vec<NodeId> {
        gosub_interface::document::Document::children(self, id).to_vec()
    }

    fn text_value(&self, id: NodeId) -> Option<String> {
        gosub_interface::document::Document::text_value(self, id).map(str::to_string)
    }

    fn control_edit_state(&self, id: NodeId) -> Option<ControlEditState> {
        gosub_interface::document::Document::control_edit_state(self, id)
    }
}

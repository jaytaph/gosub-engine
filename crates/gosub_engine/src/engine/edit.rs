//! Editing form controls: typing into text fields, toggling checkboxes/radios. The state lives
//! on the DOM document (`ControlEditState`, `is_checked`) where selectors and the painter read it.

use crate::html::{DomConfiguration, EngineDocument};
use cow_utils::CowUtils;
use gosub_html5::control::{self, CaretStart};
use gosub_html5::temporal;
use gosub_interface::document::{ControlEditState, Document as _, SelectionDirection};
use gosub_shared::node::NodeId;

/// The value layer lives next to the DOM (`gosub_html5::control`) so the layouter and
/// painter share one answer with the engine. These wrappers pin the config type, which the
/// bare functions cannot infer from `&C::Document` alone.
pub use gosub_html5::control::ValueMode;

/// The value mode of `id`, or `None` when it is not a control with a value at all.
pub fn value_mode<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<ValueMode> {
    control::value_mode(doc, id)
}

/// The markup value: the `value` attribute, or a `<textarea>`'s text content.
pub fn initial_value<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> String {
    control::markup_value(doc, id)
}

/// The value sanitization algorithm for `id`'s type.
pub fn sanitize_value<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, raw: &str) -> String {
    control::sanitize_value(doc, id, raw)
}

/// The spec's "rules for parsing floating-point number values".
pub use gosub_html5::control::parse_number;

/// The control's edit state as the IDL sees it: an untouched control's cursor is at 0.
fn edit_state<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> ControlEditState {
    control::edit_state(doc, id, CaretStart::Idl)
}

/// Whether `id` is disabled: its own `disabled` attribute, or an ancestor `<fieldset disabled>`.
/// Controls inside that fieldset's first `<legend>` escape it, which is how a disabled
/// fieldset keeps its own legend usable.
pub fn is_disabled<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    if doc.attribute(id, "disabled").is_some() {
        return true;
    }
    let mut child = id;
    while let Some(parent) = doc.parent(child) {
        if doc.tag_name(parent) == Some("fieldset") && doc.attribute(parent, "disabled").is_some() {
            let first_legend = doc
                .children(parent)
                .iter()
                .find(|&&c| doc.tag_name(c) == Some("legend"))
                .copied();
            if first_legend != Some(child) {
                return true;
            }
        }
        child = parent;
    }
    false
}

/// Whether the user could change this control: not disabled, and not read-only.
///
/// Only some constraints care - a *required* field that nobody can type into is not
/// "missing", but a value past its `max` is still past its `max`.
pub fn is_mutable<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    !is_disabled(doc, id) && doc.attribute(id, "readonly").is_none()
}

/// The `valueAsNumber` of a control, or `NaN` when its type has no numeric reading.
pub fn value_as_number<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> f64 {
    let Some(ty) = numeric_type(doc, id) else {
        return f64::NAN;
    };
    let value = api_value(doc, id);
    match temporal::Kind::of(&ty) {
        Some(kind) => temporal::parse(kind, &value).unwrap_or(f64::NAN),
        None => parse_number(&value).unwrap_or(f64::NAN),
    }
}

/// The value string a `valueAsNumber` assignment produces, or `None` when the type has no
/// numeric reading at all - which is what makes the IDL setter throw.
pub fn value_from_number<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, number: f64) -> Option<String> {
    let ty = numeric_type(doc, id)?;
    Some(match temporal::Kind::of(&ty) {
        // A number that lands outside the type's range leaves the control empty.
        Some(kind) => temporal::serialize(kind, number).unwrap_or_default(),
        None => format_number(number),
    })
}

/// The instant `valueAsDate` reports, or `None` when this control has no date.
pub fn value_as_date<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<f64> {
    let kind = temporal::Kind::of(&numeric_type(doc, id)?)?;
    temporal::to_instant(kind, temporal::parse(kind, &api_value(doc, id))?)
}

/// The value string an assignment to `valueAsDate` produces, or `None` when the control has
/// no date at all - which is what makes the setter throw.
pub fn value_from_date<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, instant: f64) -> Option<String> {
    let kind = temporal::Kind::of(&numeric_type(doc, id)?)?;
    // A local datetime names a wall-clock reading, not a moment, so it has no date either.
    temporal::to_instant(kind, 0.0)?;
    if !instant.is_finite() {
        return Some(String::new());
    }
    Some(
        temporal::from_instant(kind, instant)
            .and_then(|number| temporal::serialize(kind, number))
            .unwrap_or_default(),
    )
}

/// The `<input>` types whose value is a number underneath.
fn numeric_type<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<String> {
    if doc.tag_name(id) != Some("input") {
        return None;
    }
    let ty = doc
        .attribute(id, "type")
        .map(|t| t.cow_to_ascii_lowercase().into_owned())
        .unwrap_or_else(|| "text".to_string());
    matches!(
        ty.as_str(),
        "number" | "range" | "date" | "month" | "week" | "time" | "datetime-local"
    )
    .then_some(ty)
}

/// Apply the value-mode transition rules for an `<input>` whose `type` is changing.
///
/// The three modes disagree about where the value lives - live editing state, the `value`
/// content attribute, or nowhere at all - so changing type has to move it across before the
/// new type's sanitization runs on it.
pub fn change_type<C: DomConfiguration>(doc: &mut EngineDocument<C>, id: NodeId, new_type: &str) {
    if doc.tag_name(id) != Some("input") {
        return;
    }
    let before = value_mode(doc, id);
    let was_selectable = supports_selection(doc, id);
    let current = api_value(doc, id);
    doc.set_attribute(id, "type", new_type);
    let after = value_mode(doc, id);

    match (before, after) {
        // The value was live state and now lives in the attribute: write it across - but
        // only a non-empty one, so a checkbox arriving from an empty field still reports
        // its "on" default rather than an empty attribute.
        (Some(ValueMode::Value), Some(ValueMode::Default | ValueMode::DefaultOn)) => {
            if !current.is_empty() {
                doc.set_attribute(id, "value", &current);
            }
            doc.set_control_edit_state(id, None);
        }
        // The value now comes from the attribute again, so forget what was typed.
        (Some(ValueMode::Default | ValueMode::DefaultOn), Some(ValueMode::Value)) => {
            doc.set_control_edit_state(id, None);
        }
        // A file control holds no value a script can set.
        (_, Some(ValueMode::Filename)) => {
            doc.set_control_edit_state(id, None);
        }
        _ => {}
    }

    // A control that has just gained a selection API starts at the beginning, whatever
    // cursor position the previous type happened to leave behind.
    if !was_selectable && supports_selection(doc, id) {
        if let Some(mut state) = doc.control_edit_state(id) {
            state.caret = 0;
            state.anchor = None;
            state.direction = SelectionDirection::None;
            doc.set_control_edit_state(id, Some(state));
        }
    }
}

/// The value the IDL and constraint validation see: the live value, sanitized.
///
/// This is deliberately not what the painter reads - a half-typed `"1e"` in a number field
/// sanitizes to the empty string, and blanking the box mid-keystroke would be absurd.
pub fn api_value<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> String {
    match doc.tag_name(id) {
        Some("select") => doc
            .selected_option(id)
            .map(|option| crate::engine::form::option_value(doc, option))
            .unwrap_or_default(),
        Some("input" | "textarea") => {
            let live = crate::engine::form::live_value(doc, id);
            sanitize_value(doc, id, &live)
        }
        _ => String::new(),
    }
}

/// `Some(multiline)` when `id` is an enabled, writable text-entry control.
pub fn text_entry_kind<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<bool> {
    let tag = doc.tag_name(id)?;
    if is_disabled(doc, id) || doc.attribute(id, "readonly").is_some() {
        return None;
    }
    match tag {
        "textarea" => Some(true),
        "input" => {
            let ty = doc
                .attribute(id, "type")
                .map(|t| t.cow_to_ascii_lowercase().into_owned());
            let typed = matches!(
                ty.as_deref(),
                None | Some("text" | "password" | "search" | "email" | "url" | "tel" | "number")
            );
            typed.then_some(false)
        }
        _ => None,
    }
}

/// Why a `stepUp()`/`stepDown()` could not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepError {
    /// The control has no stepping behaviour (a text field, or a type the engine has no
    /// number conversion for).
    NotSteppable,
    /// `step="any"`, which the spec says has no allowed value step at all.
    StepAny,
}

/// The allowed value step of `id`, or why it has none.
pub fn allowed_step<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Result<f64, StepError> {
    let Some(ty) = numeric_type(doc, id) else {
        return Err(StepError::NotSteppable);
    };
    // A temporal step counts in its own unit: `step="2"` on a date is two days.
    let (default, scale) = match temporal::Kind::of(&ty) {
        Some(kind) => (kind.default_step(), kind.step_scale()),
        None => (1.0, 1.0),
    };
    match doc.attribute(id, "step").map(str::trim) {
        Some(s) if s.eq_ignore_ascii_case("any") => Err(StepError::StepAny),
        Some(s) => Ok(parse_number(s)
            .filter(|step| *step > 0.0)
            .map_or(default, |s| s * scale)),
        None => Ok(default),
    }
}

/// The `stepUp()`/`stepDown()` algorithm. `n` is negative for a step down. Returns the new
/// value, already clamped into `[min, max]` and aligned to the step base.
pub fn step<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, n: i64) -> Result<String, StepError> {
    let step = allowed_step(doc, id)?;
    let min = bound(doc, id, "min");
    let max = bound(doc, id, "max");
    let base = step_base(doc, id);
    // A range that cannot contain anything leaves the value alone.
    if let (Some(min), Some(max)) = (min, max) {
        if min > max {
            return Ok(value_from_number(doc, id, value_as_number(doc, id)).unwrap_or_default());
        }
    }
    // A value that will not convert counts as zero, rather than making the call fail.
    let value = value_as_number(doc, id);
    let value = if value.is_nan() { 0.0 } else { value };

    let offset = (value - base) / step;
    let aligned = (offset - offset.round()).abs() < 1e-9;
    let mut result = if aligned {
        value + (n as f64) * step
    } else if n > 0 {
        // Not on the grid: snapping to the next step in the direction asked IS the step.
        base + offset.ceil() * step
    } else {
        base + offset.floor() * step
    };

    if let Some(min) = min {
        result = result.max(min);
    }
    if let Some(max) = max {
        if result > max {
            // The largest value on the grid that still fits.
            result = base + ((max - base) / step).floor() * step;
        }
    }
    Ok(value_from_number(doc, id, result).unwrap_or_default())
}

/// Where the step grid starts: the `min` attribute, else the type's own default base.
pub fn step_base<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> f64 {
    if let Some(min) = bound(doc, id, "min") {
        return min;
    }
    numeric_type(doc, id)
        .as_deref()
        .and_then(temporal::Kind::of)
        .map_or(0.0, |kind| kind.default_step_base())
}

/// A `min`/`max` attribute read in whatever units the control's type counts in.
pub fn bound<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, name: &str) -> Option<f64> {
    let raw = doc.attribute(id, name)?;
    let ty = numeric_type(doc, id)?;
    match temporal::Kind::of(&ty) {
        Some(kind) => temporal::parse(kind, raw.trim()),
        None => parse_number(raw.trim()),
    }
}

/// Whether `id` exposes the text selection API. The types that do not (number, date,
/// checkbox, ...) report `null` selections and throw on `setSelectionRange`.
pub fn supports_selection<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    match doc.tag_name(id) {
        Some("textarea") => true,
        // Not email: `multiple` lets it hold a list, so it has no single selection.
        Some("input") => matches!(
            doc.attribute(id, "type")
                .map(|t| t.cow_to_ascii_lowercase().into_owned())
                .as_deref(),
            None | Some("text" | "search" | "url" | "tel" | "password")
        ),
        _ => false,
    }
}

/// `(selectionStart, selectionEnd, selectionDirection)` in char indices.
pub fn selection<C: DomConfiguration>(
    doc: &EngineDocument<C>,
    id: NodeId,
) -> Option<(usize, usize, SelectionDirection)> {
    if !supports_selection(doc, id) {
        return None;
    }
    let state = edit_state(doc, id);
    let (start, end) = state.selection().unwrap_or((state.caret, state.caret));
    Some((start, end, state.direction))
}

/// The `setSelectionRange()` algorithm: clamp both ends to the value, and put the caret at
/// the end the direction points to.
pub fn set_selection<C: DomConfiguration>(
    doc: &EngineDocument<C>,
    id: NodeId,
    start: usize,
    end: usize,
    direction: SelectionDirection,
) -> bool {
    if !supports_selection(doc, id) {
        return false;
    }
    let mut state = edit_state(doc, id);
    let len = state.value.chars().count();
    let end = end.min(len);
    let start = start.min(end);

    state.direction = direction;
    state.anchor = Some(if direction == SelectionDirection::Backward {
        end
    } else {
        start
    });
    state.caret = if direction == SelectionDirection::Backward {
        start
    } else {
        end
    };
    doc.set_control_edit_state(id, Some(state));
    true
}

/// How `setRangeText()` leaves the selection afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeTextMode {
    Select,
    Start,
    End,
    Preserve,
}

impl RangeTextMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "select" => Some(RangeTextMode::Select),
            "start" => Some(RangeTextMode::Start),
            "end" => Some(RangeTextMode::End),
            "preserve" => Some(RangeTextMode::Preserve),
            _ => None,
        }
    }
}

/// The `setRangeText()` algorithm: splice `replacement` into `[start, end)` and place the
/// selection according to `mode`.
pub fn set_range_text<C: DomConfiguration>(
    doc: &EngineDocument<C>,
    id: NodeId,
    replacement: &str,
    start: usize,
    end: usize,
    mode: RangeTextMode,
) -> bool {
    if !supports_selection(doc, id) {
        return false;
    }
    let mut state = edit_state(doc, id);
    let len = state.value.chars().count();
    let end = end.min(len);
    let start = start.min(end);

    let (old_start, old_end) = state.selection().unwrap_or((state.caret, state.caret));
    let prefix: String = state.value.chars().take(start).collect();
    let suffix: String = state.value.chars().skip(end).collect();
    let added = replacement.chars().count();
    state.value = format!("{prefix}{replacement}{suffix}");

    let (new_start, new_end) = match mode {
        RangeTextMode::Select => (start, start + added),
        RangeTextMode::Start => (start, start),
        RangeTextMode::End => (start + added, start + added),
        RangeTextMode::Preserve => {
            // The old selection slides by however much the replacement grew or shrank.
            let shift = |index: usize| {
                if index <= start {
                    index
                } else if index >= end {
                    index + added - (end - start)
                } else {
                    start + added
                }
            };
            (shift(old_start), shift(old_end))
        }
    };
    state.anchor = Some(new_start);
    state.caret = new_end;
    doc.set_control_edit_state(id, Some(state));
    true
}

/// Drop the characters a control refuses: `type=number` takes only what can be part of a number
/// (Chrome/Safari behaviour); everything else takes anything.
pub fn filter_insert<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, text: &str) -> String {
    let numeric = doc.tag_name(id) == Some("input")
        && doc
            .attribute(id, "type")
            .is_some_and(|t| t.eq_ignore_ascii_case("number"));
    if numeric {
        return text
            .chars()
            .filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
            .collect();
    }
    // Single-line controls strip line breaks (value sanitization), so a pasted paragraph
    // becomes one line.
    if doc.tag_name(id) != Some("textarea") {
        return text.chars().filter(|c| !matches!(c, '\n' | '\r')).collect();
    }
    text.to_string()
}

/// `Some(is_radio)` when `id` is an enabled checkbox or radio button.
pub fn toggle_kind<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<bool> {
    if doc.tag_name(id) != Some("input") || is_disabled(doc, id) {
        return None;
    }
    match doc
        .attribute(id, "type")
        .map(|t| t.cow_to_ascii_lowercase().into_owned())
        .as_deref()
    {
        Some("checkbox") => Some(false),
        Some("radio") => Some(true),
        _ => None,
    }
}

/// A checkbox flips; a radio becomes checked and the rest of its group (same `name` within the
/// nearest `<form>`, or the document) is unchecked. Returns the `(node, checked)` changes.
pub fn toggle<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Vec<(NodeId, bool)> {
    match toggle_kind(doc, id) {
        None => Vec::new(),
        Some(false) => vec![(id, !doc.is_checked(id))],
        Some(true) => {
            let mut changes = Vec::new();
            if !doc.is_checked(id) {
                changes.push((id, true));
            }
            let Some(name) = doc.attribute(id, "name").filter(|n| !n.is_empty()) else {
                return changes;
            };
            let scope = crate::engine::form::form_owner(doc, id).unwrap_or_else(|| doc.root());
            let mut stack: Vec<NodeId> = doc.children(scope).iter().rev().copied().collect();
            while let Some(n) = stack.pop() {
                if n != id
                    && toggle_kind(doc, n) == Some(true)
                    && doc.attribute(n, "name") == Some(name)
                    && doc.is_checked(n)
                {
                    changes.push((n, false));
                }
                stack.extend(doc.children(n).iter().rev());
            }
            changes
        }
    }
}

/// Where a caret motion goes. Row-based moves (up/down/page) need the visual rows, so the
/// browsing context resolves those into `Motion::To`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    /// Start of the text (`Home` in a single-line field, `Ctrl+Home` anywhere).
    Start,
    End,
    /// An absolute char index (mouse, row navigation).
    To(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditAction {
    Insert(String),
    Backspace,
    Delete,
    /// Delete to the previous / next word boundary (Ctrl+Backspace / Ctrl+Delete).
    DeleteWord {
        backwards: bool,
    },
    /// `extend` keeps the anchor (Shift), otherwise the selection collapses.
    Move {
        motion: Motion,
        extend: bool,
    },
    SelectAll,
}

/// Printable keys arrive as their character; Ctrl/Meta chords are shortcuts. Keys that need the
/// visual rows (`ArrowUp`/`ArrowDown`/`PageUp`/`PageDown`, `Home`/`End` in a textarea) are not
/// mapped here.
pub fn action_for_key(key: &str, multiline: bool, ctrl_or_meta: bool, shift: bool) -> Option<EditAction> {
    let mv = |motion| EditAction::Move { motion, extend: shift };
    if ctrl_or_meta {
        return Some(match key {
            "a" | "A" => EditAction::SelectAll,
            "ArrowLeft" => mv(Motion::WordLeft),
            "ArrowRight" => mv(Motion::WordRight),
            "Home" => mv(Motion::Start),
            "End" => mv(Motion::End),
            "Backspace" => EditAction::DeleteWord { backwards: true },
            "Delete" => EditAction::DeleteWord { backwards: false },
            _ => return None,
        });
    }
    Some(match key {
        "Backspace" => EditAction::Backspace,
        "Delete" => EditAction::Delete,
        "ArrowLeft" => mv(Motion::Left),
        "ArrowRight" => mv(Motion::Right),
        "Home" if !multiline => mv(Motion::Start),
        "End" if !multiline => mv(Motion::End),
        "ArrowUp" if !multiline => mv(Motion::Start),
        "ArrowDown" if !multiline => mv(Motion::End),
        "Enter" if multiline => EditAction::Insert("\n".to_string()),
        k if k.chars().count() == 1 && !k.chars().next().is_some_and(char::is_control) => {
            EditAction::Insert(k.to_string())
        }
        _ => return None,
    })
}

fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map_or(s.len(), |(b, _)| b)
}

/// Char index of the previous word start before `caret` (skip spaces, then the word).
pub fn word_left(text: &str, caret: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = caret.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// Char index of the end of the next word after `caret` (skip spaces, then the word), the
/// GTK/Linux convention (Windows would stop at the following word's start).
pub fn word_right(text: &str, caret: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = caret.min(chars.len());
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// The word around char index `at`: a run of non-space chars, or the run of spaces itself.
pub fn word_at(text: &str, at: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let at = at.min(chars.len() - 1);
    let space = chars[at].is_whitespace();
    let (mut s, mut e) = (at, at + 1);
    while s > 0 && chars[s - 1].is_whitespace() == space {
        s -= 1;
    }
    while e < chars.len() && chars[e].is_whitespace() == space {
        e += 1;
    }
    (s, e)
}

/// Replace the selection (or `[caret, caret)`) with `text`. Returns whether anything changed.
fn replace_selection(state: &mut ControlEditState, text: &str) -> bool {
    let (start, end) = state.selection().unwrap_or((state.caret, state.caret));
    if start == end && text.is_empty() {
        return false;
    }
    let (bs, be) = (byte_at(&state.value, start), byte_at(&state.value, end));
    state.value.replace_range(bs..be, text);
    state.caret = start + text.chars().count();
    state.anchor = None;
    true
}

/// Returns whether anything changed. Indices are clamped to the value first.
pub fn apply(state: &mut ControlEditState, action: &EditAction) -> bool {
    // This is the user editing path - the IDL setters never come through here, which is
    // exactly the distinction `user_edited` records.
    state.user_edited = true;
    let len = state.value.chars().count();
    state.caret = state.caret.min(len);
    state.anchor = state.anchor.map(|a| a.min(len)).filter(|a| *a != state.caret);
    match action {
        EditAction::Insert(text) => replace_selection(state, text),
        EditAction::Backspace => {
            if state.selection().is_none() {
                if state.caret == 0 {
                    return false;
                }
                state.anchor = Some(state.caret - 1);
            }
            replace_selection(state, "")
        }
        EditAction::Delete => {
            if state.selection().is_none() {
                if state.caret >= len {
                    return false;
                }
                state.anchor = Some(state.caret + 1);
            }
            replace_selection(state, "")
        }
        EditAction::DeleteWord { backwards } => {
            if state.selection().is_none() {
                let to = if *backwards {
                    word_left(&state.value, state.caret)
                } else {
                    word_right(&state.value, state.caret)
                };
                if to == state.caret {
                    return false;
                }
                state.anchor = Some(to);
            }
            replace_selection(state, "")
        }
        EditAction::Move { motion, extend } => {
            let before = (state.caret, state.selection());
            let sel = state.selection();
            let target = match motion {
                // Without Shift, Left/Right on a selection collapse it to its edge.
                Motion::Left => match sel {
                    Some((s, _)) if !extend => s,
                    _ => state.caret.saturating_sub(1),
                },
                Motion::Right => match sel {
                    Some((_, e)) if !extend => e,
                    _ => (state.caret + 1).min(len),
                },
                Motion::WordLeft => word_left(&state.value, state.caret),
                Motion::WordRight => word_right(&state.value, state.caret),
                Motion::Start => 0,
                Motion::End => len,
                Motion::To(i) => (*i).min(len),
            };
            if *extend {
                state.anchor = state.anchor.or(Some(state.caret));
            } else {
                state.anchor = None;
            }
            state.caret = target;
            state.anchor = state.anchor.filter(|a| *a != state.caret);
            (state.caret, state.selection()) != before
        }
        EditAction::SelectAll => {
            let before = (state.caret, state.anchor);
            state.anchor = (len > 0).then_some(0);
            state.caret = len;
            (state.caret, state.anchor) != before
        }
    }
}

/// `(min, max, step)` of an enabled `<input type=range>`. `step="any"` → a fine step.
pub fn range_params<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> Option<(f64, f64, f64)> {
    if doc.tag_name(id) != Some("input")
        || is_disabled(doc, id)
        || !doc
            .attribute(id, "type")
            .is_some_and(|t| t.eq_ignore_ascii_case("range"))
    {
        return None;
    }
    let num = |name: &str| doc.attribute(id, name).and_then(|v| v.trim().parse::<f64>().ok());
    let min = num("min").unwrap_or(0.0);
    let max = num("max").unwrap_or(100.0).max(min);
    let step = match doc.attribute(id, "step") {
        Some(s) if s.trim().eq_ignore_ascii_case("any") => (max - min) / 1000.0,
        _ => num("step").filter(|s| *s > 0.0).unwrap_or(1.0),
    };
    Some((min, max, step.max(f64::EPSILON)))
}

/// The slider's current value: what the user dragged to, else the `value` attribute, else the
/// midpoint (HTML's default for range).
pub fn range_value<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId, min: f64, max: f64) -> f64 {
    doc.control_edit_state(id)
        .and_then(|s| s.value.trim().parse::<f64>().ok())
        .or_else(|| doc.attribute(id, "value").and_then(|v| v.trim().parse::<f64>().ok()))
        .unwrap_or((min + max) / 2.0)
        .clamp(min, max)
}

/// Snap `raw` to the step grid anchored at `min`, clamped to the range.
pub fn range_snap(min: f64, max: f64, step: f64, raw: f64) -> f64 {
    let v = min + ((raw - min) / step).round() * step;
    v.clamp(min, max)
}

/// Shortest decimal form: `42`, `7.5`, `0.125`.
pub fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// `id` is an enabled `<select>`.
pub fn is_select<C: DomConfiguration>(doc: &EngineDocument<C>, id: NodeId) -> bool {
    doc.tag_name(id) == Some("select") && !is_disabled(doc, id)
}

/// The enabled options of a `<select>`, in order, through `<optgroup>`s.
pub fn select_options<C: DomConfiguration>(doc: &EngineDocument<C>, select: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = doc.children(select).iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        match doc.tag_name(id) {
            Some("option") if doc.attribute(id, "disabled").is_none() => out.push(id),
            Some("optgroup") => stack.extend(doc.children(id).iter().rev()),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected states are compared against ones that have been through `apply`, which marks
    /// them as user-edited, so build them the same way.
    fn st(v: &str, caret: usize) -> ControlEditState {
        ControlEditState {
            user_edited: true,
            ..ControlEditState::new(v.to_string(), caret)
        }
    }

    fn sel(v: &str, anchor: usize, caret: usize) -> ControlEditState {
        ControlEditState {
            anchor: Some(anchor),
            ..st(v, caret)
        }
    }

    fn mv(motion: Motion, extend: bool) -> EditAction {
        EditAction::Move { motion, extend }
    }

    #[test]
    fn insert_and_delete_are_char_based() {
        let mut s = st("héllo", 2);
        assert!(apply(&mut s, &EditAction::Insert("X".into())));
        assert_eq!(s, st("héXllo", 3));
        assert!(apply(&mut s, &EditAction::Backspace));
        assert_eq!(s, st("héllo", 2));
        assert!(apply(&mut s, &EditAction::Backspace));
        assert_eq!(s, st("hllo", 1));
        assert!(apply(&mut s, &EditAction::Delete));
        assert_eq!(s, st("hlo", 1));
    }

    #[test]
    fn caret_movement_clamps() {
        let mut s = st("ab", 0);
        assert!(!apply(&mut s, &mv(Motion::Left, false)));
        assert!(apply(&mut s, &mv(Motion::End, false)));
        assert_eq!(s.caret, 2);
        assert!(!apply(&mut s, &mv(Motion::Right, false)));
        assert!(!apply(&mut s, &EditAction::Delete));
        assert!(apply(&mut s, &mv(Motion::Start, false)));
        assert_eq!(s.caret, 0);
    }

    #[test]
    fn shift_extends_and_plain_moves_collapse() {
        let mut s = st("hello world", 5);
        assert!(apply(&mut s, &mv(Motion::Left, true)));
        assert!(apply(&mut s, &mv(Motion::Left, true)));
        assert_eq!(s.selection(), Some((3, 5)));
        assert_eq!(s.caret, 3);
        // Plain Right collapses to the selection's end, not caret+1.
        assert!(apply(&mut s, &mv(Motion::Right, false)));
        assert_eq!((s.caret, s.selection()), (5, None));
        assert!(apply(&mut s, &mv(Motion::WordRight, true)));
        assert_eq!(s.selection(), Some((5, 11)));
        // Extending back across the anchor flips the selection side.
        assert!(apply(&mut s, &mv(Motion::Start, true)));
        assert_eq!(s.selection(), Some((0, 5)));
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut s = sel("hello world", 0, 5);
        assert!(apply(&mut s, &EditAction::Insert("bye".into())));
        assert_eq!(s, st("bye world", 3));
        let mut s = sel("hello world", 11, 6);
        assert!(apply(&mut s, &EditAction::Backspace));
        assert_eq!(s, st("hello ", 6));
        let mut s = sel("abc", 1, 2);
        assert!(apply(&mut s, &EditAction::Delete));
        assert_eq!(s, st("ac", 1));
    }

    #[test]
    fn select_all_and_word_deletes() {
        let mut s = st("one two  three", 14);
        assert!(apply(&mut s, &EditAction::SelectAll));
        assert_eq!(s.selection(), Some((0, 14)));
        let mut s = st("one two  three", 14);
        assert!(apply(&mut s, &EditAction::DeleteWord { backwards: true }));
        assert_eq!(s, st("one two  ", 9));
        assert!(apply(&mut s, &EditAction::DeleteWord { backwards: true }));
        assert_eq!(s, st("one ", 4));
        let mut s = st("one two", 0);
        assert!(apply(&mut s, &EditAction::DeleteWord { backwards: false }));
        assert_eq!(s, st(" two", 0));
        assert!(apply(&mut s, &EditAction::DeleteWord { backwards: false }));
        assert_eq!(s, st("", 0));
        assert_eq!(word_at("hello big world", 7), (6, 9));
        assert_eq!(word_at("a  b", 1), (1, 3));
    }

    #[test]
    fn key_mapping() {
        assert_eq!(
            action_for_key("a", false, false, false),
            Some(EditAction::Insert("a".into()))
        );
        assert_eq!(
            action_for_key(" ", false, false, false),
            Some(EditAction::Insert(" ".into()))
        );
        assert_eq!(action_for_key("Enter", false, false, false), None);
        assert_eq!(
            action_for_key("Enter", true, false, false),
            Some(EditAction::Insert("\n".into()))
        );
        assert_eq!(action_for_key("a", false, true, false), Some(EditAction::SelectAll));
        assert_eq!(action_for_key("v", false, true, false), None);
        assert_eq!(action_for_key("Shift", false, false, false), None);
        assert_eq!(
            action_for_key("ArrowLeft", false, true, true),
            Some(mv(Motion::WordLeft, true))
        );
        // Home/End/Up/Down in a textarea are row-based and resolved by the context.
        assert_eq!(action_for_key("Home", true, false, false), None);
        assert_eq!(
            action_for_key("Home", false, false, false),
            Some(mv(Motion::Start, false))
        );
    }
}

# Running web-platform-tests

The engine has no scripting environment yet, so most of the WPT suites that matter for form
controls — `html/semantics/forms/`, ~427 `testharness.js` files — cannot run against it.
`gosub_domjs` is a stopgap: a **test-only** DOM binding over a small JavaScript engine
(QuickJS, through `rquickjs`), enough to let those tests drive the engine's own DOM.

It exists to find bugs, not to run websites.

## Setup

The checkout is pinned: `tests/wpt/wpt-commit.txt` holds the commit CI uses, and results are
only comparable against that one.

```bash
git clone --filter=blob:none --sparse https://github.com/web-platform-tests/wpt.git
cd wpt
git sparse-checkout set resources html/semantics/forms
git checkout "$(cat …/tests/wpt/wpt-commit.txt)"
```

```bash
cargo run -p gosub-wpt -- <wpt-root> <test.html>... [-v]
```

Paths are taken relative to the wpt root when they are not found as given. The exit code is
non-zero if any subtest failed.

## The expectations file

`tests/wpt/forms-expectations.txt` is the committed baseline: which suites are covered, and
which subtests are known to fail. Four record types - `FILE`, `FAIL <path> :: <name>`,
`HARNESS` (the harness itself did not finish cleanly) and `ERROR` (the suite cannot run at
all, usually a support file outside the sparse checkout).

Files are listed explicitly rather than globbed, so adding tests to a wpt checkout cannot
silently change what is covered.

```bash
cargo run -p gosub-wpt -- <wpt-root> --all --expect tests/wpt/forms-expectations.txt
```

That is what `cargo test -p gosub-wpt --test wpt_conformance` runs when `WPT_ROOT` is set,
and what the `wpt-forms` CI job runs at the pinned commit. Without `WPT_ROOT` the test skips,
so an ordinary `cargo test` needs no checkout.

A listed test that starts passing is an **UNEXPECTED PASS** and fails the run. That is
deliberate: improving behaviour is supposed to make you regenerate the baseline and commit
the diff, so the file always says what the engine actually does.

```bash
cargo run --release -p gosub-wpt -- "$WPT_ROOT" --write-expectations $(paths...) \
    > tests/wpt/forms-expectations.txt
```

Diagnostics (console output, listener and timer exceptions, scripts that threw) go to
stderr; only results go to stdout, so regenerating never picks up stray lines.

## Sloppy mode

Page scripts are evaluated **non-strict**, the way a browser runs them. rquickjs defaults to
strict, which was wrong in two ways that both read as engine failures: assigning to an
accessor with no setter throws instead of silently doing nothing, and assigning to an
undeclared name throws `ReferenceError` instead of creating a global. Use
`gosub_domjs::eval_script` rather than `Ctx::eval` for anything that came out of a page.

## The one rule

**The bindings hold no DOM logic.** Every property reads or writes the real document, so a
passing test says something about the engine rather than about the binding layer. When a
test needs behaviour the engine does not have, the fix belongs in engine code — never in a
shim that makes the test go green.

## How a test runs

1. The file is parsed into a `DocumentImpl` by `gosub_html5`.
2. A fresh QuickJS context gets `self`, then wpt's own `testharness.js`.
3. `document` is installed **after** testharness.js. testharness picks its environment by
   looking for `document` on the global scope; without one it uses the shell environment,
   which needs no window, no load event and no result-reporting DOM. Installing `document`
   afterwards keeps it in that mode while still giving tests a DOM.
4. Every `<script>` in the document runs in tree order (`testharness.js` and
   `testharnessreport.js` are skipped — the driver loads the first and replaces the second).
   Microtasks are drained after each one.
5. The driver calls `done()`, then pumps the timer queue until the harness reports or the
   queue runs dry.
6. If the queue drains and nothing has reported, the driver calls testharness's `timeout()`.
   The shell environment has no default timeout, so an async test whose event never arrives
   would otherwise hang the run forever; this turns it into a TIMEOUT result instead.
7. Results come out of an `add_completion_callback` hook.

## Timers

There is no clock. `setTimeout`, `setInterval`, `requestAnimationFrame` and their cancel
functions all feed one queue ordered by due time and then insertion order, and firing a
callback advances a virtual "now" to that callback's due time. Nothing waits on wall-clock
time, and a test that schedules a 10-second timeout costs nothing to run.

`requestAnimationFrame` resolves one frame (16ms of virtual time) later and passes a
timestamp. Nothing paints — it is the delay that matters, since 57 of the forms tests use
rAF purely to wait a turn.

testharness passes `null` where a delay or a timer id is expected, which is not the same as
omitting the argument, so both are taken as raw values and coerced.

## Events

`addEventListener`/`removeEventListener`/`dispatchEvent` are on nodes, on `document` and on
the global object; `Event` is constructible with `bubbles`/`cancelable`/`composed`. Dispatch
implements capture → at-target → bubble over the **real document tree**, with
`stopPropagation`, `stopImmediatePropagation`, `preventDefault`, the `once` and `capture`
listener options, and the spec's dedup rule (same type + callback + capture is ignored).

`element.click()` fires a click event but has **no activation behaviour**: a checkbox does
not toggle and a submit button does not submit, because that lives in `gosub_engine`'s
private `edit`/`form` modules. Tests that click and then wait for the resulting change now
report TIMEOUT rather than hanging.

Removed listeners are tombstoned rather than deleted, because dispatch holds indices into
the listener list and has to observe removals made by listeners that run before them.

## What is bound

`document`: `getElementById`, `createElement`, `createTextNode`, `querySelector`,
`getElementsByTagName`, `body`, `head`, `documentElement`.

`Node` also carries `addEventListener`, `removeEventListener`, `dispatchEvent` and `click`.

Reflected IDL attributes come in getter/setter **pairs**. A getter on its own is worse than
no binding at all: in a page script the assignment is a silent no-op, so the test carries on
and fails far away from the line that actually did nothing. Bound today: `id`, `className`,
`name`, `placeholder`, `pattern`, `min`, `max`, `step`, `accept`, `autocomplete`, `action`,
`method`, `htmlFor`, `disabled`, `required`, `readOnly`, `multiple`, `autofocus`,
`noValidate`, `maxLength`, `minLength`, `size`, `rows`, `cols`, `type`.

Live control state is read and written through the engine, never mirrored in the binding:
`value` (via `edit::value_mode` and `form::live_value`), `checked`, `defaultValue`,
`defaultChecked`, `selected`, `defaultSelected`, `selectedIndex`, `options`,
`selectedOptions`, `text`, `label`, `textLength`, and `form` (via `form::form_owner`).

`Node`: `nodeType`, `nodeName`, `tagName`, `localName`, `parentNode`, `parentElement`,
`childNodes`, `children`, `firstChild`, `appendChild`, `removeChild`, `remove`,
`hasChildNodes`, `get`/`set`/`remove`/`hasAttribute`, `getAttributeNS`/`setAttributeNS`,
`id`, `className`, `textContent`, `outerHTML`, `querySelector`, `getElementsByTagName`, and
the option/textarea reflections `value`, `label`, `text`, `type`.

Node wrappers are cached per node, so `a.parentNode === b` holds.

## Markup, handlers and activation

`innerHTML` reads and writes through `gosub_html5::document::inner_html`. The setter parses
in the target's context and moves the result in - `parse_fragment` builds into a fresh
`<html>` it hangs off the document root, so that scaffolding is moved across and discarded.

`onclick`, `oninput`, `onchange`, `onselect`, `onsubmit`, `onreset` and `oninvalid` register
a listener that assigning again replaces, without disturbing anything added through
`addEventListener`. `document.createEvent()` + `initEvent()` cover the legacy path.

`click()` now runs the activation behaviour behind the event: a checkbox toggles, a submit
button submits, a reset button resets - decided by the engine's own `form::button_kind` and
`edit::toggle_kind`, not by a second classifier here. A disabled control does nothing at
all, not even dispatch, and a listener calling `preventDefault()` cancels the activation.

## Gauges and named access

`<meter>`'s `value`/`min`/`max`/`low`/`high`/`optimum` and `<progress>`'s `value`/`max`/
`position` resolve through `gosub_engine::gauge`, which applies the spec's defaults and
clamps in order - `high` is clamped against the already-clamped `low`, not the raw
attribute. The getters report resolved numbers while the setters store the raw one, so a
`low` above `max` keeps its attribute and reads back clamped. Setting any of them to
something that is not a finite number throws `TypeError`, and a `<progress>` ignores a
maximum that is not positive.

An element with `id="foo"` is reachable as the global `foo`. Real named access is a live
lookup on the window; this is a snapshot taken once after parsing, which is enough because
the harness runs every script afterwards. Names that would shadow an existing global - or a
testharness function - are skipped.

## Form submission and stepping

`submit()`, `requestSubmit()` and `reset()` are bound, but there is no navigation here: what
they exercise is the *difference* between them. `submit()` fires nothing and validates
nothing; `requestSubmit()` validates first (firing `invalid` at each failing control) and
then fires a cancelable `submit`; `reset()` fires a cancelable `reset` and, unless that is
cancelled, calls `form::reset` - the same code a reset button runs.

`stepUp()`/`stepDown()` use `edit::step`, which implements the spec's snap-then-clamp: a
value already on the step grid moves by n steps, a value off the grid snaps to the next one
in that direction, and the result is clamped to `[min, max]` on the grid. `number` and
`range` only; anything else, including `step="any"`, throws `InvalidStateError`.

## Selection, focus and exceptions

`selectionStart`, `selectionEnd`, `selectionDirection`, `setSelectionRange()`, `select()` and
`setRangeText()` are bound to `gosub_engine::edit`, which operates on the same
`ControlEditState` the painter and the keyboard path use. A control that has no selection API
reports `null` (not `undefined` - tests compare against null) and throws on
`setSelectionRange`.

Changing the selection queues a `select` event: only when the selection actually moves, never
synchronously, and coalesced so several changes in one turn produce one event. Scheduling
goes through the page's own `setTimeout`, so it lands in the same virtual-time queue as
everything else. An untouched control reports its selection at 0 - the caret-at-the-end
default is what *focus* does, not what the IDL reports.

`focus()`/`blur()` and `document.activeElement` read and write the document's focus state,
gated by `focus::focusability`, so an unfocusable element quietly stays unfocused. No
scrolling and no focus ring: those are paint concerns and nothing here paints.

`DOMException` is a real class with `name`, `message` and the legacy `code` table.
`assert_throws_dom` checks all three, so a plain `Error` fails a test the engine actually got
right.

## Changing an input's type

`edit::change_type` implements the value-mode transition rules. The three modes disagree
about where a value lives - live editing state, the `value` content attribute, or nowhere -
so changing type moves it across before the new type's sanitization runs on it. Two details
the tests are strict about: an empty value is *not* written into the attribute (so a
checkbox arriving from an emptied field still reports its `"on"` default), and a control
that has just gained a selection API starts at offset 0 whatever cursor the previous type
left behind.

Sanitization covers the temporal types through `gosub_engine::temporal` and `color` (anything that is not `#` plus six hex digits becomes `#000000`). A `file`
control throws `InvalidStateError` for any value but the empty string.

## Date and time types

`gosub_engine::temporal` parses and serialises `date`, `month`, `week`, `time` and
`datetime-local`, and converts each to the number `valueAsNumber` reports - milliseconds
since the epoch for most, months since 1970-01 for a month, milliseconds into the day for a
time. Sanitization, stepping and the range constraints all read through it.

Three things the formats disagree about, and each one is a test that fails if you assume
otherwise: a `time` **wraps** into its day when a number is assigned (any millisecond count
lands somewhere between 00:00 and 24:00) while a `datetime-local` that lands outside a real
date just goes empty; the default step is a **minute** for `time` and `datetime-local`, not
a second; and a `week` counts its steps from **1969-12-29**, because 1970-01-01 was a
Thursday and week steps have to land on Mondays.

## Constraint validation

`willValidate`, `validity`, `validationMessage`, `checkValidity()`, `reportValidity()` and
`setCustomValidity()` are bound to `gosub_engine::validity`. `ValidityState` re-reads the
document on every getter, so it stays live: holding on to `input.validity` and then changing
the value reports the new state.

Implemented: `valueMissing` (including radio groups and a `<select>`'s placeholder label
option), `typeMismatch` for `email`/`url`, anchored `patternMismatch`, `tooLong`/`tooShort`
(only once the value is dirty), and `rangeUnderflow`/`rangeOverflow`/`stepMismatch` for
`number` and `range`. Not implemented: the date and time types, and `badInput` - the engine
has no type-specific editor that can hold an unconvertible value.

`reportValidity()` is `checkValidity()`: there is no UI to show a message in.

Two rules that are easy to get backwards. **Only `valueMissing` asks whether the control is
mutable** - a required field nobody can type into is not "missing", but a value past its
`max` is still past its `max` even on a disabled control. (Candidacy is a separate thing:
`checkValidity()` returns true for a barred control whatever its flags say.) And
`maxlength`/`minlength` constrain **what a person typed**, not what script assigned:
`ControlEditState::user_edited` records that difference, set only on the engine's editing
path, so assigning a long string to `value` never makes a control too long.

## What is not

- **No activation behaviour** behind `click()`, and no `focus()`/`blur()`/`activeElement`
  (288 uses in the forms corpus) — both need engine code that is not public yet.
- **No `CustomEvent`, `MouseEvent` or `KeyboardEvent`** constructors, and no `EventTarget`
  constructor. The forms corpus never uses the first; it uses the mouse and keyboard ones in
  13 files.
- **No interface hierarchy.** One `Node` class dispatches on tag name, so `instanceof`,
  `Option`, `NodeList` and prototype-chain tests fail.
- **No layout and no navigation**, so iframes, `getBoundingClientRect` and form submission
  are out of reach.
- **Scripts run after parsing**, not during it, so document.write and parser-timing tests
  are meaningless here.
- `querySelector` handles a single compound selector (`tag`, `#id`, `.class` and
  combinations) and throws on anything else, rather than silently mismatching.
- The document has no attribute namespaces; `setAttributeNS` parks the value under a key no
  HTML attribute name can produce, which keeps it out of the reflection path.
- `appendChild` cannot throw `HierarchyRequestError` properly — the document refuses to
  build a cycle instead of raising, so the binding turns that refusal into a plain error.

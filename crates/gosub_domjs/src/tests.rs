//! Smoke tests: the bindings must read the real document, not a JS-side copy.

use rquickjs::{CatchResultExt, Context, Runtime};

use crate::timers::{TimerState, Timers};
use crate::{install, parse_document};

/// Run `script`, drain the timer queue, then read `after` back out - the timer tests need to
/// observe state that only exists once the queue has been pumped.
fn eval_after_timers(html: &str, script: &str, after: &str) -> String {
    let (doc, _) = parse_document(html, None).expect("parse");
    let runtime = Runtime::new().expect("runtime");
    let context = Context::full(&runtime).expect("context");
    let timers: Timers = std::rc::Rc::new(std::cell::RefCell::new(TimerState::default()));
    context.with(|ctx| {
        install(&ctx, doc, &timers).expect("install");
        crate::eval_script::<()>(&ctx, script.as_bytes())
            .catch(&ctx)
            .expect("eval");
        while crate::timers::run_next(&ctx, &timers).expect("timers") {}
        crate::eval_script::<String>(&ctx, after.as_bytes())
            .catch(&ctx)
            .expect("read back")
    })
}

fn eval(html: &str, script: &str) -> String {
    let (doc, _) = parse_document(html, None).expect("parse");
    let runtime = Runtime::new().expect("runtime");
    let context = Context::full(&runtime).expect("context");
    let timers: Timers = std::rc::Rc::new(std::cell::RefCell::new(TimerState::default()));
    context.with(|ctx| {
        install(&ctx, doc, &timers).expect("install");
        let result = crate::eval_script::<String>(&ctx, script.as_bytes())
            .catch(&ctx)
            .expect("eval");
        while crate::timers::run_next(&ctx, &timers).expect("timers") {}
        result
    })
}

#[test]
fn reads_parsed_markup() {
    let value = eval(
        "<select><option value=a>First</option><option>Second</option></select>",
        "document.getElementsByTagName('option')[1].value",
    );
    assert_eq!(value, "Second");
}

#[test]
fn option_value_falls_back_to_stripped_and_collapsed_text() {
    let value = eval(
        "<option> child  node </option>",
        "document.querySelector('option').value",
    );
    assert_eq!(value, "child node");
}

#[test]
fn mutations_land_in_the_document() {
    let html = eval(
        "<body><p id=target></p></body>",
        "const p = document.getElementById('target');
         p.appendChild(document.createTextNode('hi'));
         p.setAttribute('data-x', '1');
         p.outerHTML",
    );
    assert!(html.contains("hi"), "{html}");
    assert!(html.contains("data-x"), "{html}");
}

#[test]
fn node_wrappers_have_stable_identity() {
    let same = eval(
        "<div id=parent><span id=child></span></div>",
        "String(document.getElementById('child').parentNode === document.getElementById('parent'))",
    );
    assert_eq!(same, "true");
}

#[test]
fn listeners_run_in_capture_target_bubble_order() {
    let order = eval(
        "<div id=outer><span id=inner></span></div>",
        "const seen = [];
         const outer = document.getElementById('outer');
         const inner = document.getElementById('inner');
         outer.addEventListener('x', () => seen.push('capture'), true);
         outer.addEventListener('x', () => seen.push('bubble'));
         inner.addEventListener('x', e => seen.push('target' + e.eventPhase));
         inner.dispatchEvent(new Event('x', {bubbles: true}));
         seen.join(',')",
    );
    assert_eq!(order, "capture,target2,bubble");
}

#[test]
fn duplicate_listeners_are_ignored_and_once_runs_once() {
    let count = eval(
        "<span id=el></span>",
        "let n = 0;
         const el = document.getElementById('el');
         const handler = () => n++;
         el.addEventListener('y', handler, {once: true});
         el.addEventListener('y', handler, {once: true});
         el.dispatchEvent(new Event('y'));
         el.dispatchEvent(new Event('y'));
         String(n)",
    );
    assert_eq!(count, "1");
}

#[test]
fn prevent_default_is_reported_by_dispatch_event() {
    let result = eval(
        "<span id=el></span>",
        "const el = document.getElementById('el');
         el.addEventListener('z', e => e.preventDefault());
         String(el.dispatchEvent(new Event('z', {cancelable: true})))",
    );
    assert_eq!(result, "false");
}

#[test]
fn stop_propagation_keeps_an_event_from_the_parent() {
    let seen = eval(
        "<div id=outer><span id=inner></span></div>",
        "const seen = [];
         document.getElementById('outer').addEventListener('s', () => seen.push('outer'));
         const inner = document.getElementById('inner');
         inner.addEventListener('s', e => { seen.push('inner'); e.stopPropagation(); });
         inner.dispatchEvent(new Event('s', {bubbles: true}));
         seen.join(',')",
    );
    assert_eq!(seen, "inner");
}

#[test]
fn timers_fire_in_due_order_not_registration_order() {
    let order = eval_after_timers(
        "<span></span>",
        "globalThis.seen = [];
         setTimeout(() => seen.push('late'), 50);
         setTimeout(() => seen.push('early'), 1);
         setTimeout(() => seen.push('same-time-second'), 1);",
        "seen.join(',')",
    );
    assert_eq!(order, "early,same-time-second,late");
}

#[test]
fn a_cleared_timer_never_runs() {
    let ran = eval_after_timers(
        "<span></span>",
        "globalThis.ran = false;
         const id = setTimeout(() => { ran = true; }, 5);
         clearTimeout(id);",
        "String(ran)",
    );
    assert_eq!(ran, "false");
}

#[test]
fn request_animation_frame_delivers_a_timestamp() {
    let kind = eval_after_timers(
        "<span></span>",
        "globalThis.kind = 'never ran';
         requestAnimationFrame(ts => { kind = typeof ts; });",
        "kind",
    );
    assert_eq!(kind, "number");
}

#[test]
fn click_reaches_a_listener_on_an_ancestor() {
    let seen = eval(
        "<form id=f><button id=b>go</button></form>",
        "const seen = [];
         document.getElementById('f').addEventListener('click', e => seen.push(e.target.tagName));
         document.getElementById('b').click();
         seen.join(',')",
    );
    assert_eq!(seen, "BUTTON");
}

#[test]
fn setting_value_does_not_disturb_the_default() {
    let result = eval(
        "<input id=i value=start>",
        "const i = document.getElementById('i');
         i.value = 'typed';
         [i.value, i.defaultValue, i.getAttribute('value')].join('|')",
    );
    assert_eq!(result, "typed|start|start");
}

#[test]
fn checked_is_live_state_and_the_attribute_is_the_default() {
    let result = eval(
        "<input id=c type=checkbox>",
        "const c = document.getElementById('c');
         c.checked = true;
         [c.checked, c.defaultChecked, c.hasAttribute('checked')].join('|')",
    );
    assert_eq!(result, "true|false|false");
}

#[test]
fn select_value_and_selected_index_agree() {
    let result = eval(
        "<select id=s><option value=a>A<option value=b>B</select>",
        "const s = document.getElementById('s');
         s.value = 'b';
         const first = s.selectedIndex;
         s.selectedIndex = 0;
         [first, s.value, s.options.length].join('|')",
    );
    assert_eq!(result, "1|a|2");
}

#[test]
fn clearing_a_boolean_property_removes_the_attribute() {
    let result = eval(
        "<input id=i disabled>",
        "const i = document.getElementById('i');
         const before = i.disabled;
         i.disabled = false;
         [before, i.disabled, i.hasAttribute('disabled')].join('|')",
    );
    assert_eq!(result, "true|false|false");
}

#[test]
fn form_owner_follows_the_form_attribute_out_of_the_tree() {
    let result = eval(
        "<form id=f></form><input id=i form=f>",
        "String(document.getElementById('i').form === document.getElementById('f'))",
    );
    assert_eq!(result, "true");
}

#[test]
fn validity_state_is_live() {
    let result = eval(
        "<input id=i required>",
        "const i = document.getElementById('i');
         const v = i.validity;
         const missing = v.valueMissing;
         i.value = 'x';
         [missing, v.valueMissing, v.valid, i.checkValidity()].join('|')",
    );
    assert_eq!(result, "true|false|true|true");
}

#[test]
fn barred_controls_never_fail_validation() {
    let result = eval(
        "<input id=h type=hidden required><fieldset disabled><input id=f required></fieldset>",
        "const h = document.getElementById('h'), f = document.getElementById('f');
         [h.willValidate, f.willValidate, h.checkValidity(), f.checkValidity()].join('|')",
    );
    assert_eq!(result, "false|false|true|true");
}

#[test]
fn custom_validity_wins_over_the_generic_message() {
    let result = eval(
        "<input id=i required>",
        "const i = document.getElementById('i');
         const generic = i.validationMessage.length > 0;
         i.setCustomValidity('nope');
         [generic, i.validity.customError, i.validationMessage].join('|')",
    );
    assert_eq!(result, "true|true|nope");
}

#[test]
fn setting_value_runs_the_sanitization_algorithm() {
    let result = eval(
        "<input id=n type=number><input id=t>",
        "const n = document.getElementById('n'), t = document.getElementById('t');
         n.value = 'not a number';
         t.value = 'a\\r\\nb';
         [n.value, t.value].join('|')",
    );
    assert_eq!(result, "|ab");
}

#[test]
fn clone_node_is_deep_only_when_asked() {
    let result = eval(
        "<div id=host><span>1</span><span>2</span></div>",
        "const host = document.getElementById('host');
         [host.cloneNode(true).children.length, host.cloneNode(false).children.length].join('|')",
    );
    assert_eq!(result, "2|0");
}

#[test]
fn validating_a_form_fires_invalid_at_each_failing_control() {
    let result = eval(
        "<form id=f><input id=a required><input id=b value=ok required></form>",
        "const seen = [];
         document.getElementById('a').addEventListener('invalid', e => seen.push(e.target.id));
         document.getElementById('b').addEventListener('invalid', e => seen.push(e.target.id));
         const ok = document.getElementById('f').checkValidity();
         [ok, seen.join(',')].join('|')",
    );
    assert_eq!(result, "false|a");
}

#[test]
fn set_selection_range_clamps_and_remembers_direction() {
    let result = eval(
        "<input id=i value=abcdef>",
        "const i = document.getElementById('i');
         i.setSelectionRange(2, 99, 'backward');
         [i.selectionStart, i.selectionEnd, i.selectionDirection].join('|')",
    );
    assert_eq!(result, "2|6|backward");
}

#[test]
fn set_range_text_splices_and_places_the_selection() {
    let result = eval(
        "<textarea id=t>abcdef</textarea>",
        "const t = document.getElementById('t');
         t.setRangeText('XY', 1, 3, 'select');
         [t.value, t.selectionStart, t.selectionEnd].join('|')",
    );
    assert_eq!(result, "aXYdef|1|3");
}

#[test]
fn controls_without_a_selection_api_report_null() {
    let result = eval(
        "<input id=n type=number value=5>",
        "const n = document.getElementById('n');
         let threw = '';
         try { n.setSelectionRange(0, 1); } catch (e) { threw = e.name + ':' + e.code; }
         [String(n.selectionStart), threw].join('|')",
    );
    assert_eq!(result, "null|InvalidStateError:11");
}

#[test]
fn focus_and_blur_move_active_element() {
    let result = eval(
        "<input id=i><input id=hidden type=hidden>",
        "const i = document.getElementById('i');
         const before = document.activeElement === document.body;
         i.focus();
         const focused = document.activeElement === i;
         document.getElementById('hidden').focus();
         const unfocusable = document.activeElement === i;
         i.blur();
         [before, focused, unfocusable, document.activeElement === document.body].join('|')",
    );
    assert_eq!(result, "true|true|true|true");
}

#[test]
fn request_submit_fires_the_event_but_submit_does_not() {
    let result = eval(
        "<form id=f><input value=x></form>",
        "const seen = [];
         const f = document.getElementById('f');
         f.addEventListener('submit', () => seen.push('submit'));
         f.submit();
         const afterSubmit = seen.length;
         f.requestSubmit();
         [afterSubmit, seen.length].join('|')",
    );
    assert_eq!(result, "0|1");
}

#[test]
fn a_cancelled_reset_keeps_the_live_state() {
    let result = eval(
        "<form id=f><input id=t value=orig></form>",
        "const f = document.getElementById('f'), t = document.getElementById('t');
         t.value = 'typed';
         const cancel = e => e.preventDefault();
         f.addEventListener('reset', cancel);
         f.reset();
         const kept = t.value;
         f.removeEventListener('reset', cancel);
         f.reset();
         [kept, t.value].join('|')",
    );
    assert_eq!(result, "typed|orig");
}

#[test]
fn stepping_snaps_to_the_grid_and_clamps() {
    let result = eval(
        "<input id=n type=number value=5 min=0 max=12 step=5>",
        "const n = document.getElementById('n');
         n.stepUp();
         const up = n.value;
         n.stepUp();
         const clamped = n.value;
         n.stepDown(2);
         [up, clamped, n.value].join('|')",
    );
    assert_eq!(result, "10|10|0");
}

#[test]
fn a_control_without_a_step_throws() {
    let result = eval(
        "<input id=a type=number step=any><input id=b value=x>",
        "const names = [];
         for (const id of ['a', 'b']) {
           try { document.getElementById(id).stepUp(); } catch (e) { names.push(e.name); }
         }
         names.join(',')",
    );
    assert_eq!(result, "InvalidStateError,InvalidStateError");
}

#[test]
fn the_form_attribute_beats_the_ancestor_form() {
    let result = eval(
        "<form id=a></form><form id=b><input id=i form=a></form>",
        "const i = document.getElementById('i');
         const first = i.form.id;
         i.removeAttribute('form');
         [first, i.form.id].join('|')",
    );
    assert_eq!(result, "a|b");
}

#[test]
fn a_form_attribute_pointing_at_a_non_form_has_no_owner() {
    let result = eval(
        "<span id=target></span><form id=real></form><input id=i form=target>",
        "const i = document.getElementById('i');
         const none = i.form;
         document.getElementById('target').id = 'other';
         document.getElementById('real').id = 'target';
         [String(none), i.form.id].join('|')",
    );
    assert_eq!(result, "null|target");
}

#[test]
fn a_detached_control_ignores_its_form_attribute() {
    let result = eval(
        "<form id=a></form><form id=outer></form>",
        "const outer = document.getElementById('outer');
         const i = document.createElement('input');
         i.setAttribute('form', 'a');
         const detached = i.form;
         outer.appendChild(i);
         [String(detached), i.form.id].join('|')",
    );
    assert_eq!(result, "null|a");
}

#[test]
fn a_parser_association_survives_moving_the_form_but_not_the_control() {
    let result = eval(
        "<table><form><tr><td><input></table><div id=box></div>",
        "const input = document.querySelector('input');
         const form = document.querySelector('form');
         const box = document.getElementById('box');
         const parsed = input.form === form;
         box.appendChild(form.parentNode);
         const carried = input.form === form;
         box.appendChild(input);
         [parsed, carried, String(input.form)].join('|')",
    );
    assert_eq!(result, "true|true|null");
}

#[test]
fn a_label_reports_its_controls_form() {
    let result = eval(
        "<form id=f><input id=i></form><label id=l for=i>x</label>",
        "const l = document.getElementById('l');
         [l.control.id, l.form.id, document.getElementById('i').labels.length].join('|')",
    );
    assert_eq!(result, "i|f|1");
}

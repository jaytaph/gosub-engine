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

#[test]
fn meter_values_are_clamped_in_order() {
    let result = eval(
        "<meter id=m min=10 max=20 low=25 value=30></meter>",
        "const m = document.getElementById('m');
         // low clamps into [min, max]; high defaults to max; value clamps last.
         [m.value, m.min, m.max, m.low, m.high, m.optimum].join('|')",
    );
    assert_eq!(result, "20|10|20|20|20|15");
}

#[test]
fn a_meter_maximum_below_its_minimum_collapses() {
    let result = eval(
        "<meter id=m min=12.1></meter>",
        "const m = document.getElementById('m');
         [m.max, m.value, m.optimum].join('|')",
    );
    assert_eq!(result, "12.1|12.1|12.1");
}

#[test]
fn meter_setters_reject_values_that_are_not_numbers() {
    let result = eval(
        "<meter id=m></meter>",
        "const m = document.getElementById('m');
         const names = [];
         for (const prop of ['value', 'min', 'max', 'low', 'high', 'optimum']) {
           try { m[prop] = 'foobar'; } catch (e) { names.push(e.constructor.name); }
         }
         names.join(',')",
    );
    assert_eq!(result, "TypeError,TypeError,TypeError,TypeError,TypeError,TypeError");
}

#[test]
fn a_progress_without_a_value_is_indeterminate() {
    let result = eval(
        "<progress id=p></progress><progress id=q value=1 max=4></progress>",
        "const p = document.getElementById('p'), q = document.getElementById('q');
         [p.position, p.value, p.max, q.position].join('|')",
    );
    assert_eq!(result, "-1|0|1|0.25");
}

#[test]
fn a_progress_refuses_a_maximum_that_is_not_positive() {
    let result = eval(
        "<progress id=p></progress>",
        "const p = document.getElementById('p');
         p.max = 42;
         p.max = 0;
         p.max = -1000;
         [p.max, p.getAttribute('max')].join('|')",
    );
    assert_eq!(result, "42|42");
}

#[test]
fn an_element_id_is_reachable_as_a_global() {
    let result = eval("<progress id=bar value=3 max=6></progress>", "String(bar.position)");
    assert_eq!(result, "0.5");
}

#[test]
fn inner_html_replaces_the_subtree() {
    let result = eval(
        "<div id=host><span>old</span></div>",
        "const host = document.getElementById('host');
         host.innerHTML = \"<input id='fresh' disabled><b>x</b>\";
         const fresh = document.getElementById('fresh');
         [host.children.length, fresh.tagName, fresh.disabled, host.innerHTML.includes('old')].join('|')",
    );
    assert_eq!(result, "2|INPUT|true|false");
}

#[test]
fn a_removed_id_stops_answering_get_element_by_id() {
    let result = eval(
        "<div id=host><span id=gone>a</span></div>",
        "const host = document.getElementById('host');
         host.innerHTML = \"<span id='gone'>b</span>\";
         // The old #gone was deleted, so this must find the new one, not the corpse.
         document.getElementById('gone').textContent",
    );
    assert_eq!(result, "b");
}

#[test]
fn an_event_handler_property_replaces_itself() {
    let result = eval(
        "<button id=b></button>",
        "const b = document.getElementById('b');
         const seen = [];
         b.onclick = () => seen.push('first');
         b.onclick = () => seen.push('second');
         b.addEventListener('click', () => seen.push('listener'));
         b.dispatchEvent(new Event('click'));
         [seen.join(','), typeof b.onclick].join('|')",
    );
    assert_eq!(result, "second,listener|function");
}

#[test]
fn clicking_a_checkbox_toggles_it_and_a_disabled_one_does_nothing() {
    let result = eval(
        "<input id=c type=checkbox><input id=d type=checkbox disabled>",
        "const c = document.getElementById('c'), d = document.getElementById('d');
         let dispatched = 0;
         d.onclick = () => dispatched++;
         c.click();
         d.click();
         [c.checked, d.checked, dispatched].join('|')",
    );
    assert_eq!(result, "true|false|0");
}

#[test]
fn clicking_a_reset_button_resets_the_form() {
    let result = eval(
        "<form id=f><input id=t value=orig><input id=r type=reset></form>",
        "const t = document.getElementById('t');
         t.value = 'typed';
         document.getElementById('r').click();
         t.value",
    );
    assert_eq!(result, "orig");
}

#[test]
fn a_cancelled_click_skips_the_activation_behaviour() {
    let result = eval(
        "<input id=c type=checkbox>",
        "const c = document.getElementById('c');
         c.onclick = e => e.preventDefault();
         c.click();
         String(c.checked)",
    );
    assert_eq!(result, "false");
}

#[test]
fn an_untouched_control_starts_with_its_selection_at_zero() {
    let result = eval(
        "<input id=i value=foobar>",
        "const i = document.getElementById('i');
         [i.selectionStart, i.selectionEnd].join('|')",
    );
    assert_eq!(result, "0|0");
}

#[test]
fn changing_the_selection_queues_one_select_event() {
    let result = eval_after_timers(
        "<input id=i value=foobar>",
        "globalThis.count = 0;
         const i = document.getElementById('i');
         i.addEventListener('select', e => { count++; globalThis.trusted = e.isTrusted; });
         i.select();
         i.setSelectionRange(0, 6);
         globalThis.sync = count;",
        "[sync, count, trusted].join('|')",
    );
    // Nothing synchronously, one event however many changes there were, and it is trusted.
    assert_eq!(result, "0|1|true");
}

#[test]
fn a_selection_that_does_not_move_fires_nothing() {
    let result = eval_after_timers(
        "<input id=i value=foobar>",
        "globalThis.count = 0;
         const i = document.getElementById('i');
         i.addEventListener('select', () => count++);
         i.setSelectionRange(1, 3);
         // A later task repeats the same range: the selection does not move, so no event.
         setTimeout(() => i.setSelectionRange(1, 3), 10);",
        "String(count)",
    );
    assert_eq!(result, "1");
}

#[test]
fn changing_type_moves_the_value_between_state_and_attribute() {
    let result = eval(
        "<input id=i>",
        "const i = document.getElementById('i');
         i.value = 'typed';
         i.type = 'submit';
         // Live state moved into the attribute...
         const asSubmit = [i.value, i.getAttribute('value')].join(',');
         i.type = 'text';
         // ...and comes back out of it.
         [asSubmit, i.value].join('|')",
    );
    assert_eq!(result, "typed,typed|typed");
}

#[test]
fn a_checkbox_arriving_from_an_empty_field_keeps_its_on_default() {
    let result = eval(
        "<input id=i type=number>",
        "const i = document.getElementById('i');
         i.value = 'not a number';
         const emptied = i.value;
         i.type = 'checkbox';
         [emptied, i.value, i.hasAttribute('value')].join('|')",
    );
    assert_eq!(result, "|on|false");
}

#[test]
fn becoming_selectable_resets_the_selection() {
    let result = eval(
        "<input id=i type=color>",
        "const i = document.getElementById('i');
         i.value = 'nonsense';
         // A colour has no selection API, and its value sanitized to seven characters.
         const beforeSelectable = String(i.selectionStart);
         i.type = 'text';
         [beforeSelectable, i.value, i.selectionStart, i.selectionEnd].join('|')",
    );
    assert_eq!(result, "null|#000000|0|0");
}

#[test]
fn a_file_control_refuses_a_value() {
    let result = eval(
        "<input id=i type=file>",
        "const i = document.getElementById('i');
         let name = '';
         try { i.value = 'C:/passwd'; } catch (e) { name = e.name; }
         [name, i.value].join('|')",
    );
    assert_eq!(result, "InvalidStateError|");
}

#[test]
fn temporal_and_color_values_sanitize() {
    let result = eval(
        "<input id=d type=date><input id=c type=color>",
        "const d = document.getElementById('d'), c = document.getElementById('c');
         d.value = 'not a date';
         c.value = 'not a colour';
         const good = document.getElementById('c');
         good.value = '#ABCDEF';
         [d.value, c.value].join('|')",
    );
    assert_eq!(result, "|#abcdef");
}

#[test]
fn a_barred_control_reports_no_validity_failures() {
    let result = eval(
        "<input id=i required disabled><input id=j required>",
        "const i = document.getElementById('i'), j = document.getElementById('j');
         [i.validity.valueMissing, i.validity.valid, j.validity.valueMissing].join('|')",
    );
    assert_eq!(result, "false|true|true");
}

#[test]
fn a_script_set_value_is_never_too_long() {
    let result = eval(
        "<input id=i maxlength=4>",
        "const i = document.getElementById('i');
         i.value = 'abcdefgh';
         // maxlength constrains what a person typed, not what script assigned.
         [i.validity.tooLong, i.checkValidity()].join('|')",
    );
    assert_eq!(result, "false|true");
}

#[test]
fn value_as_number_reads_each_temporal_format() {
    let result = eval(
        "<input id=d type=date><input id=m type=month><input id=w type=week><input id=t type=time>",
        "const read = (id, v) => { const el = document.getElementById(id); el.value = v; return el.valueAsNumber; };
         [read('d', '2019-12-10'), read('m', '2019-12'), read('w', '2019-W50'), read('t', '12:00'),
          read('d', '2019-02-29')].join('|')",
    );
    assert_eq!(result, "1575936000000|599|1575849600000|43200000|NaN");
}

#[test]
fn setting_value_as_number_serializes_back() {
    let result = eval(
        "<input id=d type=date><input id=t type=time><input id=x type=datetime-local>",
        "const write = (id, n) => { const el = document.getElementById(id); el.valueAsNumber = n; return el.value; };
         // A time wraps into its day; a datetime that lands nowhere real goes empty.
         [write('d', 0), write('t', -3600000), write('x', 2.7343337071894478e26)].join('|')",
    );
    assert_eq!(result, "1970-01-01|23:00|");
}

#[test]
fn temporal_stepping_uses_the_types_own_unit() {
    let result = eval(
        "<input id=d type=date value=2019-12-10><input id=t type=time value=12:00><input id=w type=week>",
        "const step = id => { const el = document.getElementById(id); el.stepUp(); return el.value; };
         // A day, a minute (not a second), and the Monday of the next week.
         [step('d'), step('t'), step('w')].join('|')",
    );
    assert_eq!(result, "2019-12-11|12:01|1970-W02");
}

#[test]
fn a_control_with_no_numeric_value_refuses_one() {
    let result = eval(
        "<input id=c type=checkbox>",
        "const c = document.getElementById('c');
         let caught = '';
         try { c.valueAsNumber = 5; } catch (e) { caught = e.name + '/' + (e.code === e.INVALID_STATE_ERR); }
         [String(c.valueAsNumber), caught].join('|')",
    );
    assert_eq!(result, "NaN|InvalidStateError/true");
}

#[test]
fn only_being_missing_asks_whether_the_control_is_mutable() {
    let result = eval(
        "<input id=a required disabled><input id=b type=number max=5 value=9 disabled>",
        "const a = document.getElementById('a'), b = document.getElementById('b');
         // A required field nobody can fill in is not missing, but a value past its max is
         // still past its max.
         [a.validity.valueMissing, b.validity.rangeOverflow].join('|')",
    );
    assert_eq!(result, "false|true");
}

#[test]
fn temporal_bounds_are_compared_in_their_own_units() {
    let result = eval(
        "<input id=d type=date min=2019-01-01 max=2019-12-31 value=2020-06-01>
         <input id=t type=time max=12:00 value=13:00>",
        "const d = document.getElementById('d'), t = document.getElementById('t');
         [d.validity.rangeOverflow, d.validity.rangeUnderflow, t.validity.rangeOverflow].join('|')",
    );
    assert_eq!(result, "true|false|true");
}

#[test]
fn value_as_date_is_an_instant_not_the_types_own_number() {
    let result = eval(
        "<input id=m type=month value=2019-12><input id=w type=week value=2019-W50>
         <input id=n type=number value=5>",
        "const iso = id => { const d = document.getElementById(id).valueAsDate; return d && d.toISOString(); };
         // A month's date is the first of that month; a week's is its Monday.
         [iso('m'), iso('w'), String(iso('n'))].join('|')",
    );
    assert_eq!(result, "2019-12-01T00:00:00.000Z|2019-12-09T00:00:00.000Z|null");
}

#[test]
fn cloning_carries_the_value_but_not_the_selection() {
    let result = eval(
        "<input id=i value=DEFAULT><input id=c type=checkbox checked>",
        "const i = document.getElementById('i'), c = document.getElementById('c');
         i.value = 'CHANGED';
         i.setSelectionRange(1, 4);
         c.checked = false;
         const [ic, cc] = [i.cloneNode(true), c.cloneNode(true)];
         [ic.value, ic.selectionStart, ic.selectionEnd, cc.checked].join('|')",
    );
    assert_eq!(result, "CHANGED|0|0|false");
}

#[test]
fn files_is_null_unless_the_control_takes_files() {
    let result = eval(
        "<input id=t><input id=f type=file>",
        "[String(document.getElementById('t').files),
          document.getElementById('f').files.length].join('|')",
    );
    assert_eq!(result, "null|0");
}

#[test]
fn only_some_input_types_have_a_selection_api() {
    let result = eval(
        "<input id=t type=text><input id=e type=email><input id=n type=number>",
        "const probe = id => {
           const el = document.getElementById(id);
           let threw = false;
           try { el.selectionStart = 0; } catch (e) { threw = e.name === 'InvalidStateError'; }
           return String(el.selectionStart) + ':' + threw;
         };
         // Email is out: `multiple` lets it hold a list, so it has no single selection.
         [probe('t'), probe('e'), probe('n')].join('|')",
    );
    assert_eq!(result, "0:false|null:true|null:true");
}

#[test]
fn assigning_the_same_value_leaves_the_cursor_alone() {
    let result = eval(
        "<input id=i value=abcdef><textarea id=t>a\nb</textarea>",
        "const i = document.getElementById('i'), t = document.getElementById('t');
         i.setSelectionRange(1, 4);
         i.value = 'abcdef';
         const kept = [i.selectionStart, i.selectionEnd].join(',');
         i.value = 'changed';
         const moved = [i.selectionStart, i.selectionEnd].join(',');
         // A textarea normalises CRLF, so this counts as the same value too.
         t.setSelectionRange(1, 1);
         t.value = 'a\\r\\nb';
         [kept, moved, t.selectionStart].join('|')",
    );
    assert_eq!(result, "1,4|7,7|1");
}

#[test]
fn a_detached_label_cannot_reach_into_the_document() {
    let result = eval(
        "<input id=target><label id=inside for=target>x</label>",
        "const detached = document.createElement('label');
         detached.setAttribute('for', 'target');
         const inside = document.getElementById('inside');
         [String(detached.control), inside.control.id].join('|')",
    );
    assert_eq!(result, "null|target");
}

//! water-rs/hydrolysis#189 — `LongPressGesture` resolves to a recognizer on
//! the same model as the tap and drag recognizers: it fires after the
//! configured minimum duration while the press stays within the movement
//! tolerance, for a touch and for a held primary pointer; it fails when the
//! press moves out of tolerance or is released early; and it composes through
//! `.then` / `.sequenced_before` / `.simultaneously_with` the way the other
//! recognizers do. The timer runs through the runtime's own pump — a queued
//! input event lands under `pump_at`'s frame instant and a later pump is the
//! tick the deadline was armed for — never a thread sleep.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use waterui::gesture::{Gesture, LongPressEvent, LongPressGesture, TapGesture};
use waterui::{AnyView, Color, ViewExt as _};
use waterui_core::handler::AnyViewBuilder;
use waterui_layout::stack::hstack;

use super::{MinimalTestTheme, test_environment};
use crate::HeadlessRuntime;
use crate::platform::{InputEvent, PointerButton, PointerKind};
use crate::time::Instant;

const POINTER_ID: u64 = 9;
const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;
const HANDLE_WIDTH: f32 = 20.0;

/// The recognizer's movement tolerance is 10pt — the same slop model the tap
/// and drag recognizers use — so a 4pt move keeps the press armed and a 20pt
/// move kills it.
const WITHIN_TOLERANCE: f32 = 4.0;
const PAST_TOLERANCE: f32 = 20.0;
const DURATION_MS: u64 = 300;

/// Each fired long-press records its event's configured duration (rounded to
/// whole milliseconds, keeping the assertions float-free) so the tests see
/// the recognizer's own payload, not just that an action ran.
type Fired = Rc<RefCell<Vec<u64>>>;

fn fired_sink() -> Fired {
    Rc::new(RefCell::new(Vec::new()))
}

fn runtime_with(view: AnyView) -> HeadlessRuntime {
    let view = RefCell::new(Some(view));
    let builder = AnyViewBuilder::<AnyView>::new(move || {
        view.borrow_mut()
            .take()
            .expect("the test view is built once")
    });
    let mut runtime = HeadlessRuntime::new_for_tests(
        test_environment(),
        builder,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        MinimalTestTheme::default(),
    );
    // The first pump is the flush that registers the gesture targets — a
    // press inside it dispatches before any region exists, the way a real
    // frame can never order input ahead of the tree that produced the regions.
    for _ in 0..4 {
        let _ = runtime.pump(false);
    }
    runtime
}

fn at(start: Instant, ms: u64) -> Instant {
    start
        .checked_add(Duration::from_millis(ms))
        .expect("test timestamp overflow")
}

fn pointer_down_at(runtime: &mut HeadlessRuntime, kind: PointerKind, x: f32, y: f32, at: Instant) {
    runtime.push_input_event(InputEvent::PointerDown {
        id: POINTER_ID,
        kind,
        x,
        y,
        button: PointerButton::Primary,
    });
    let _ = runtime.pump_at(false, at);
}

fn pointer_move_at(runtime: &mut HeadlessRuntime, kind: PointerKind, x: f32, y: f32, at: Instant) {
    runtime.push_input_event(InputEvent::PointerMove {
        id: POINTER_ID,
        kind,
        x,
        y,
    });
    let _ = runtime.pump_at(false, at);
}

fn pointer_up_at(runtime: &mut HeadlessRuntime, kind: PointerKind, x: f32, y: f32, at: Instant) {
    runtime.push_input_event(InputEvent::PointerUp {
        id: POINTER_ID,
        kind,
        x,
        y,
        button: PointerButton::Primary,
    });
    let _ = runtime.pump_at(false, at);
}

fn pump_at(runtime: &mut HeadlessRuntime, at: Instant) {
    let _ = runtime.pump_at(false, at);
}

fn handle(gesture: impl Into<Gesture>, fired: Fired) -> AnyView {
    AnyView::new(Color::srgb_hex("#3F3F46").width(HANDLE_WIDTH).gesture(
        gesture,
        move |event: waterui_core::extract::Use<LongPressEvent>| {
            fired.borrow_mut().push(event.duration.round() as u64);
        },
    ))
}

fn split(handle: AnyView) -> AnyView {
    AnyView::new(
        hstack((
            AnyView::new(Color::srgb_hex("#18181B")),
            handle,
            AnyView::new(Color::srgb_hex("#27272A")),
        ))
        .spacing(0.0),
    )
}

/// A held primary pointer fires the handler once when the deadline passes —
/// not before it, and not again while the press keeps holding.
#[test]
fn held_primary_pointer_fires_once_past_duration() {
    let fired = fired_sink();
    let mut runtime = runtime_with(split(handle(
        LongPressGesture::new(DURATION_MS as u32),
        Rc::clone(&fired),
    )));
    let start = Instant::now();

    pointer_down_at(&mut runtime, PointerKind::Mouse, 400.0, 300.0, start);
    pump_at(&mut runtime, at(start, DURATION_MS - 1));
    assert!(
        fired.borrow().is_empty(),
        "the press must not fire before the minimum duration"
    );

    pump_at(&mut runtime, at(start, DURATION_MS));
    assert_eq!(
        fired.borrow().as_slice(),
        &[DURATION_MS],
        "the press must fire once when the deadline passes"
    );

    pump_at(&mut runtime, at(start, DURATION_MS * 3));
    assert_eq!(
        fired.borrow().len(),
        1,
        "a press still held must not fire a second time"
    );

    pointer_up_at(
        &mut runtime,
        PointerKind::Mouse,
        400.0,
        300.0,
        at(start, DURATION_MS * 3),
    );
    assert_eq!(fired.borrow().len(), 1, "release must not refire");
}

/// Touch takes the same recognizer path the primary pointer does.
#[test]
fn held_touch_fires_once_past_duration() {
    let fired = fired_sink();
    let mut runtime = runtime_with(split(handle(
        LongPressGesture::new(DURATION_MS as u32),
        Rc::clone(&fired),
    )));
    let start = Instant::now();

    pointer_down_at(&mut runtime, PointerKind::Touch, 400.0, 300.0, start);
    pump_at(&mut runtime, at(start, DURATION_MS * 2));
    assert_eq!(fired.borrow().as_slice(), &[DURATION_MS]);

    pointer_up_at(
        &mut runtime,
        PointerKind::Touch,
        400.0,
        300.0,
        at(start, DURATION_MS * 2),
    );
    assert_eq!(fired.borrow().len(), 1);
}

/// A release before the deadline fails the recognizer: the handler never runs
/// and a later tick does not resurrect the press.
#[test]
fn early_release_does_not_fire() {
    let fired = fired_sink();
    let mut runtime = runtime_with(split(handle(
        LongPressGesture::new(DURATION_MS as u32),
        Rc::clone(&fired),
    )));
    let start = Instant::now();

    pointer_down_at(&mut runtime, PointerKind::Mouse, 400.0, 300.0, start);
    pointer_up_at(
        &mut runtime,
        PointerKind::Mouse,
        400.0,
        300.0,
        at(start, 100),
    );
    pump_at(&mut runtime, at(start, DURATION_MS * 2));
    assert!(fired.borrow().is_empty());
}

/// Movement out of tolerance fails the recognizer; a press that stays inside
/// it keeps its deadline.
#[test]
fn movement_past_tolerance_cancels() {
    let fired = fired_sink();
    let mut runtime = runtime_with(split(handle(
        LongPressGesture::new(DURATION_MS as u32),
        Rc::clone(&fired),
    )));
    let start = Instant::now();

    pointer_down_at(&mut runtime, PointerKind::Mouse, 400.0, 300.0, start);
    pointer_move_at(
        &mut runtime,
        PointerKind::Mouse,
        400.0 + PAST_TOLERANCE,
        300.0,
        at(start, 50),
    );
    pump_at(&mut runtime, at(start, DURATION_MS * 2));
    assert!(
        fired.borrow().is_empty(),
        "a move out of tolerance must cancel the pending press"
    );
}

/// A move inside the tolerance keeps the press armed — the deadline still
/// fires from the original down.
#[test]
fn movement_within_tolerance_still_fires() {
    let fired = fired_sink();
    let mut runtime = runtime_with(split(handle(
        LongPressGesture::new(DURATION_MS as u32),
        Rc::clone(&fired),
    )));
    let start = Instant::now();

    pointer_down_at(&mut runtime, PointerKind::Mouse, 400.0, 300.0, start);
    pointer_move_at(
        &mut runtime,
        PointerKind::Mouse,
        400.0 + WITHIN_TOLERANCE,
        300.0,
        at(start, 50),
    );
    pump_at(&mut runtime, at(start, DURATION_MS * 2));
    assert_eq!(fired.borrow().as_slice(), &[DURATION_MS]);
}

/// `TapGesture::new().then(LongPressGesture::new(300))` fires the handler once,
/// after the tap completes and the following press holds past the duration —
/// the completing child's event is the one delivered.
#[test]
fn tap_then_long_press_fires_in_order() {
    let fired = fired_sink();
    let mut runtime = runtime_with(split(handle(
        TapGesture::new().then(LongPressGesture::new(DURATION_MS as u32)),
        Rc::clone(&fired),
    )));
    let start = Instant::now();

    // A hold alone only completes the tap leg on release — the sequence must
    // not fire for it.
    pointer_down_at(&mut runtime, PointerKind::Mouse, 400.0, 300.0, start);
    pump_at(&mut runtime, at(start, DURATION_MS * 2));
    assert!(
        fired.borrow().is_empty(),
        "the sequence must not fire while the first leg is still running"
    );
    pointer_up_at(
        &mut runtime,
        PointerKind::Mouse,
        400.0,
        300.0,
        at(start, DURATION_MS * 2),
    );
    assert!(
        fired.borrow().is_empty(),
        "the tap leg completing must not fire the composed gesture"
    );

    // The press that follows the tap holds past the duration.
    pointer_down_at(
        &mut runtime,
        PointerKind::Mouse,
        400.0,
        300.0,
        at(start, DURATION_MS * 2 + 16),
    );
    pump_at(&mut runtime, at(start, DURATION_MS * 3 + 16));
    assert_eq!(
        fired.borrow().as_slice(),
        &[DURATION_MS],
        "the sequence fires once when the long-press leg completes in order"
    );
}

//! water-rs/hydrolysis#177 — the action environment contract is **ancestor
//! order**: a `.gesture`/`.action`/`on_hover_enter` handler sees `.state(&v)`
//! installed *outside* the handler's modifier, never inside it.
//!
//! The contract is decided on three independent fronts:
//!
//! - the canonical idiom in `waterui/docs/api-style.md` is
//!   `.action(|State(c): State<Binding<i32>>| *c.get_mut() += 1).state(&counter)`
//!   — the state install wraps the handler;
//! - the extractor's own panic says where to install: "install the value with
//!   `.state(&value)` on an ancestor of the handler's view"
//!   (waterui `core/src/foundation/extract.rs`);
//! - every dispatch path in this backend resolves the handler against the env
//!   the observer saw at apply time — `apply_gesture_observer`,
//!   `emit_gesture_observer_accessibility`, `apply_on_event`, the press path
//!   and keyboard activation all capture `env.clone()` at the metadata node
//!   and invoke `captured_env.layered_on(runtime_env)`.
//!
//! What actually regressed in 1ec6cd4d..4332751c is dispatch order, not
//! environment resolution: `metadata.rs` and the waterui env code are
//! byte-identical across the range, and the same inside-order view panics
//! identically on 1ec6cd4d (verified: this file's tests run there and fail
//! the same way). fd112e8 (#170) moved `gesture_engine.handle_pointer_move`
//! ahead of `handle_embedded_pointer_move`, so a recognizer pending on the
//! handle is no longer starved by a move that lands on an input-receiving
//! neighbour. Before #170 a drag off the narrow handle was claimed by the
//! adjacent pane on its first move and the action simply never ran — a leak
//! of non-execution that looked like the contract allowing inside order.
//!
//! Each test below pins one dispatch path on one runtime against both
//! modifier orders.

use std::cell::RefCell;
use std::rc::Rc;

use accesskit::{Action, ActionRequest, NodeId, Role, TreeId, TreeUpdate};
use waterui::accessibility::AccessibilityRole;
use waterui::gesture::{DragEvent, DragGesture, TapGesture};
use waterui::{AnyView, Binding, Color, ViewExt as _};
use waterui_backend_core::widget::{ButtonMetrics, InteractionStyle};
use waterui_core::Environment;
use waterui_core::extract::{State, Use};
use waterui_core::handler::AnyViewBuilder;
use waterui_layout::stack::hstack;

use super::{MinimalTestTheme, test_environment};
use crate::platform::{InputEvent, KeyCode, KeyState, Modifiers, PointerButton, PointerKind};
use crate::runner::SemanticRuntime;
use crate::{HeadlessRuntime, keyboard_types};

const POINTER_ID: u64 = 9;
const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;
const HANDLE_WIDTH: f32 = 20.0;

/// What the action records per fire: `(sizes[0], grab)`. `None` entries mark a
/// fire that ran but could not extract the `State` — possible because the
/// handler asks for `Option<State<_>>`.
type Fired = Rc<RefCell<Vec<(Option<f32>, Option<f32>)>>>;

/// `Option` so the action still runs when the env cannot satisfy the extractor
/// — exactly what inside order produces.
type OptState<T> = Option<State<Binding<T>>>;

fn fired_sink() -> Fired {
    Rc::new(RefCell::new(Vec::new()))
}

fn runtime_with(view: AnyView) -> HeadlessRuntime {
    runtime_with_env(test_environment(), view)
}

fn runtime_with_env(env: Environment, view: AnyView) -> HeadlessRuntime {
    let view = RefCell::new(Some(view));
    let builder = AnyViewBuilder::<AnyView>::new(move || {
        view.borrow_mut()
            .take()
            .expect("the test view is built once")
    });
    let mut runtime = HeadlessRuntime::new_for_tests(
        env,
        builder,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        MinimalTestTheme::default(),
    );
    for _ in 0..4 {
        let _ = runtime.pump(false);
    }
    runtime
}

fn pointer_down(runtime: &mut HeadlessRuntime, x: f32, y: f32) {
    runtime.push_input_event(InputEvent::PointerDown {
        id: POINTER_ID,
        kind: PointerKind::Mouse,
        x,
        y,
        button: PointerButton::Primary,
    });
    let _ = runtime.pump(false);
}

fn pointer_move(runtime: &mut HeadlessRuntime, x: f32, y: f32) {
    runtime.push_input_event(InputEvent::PointerMove {
        id: POINTER_ID,
        kind: PointerKind::Mouse,
        x,
        y,
    });
    let _ = runtime.pump(false);
}

fn pointer_up(runtime: &mut HeadlessRuntime, x: f32, y: f32) {
    runtime.push_input_event(InputEvent::PointerUp {
        id: POINTER_ID,
        kind: PointerKind::Mouse,
        x,
        y,
        button: PointerButton::Primary,
    });
    let _ = runtime.pump(false);
}

fn key(runtime: &mut HeadlessRuntime, key: KeyCode, state: KeyState) {
    runtime.push_input_event(InputEvent::Key {
        logical_key: key.to_w3c_key(),
        physical_code: keyboard_types::Code::Unidentified,
        repeat: false,
        key,
        state,
        modifiers: Modifiers::default(),
    });
    let _ = runtime.pump(false);
}

/// The issue's shape verbatim: a 20-point handle between two plain panes. The
/// handle view is where the modifier order under test is applied.
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

/// The drag action body both orders share: extract `State` optionally so the
/// action still runs when the environment cannot satisfy the extractor —
/// which is exactly what inside order produces.
fn drag_action(fired: Fired) -> impl Fn(Use<DragEvent>, OptState<Vec<f32>>, OptState<f32>) {
    move |_drag: Use<DragEvent>, sizes: OptState<Vec<f32>>, grab: OptState<f32>| {
        fired.borrow_mut().push((
            sizes.map(|State(s)| s.get_mut()[0]),
            grab.map(|State(g)| *g.get_mut()),
        ));
    }
}

fn tap_action(fired: Fired) -> impl Fn(OptState<Vec<f32>>, OptState<f32>) {
    move |sizes: OptState<Vec<f32>>, grab: OptState<f32>| {
        fired.borrow_mut().push((
            sizes.map(|State(s)| s.get_mut()[0]),
            grab.map(|State(g)| *g.get_mut()),
        ));
    }
}

fn state_bindings() -> (Binding<Vec<f32>>, Binding<f32>) {
    (nami::binding(vec![400.0, 393.0]), nami::binding(0.0_f32))
}

fn handle() -> AnyView {
    AnyView::new(Color::srgb_hex("#3F3F46").width(HANDLE_WIDTH))
}

fn assert_all_extracted(fired: &Fired, path: &str) {
    let fires = fired.borrow();
    assert!(
        !fires.is_empty(),
        "{path}: the action never fired — it must run and extract"
    );
    assert!(
        fires.iter().all(|fire| *fire == (Some(400.0), Some(0.0))),
        "{path}: every fire must extract the ancestor `.state` values, got {fires:?}"
    );
}

fn assert_all_missing(fired: &Fired, path: &str) {
    let fires = fired.borrow();
    assert!(
        !fires.is_empty(),
        "{path}: the action never fired — inside order still runs it"
    );
    assert!(
        fires.iter().all(|fire| *fire == (None, None)),
        "{path}: inside-order `.state` must be invisible, got {fires:?}"
    );
}

// ---------------------------------------------------------------------------
// Rendered runtime — `KeyboardActivation::PressRelease` (hit_test.rs:166-178).
// ---------------------------------------------------------------------------

/// The issue's order verbatim: `.state` between the view and `.gesture`. The
/// drag fires — and extracts nothing — because the installs sit inside the
/// observer, not on an ancestor.
#[test]
fn drag_gesture_cannot_extract_state_installed_between_view_and_gesture() {
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let fired_for_view = Rc::clone(&fired);
    let view = handle()
        .state(&sizes)
        .state(&grab)
        .gesture(DragGesture::new(0.0), drag_action(fired_for_view));
    let mut runtime = runtime_with(split(AnyView::new(view)));

    pointer_down(&mut runtime, 400.0, 300.0);
    pointer_move(&mut runtime, 430.0, 300.0);

    assert_all_missing(&fired, "drag gesture, inside order");
}

/// Canonical order: `.gesture` first, `.state` after — the installs land on an
/// ancestor of the handler's view and every fire extracts them.
#[test]
fn drag_gesture_extracts_state_installed_on_an_ancestor() {
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .gesture(DragGesture::new(0.0), drag_action(Rc::clone(&fired)))
        .state(&sizes)
        .state(&grab);
    let mut runtime = runtime_with(split(AnyView::new(view)));

    pointer_down(&mut runtime, 400.0, 300.0);
    pointer_move(&mut runtime, 430.0, 300.0);

    assert_all_extracted(&fired, "drag gesture, ancestor order");
}

/// Tap resolves through the same captured-env `layered_action` (metadata.rs).
#[test]
fn tap_gesture_extracts_state_installed_on_an_ancestor() {
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .gesture(TapGesture::new(), tap_action(Rc::clone(&fired)))
        .state(&sizes)
        .state(&grab);
    let mut runtime = runtime_with(split(AnyView::new(view)));

    pointer_down(&mut runtime, 400.0, 300.0);
    pointer_up(&mut runtime, 400.0, 300.0);

    assert_all_extracted(&fired, "tap gesture, ancestor order");
}

/// Press/keyboard: with an `InteractionStyle` installed, a tap activates
/// through `register_interactive_pointer_target_with_keyboard` rather than the
/// recognizer — still `captured_env.layered_on(runtime_env)` (metadata.rs).
#[test]
fn press_activation_extracts_state_installed_on_an_ancestor() {
    let mut env = test_environment();
    env.install(InteractionStyle::new(
        ButtonMetrics::new(16.0, 8.0, 0.0, 0.0),
        Color::srgb(0, 0, 0),
        0.0_f64,
    ));
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .gesture(TapGesture::new(), tap_action(Rc::clone(&fired)))
        .state(&sizes)
        .state(&grab);
    let mut runtime = runtime_with_env(env, split(AnyView::new(view)));

    pointer_down(&mut runtime, 400.0, 300.0);
    pointer_up(&mut runtime, 400.0, 300.0);

    assert_all_extracted(&fired, "press activation, ancestor order");
}

/// Keyboard: the pointer-down focus handoff (hit_test.rs) then Enter release
/// fires `(target.action)(self, point, env)` with the captured env.
#[test]
fn keyboard_activation_extracts_state_installed_on_an_ancestor() {
    let mut env = test_environment();
    env.install(InteractionStyle::new(
        ButtonMetrics::new(16.0, 8.0, 0.0, 0.0),
        Color::srgb(0, 0, 0),
        0.0_f64,
    ));
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .gesture(TapGesture::new(), tap_action(Rc::clone(&fired)))
        .state(&sizes)
        .state(&grab);
    let mut runtime = runtime_with_env(env, split(AnyView::new(view)));

    // Focus lands on the interactive target on press; Enter release fires it.
    pointer_down(&mut runtime, 400.0, 300.0);
    key(
        &mut runtime,
        KeyCode::Named("Enter".into()),
        KeyState::Pressed,
    );
    key(
        &mut runtime,
        KeyCode::Named("Enter".into()),
        KeyState::Released,
    );
    pointer_up(&mut runtime, 400.0, 300.0);

    assert_all_extracted(&fired, "keyboard activation, ancestor order");
}

/// `on_hover_enter` is `Metadata<OnEvent>` — a different observer type than
/// `GestureObserver`, same captured-env resolution (`apply_on_event`).
#[test]
fn hover_enter_extracts_state_installed_on_an_ancestor() {
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .on_hover_enter(tap_action(Rc::clone(&fired)))
        .state(&sizes)
        .state(&grab);
    let mut runtime = runtime_with(split(AnyView::new(view)));

    pointer_move(&mut runtime, 400.0, 300.0);

    assert_all_extracted(&fired, "hover enter, ancestor order");
}

// ---------------------------------------------------------------------------
// Semantic runtime — `KeyboardActivation::Semantic` (hit_test.rs:166-178):
// dispatch goes through the accessibility tree's Activate action.
// ---------------------------------------------------------------------------

fn semantic_runtime_with(view: AnyView) -> SemanticRuntime {
    let view = RefCell::new(Some(view));
    SemanticRuntime::new_for_tests(
        Environment::new(),
        AnyViewBuilder::<AnyView>::new(move || {
            view.borrow_mut()
                .take()
                .expect("the test view is built once")
        }),
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
    )
}

fn semantic_update(runtime: &mut SemanticRuntime) -> TreeUpdate {
    let mut last = None;
    for _ in 0..64 {
        let result = runtime.pump();
        if let Some(update) = result.tree_update {
            last = Some(update);
        }
        if runtime.is_settled() {
            break;
        }
    }
    assert!(
        !runtime.has_pending_semantic_update(),
        "semantic runtime never settled"
    );
    last.expect("the pump emitted no tree update")
}

fn first_of_role(update: &TreeUpdate, role: Role) -> NodeId {
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.role() == role).then_some(*id))
        .unwrap_or_else(|| panic!("no {role:?} node in the semantic tree"))
}

fn semantic_action(runtime: &mut SemanticRuntime, action: Action, target: NodeId) {
    assert!(
        runtime.perform_accessibility_action(ActionRequest {
            action,
            target_node: target,
            target_tree: TreeId::ROOT,
            data: None,
        }),
        "the accessibility {action:?} changed nothing"
    );
    let _ = semantic_update(runtime);
}

fn semantic_key(runtime: &mut SemanticRuntime, key: KeyCode) {
    runtime.push_input_event(InputEvent::Key {
        logical_key: key.to_w3c_key(),
        physical_code: keyboard_types::Code::Unidentified,
        repeat: false,
        key,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
    });
    let _ = semantic_update(runtime);
}

/// The Tap observer emits a Button node when `AccessibilityRole` is in the
/// apply-time env; Activate resolves through the same captured env
/// (emit_gesture_observer_accessibility).
#[test]
fn semantic_activate_extracts_state_installed_on_an_ancestor() {
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .gesture(TapGesture::new(), tap_action(Rc::clone(&fired)))
        .a11y_role(AccessibilityRole::Button)
        .state(&sizes)
        .state(&grab);
    let mut runtime = semantic_runtime_with(split(AnyView::new(view)));

    let update = semantic_update(&mut runtime);
    let node = first_of_role(&update, Role::Button);
    semantic_action(&mut runtime, Action::Click, node);

    assert_all_extracted(&fired, "semantic activate, ancestor order");
}

#[test]
fn semantic_activate_cannot_extract_state_installed_between_view_and_gesture() {
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .state(&sizes)
        .state(&grab)
        .gesture(TapGesture::new(), tap_action(Rc::clone(&fired)))
        .a11y_role(AccessibilityRole::Button);
    let mut runtime = semantic_runtime_with(split(AnyView::new(view)));

    let update = semantic_update(&mut runtime);
    let node = first_of_role(&update, Role::Button);
    semantic_action(&mut runtime, Action::Click, node);

    assert_all_missing(&fired, "semantic activate, inside order");
}

/// Semantic keyboard dispatch: Enter on the focused node goes through the same
/// semantic Activate target `Action::Click` uses (hit_test.rs:166-178).
#[test]
fn semantic_keyboard_activate_extracts_state_installed_on_an_ancestor() {
    let fired = fired_sink();
    let (sizes, grab) = state_bindings();
    let view = handle()
        .gesture(TapGesture::new(), tap_action(Rc::clone(&fired)))
        .a11y_role(AccessibilityRole::Button)
        .state(&sizes)
        .state(&grab);
    let mut runtime = semantic_runtime_with(split(AnyView::new(view)));

    let update = semantic_update(&mut runtime);
    let node = first_of_role(&update, Role::Button);
    semantic_action(&mut runtime, Action::Focus, node);
    semantic_key(&mut runtime, KeyCode::Named("Enter".into()));

    assert_all_extracted(&fired, "semantic keyboard activate, ancestor order");
}

//! Scripted platform IME sequences replayed through the runner (#85).
//!
//! Every fixture in `tests/fixtures/ime` encodes the `InputEvent`s a real
//! platform's input method produces, in the order its documentation gives
//! (winit backend sources, IBus/fcitx5, TSF/IMM, AppKit NSTextInputClient).
//! The events are pushed through `HeadlessRuntime::push_input_event` one
//! platform batch per `[[step]]`, so what the receivers observe is what a
//! winit window would deliver.
//!
//! The receivers are a single-line `TextField`, a multi-line editor, a
//! `SecureField` (IME must not be allowed, `purpose == Password`), an
//! input-wanting `GpuSurface`, and an input-wanting `SceneView`. Beyond the
//! text and selection each fixture expects, the replay asserts that the
//! caret rect reported through `focused_text_input_state` follows the
//! pre-edit caret (#25) and that keys/text the platform already consumed for
//! the composition never reach ordinary key handling or the surface.

use core::time::Duration;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use serde::Deserialize;
use waterui::ViewExt as _;
use waterui::component::text;
use waterui_controls::button::button;
use waterui_controls::text_field::field;
use waterui_core::handler::AnyViewBuilder;
use waterui_core::{AnyView, Binding, Str};
use waterui_form::secure::{Secure, secure};
use waterui_graphics::input::{Code, Key, NamedKey, SurfaceInputEvent};
use waterui_graphics::{
    GpuContext, GpuFrame, GpuSurface, GpuView, Scene2D, SceneContent, SceneView,
};
use waterui_layout::stack::vstack;

use super::{MinimalTestTheme, test_environment};
use crate::HeadlessRuntime;
use crate::platform::{
    InputEvent, KeyCode, KeyState, Modifiers, PointerButton, PointerKind, TextInputPurpose,
};

const WINDOW_WIDTH: u32 = 400;
const WINDOW_HEIGHT: u32 = 640;
const HEADER_HEIGHT: f32 = 100.0;
const FIELD_WIDTH: f32 = 300.0;
const FIELD_HEIGHT: f32 = 60.0;
const EDITOR_HEIGHT: f32 = 120.0;
const SURFACE_WIDTH: f32 = 200.0;
const SURFACE_HEIGHT: f32 = 150.0;

/// Where the 200-wide surface lands in the 400-wide window, under the
/// full-width 100-high header plus the stack's 10-point default spacing.
const SURFACE_ORIGIN_X: f64 = 100.0;
const SURFACE_ORIGIN_Y: f64 = 110.0;

const POINTER_ID: u64 = 7;

#[derive(Debug, Deserialize)]
struct Fixture {
    final_text: String,
    selection: [usize; 2],
    committing_key_swallowed: bool,
    #[serde(rename = "step", default)]
    steps: Vec<FixtureStep>,
}

#[derive(Debug, Deserialize)]
struct FixtureStep {
    #[serde(default)]
    events: Vec<FixtureEvent>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FixtureEvent {
    Preedit { preedit: FixturePreedit },
    Commit { commit: String },
    Disabled { disabled: bool },
    Key { key: FixtureKey },
    Text { text: String },
}

#[derive(Debug, Deserialize)]
struct FixturePreedit {
    text: String,
    caret: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FixtureKey {
    logical: String,
    code: String,
    state: String,
}

struct LoadedFixture {
    name: String,
    fixture: Fixture,
}

fn load_fixtures() -> Vec<LoadedFixture> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ime");
    let mut fixtures: Vec<LoadedFixture> = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("cannot enumerate {}: {err}", dir.display()))
    {
        let path = entry
            .expect("fixture directory entry must be readable")
            .path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
        let fixture: Fixture = toml::from_str(&source)
            .unwrap_or_else(|err| panic!("{} fails to parse: {err}", path.display()));
        fixtures.push(LoadedFixture {
            name: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("fixture name is UTF-8")
                .to_owned(),
            fixture,
        });
    }
    fixtures.sort_by(|left, right| left.name.cmp(&right.name));
    assert!(
        !fixtures.is_empty(),
        "no IME fixtures found under {}",
        dir.display()
    );
    fixtures
}

fn to_input_event(event: &FixtureEvent) -> InputEvent {
    match event {
        FixtureEvent::Preedit { preedit } => InputEvent::ImePreedit {
            text: preedit.text.clone(),
            caret: preedit.caret,
        },
        FixtureEvent::Commit { commit } => InputEvent::ImeCommit {
            text: commit.clone(),
        },
        FixtureEvent::Disabled { disabled } => {
            assert!(*disabled, "a `disabled` fixture event must be true");
            InputEvent::ImeDisabled
        }
        FixtureEvent::Text { text } => InputEvent::TextInput { text: text.clone() },
        FixtureEvent::Key { key } => {
            let logical_key = key
                .logical
                .parse::<Key>()
                .unwrap_or(Key::Named(NamedKey::Unidentified));
            let key_code = match &logical_key {
                Key::Named(named) => KeyCode::Named(format!("{named:?}")),
                Key::Character(value) => KeyCode::Character(value.clone()),
            };
            InputEvent::Key {
                key: key_code,
                logical_key,
                physical_code: key.code.parse::<Code>().unwrap_or(Code::Unidentified),
                repeat: false,
                state: match key.state.as_str() {
                    "pressed" => KeyState::Pressed,
                    "released" => KeyState::Released,
                    other => panic!("unsupported fixture key state {other:?}"),
                },
                modifiers: Modifiers::default(),
            }
        }
    }
}

fn push_step(runtime: &mut HeadlessRuntime, step: &FixtureStep) {
    for event in &step.events {
        runtime.push_input_event(to_input_event(event));
    }
}

fn runtime_with(view: AnyView) -> HeadlessRuntime {
    let view = RefCell::new(Some(view));
    let builder = AnyViewBuilder::<AnyView>::new(move || {
        view.borrow_mut()
            .take()
            .expect("the test view is built once")
    });
    HeadlessRuntime::new_for_tests(
        test_environment(),
        builder,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        MinimalTestTheme::default(),
    )
}

/// Pumps until the mounted view has registered every target the replay needs.
fn settled(runtime: &mut HeadlessRuntime, start: Instant) {
    for frame in 0..6 {
        let _ = runtime.pump_at(false, start + Duration::from_millis(frame * 16));
    }
}

fn press(runtime: &mut HeadlessRuntime, x: f32, y: f32) {
    runtime.push_input_event(InputEvent::PointerDown {
        id: POINTER_ID,
        kind: PointerKind::Mouse,
        x,
        y,
        button: PointerButton::Primary,
    });
    runtime.push_input_event(InputEvent::PointerUp {
        id: POINTER_ID,
        kind: PointerKind::Mouse,
        x,
        y,
        button: PointerButton::Primary,
    });
}

/// Presses the center of the text input registered at `index` — the frame a
/// fixed `.size()` produces centers the material field, so its real bounds
/// sit inside the frame rather than filling it.
fn press_text_input(runtime: &mut HeadlessRuntime, index: usize) {
    let center = runtime.renderer().text_editing.text_input_targets[index]
        .bounds
        .center();
    press(runtime, center.x as f32, center.y as f32);
}

/// The observable state of a text-input receiver while a fixture runs.
struct TextReplay<'a> {
    name: &'a str,
    /// Accumulated committed text the fixture has produced so far.
    committed: String,
    /// The pre-edit the receiver should currently hold, and the byte offset
    /// of the caret the platform reported inside it.
    composing: Option<(String, Option<usize>)>,
    /// `(pre-edit text, caret, reported rect x)` per composition update, so a
    /// caret that moves inside unchanged marked text can be seen to move the
    /// reported rect (#25).
    caret_samples: Vec<(String, usize, f64)>,
}

impl<'a> TextReplay<'a> {
    fn new(name: &'a str) -> Self {
        Self {
            name,
            committed: String::new(),
            composing: None,
            caret_samples: Vec::new(),
        }
    }

    fn track_events(&mut self, step: &FixtureStep) {
        for event in &step.events {
            match event {
                FixtureEvent::Preedit { preedit } if preedit.text.is_empty() => {
                    self.composing = None;
                }
                FixtureEvent::Preedit { preedit } => {
                    self.composing = Some((preedit.text.clone(), preedit.caret));
                }
                FixtureEvent::Commit { commit } => {
                    self.committed += commit;
                    self.composing = None;
                }
                FixtureEvent::Disabled { .. }
                | FixtureEvent::Key { .. }
                | FixtureEvent::Text { .. } => {}
            }
        }
    }
}

/// Samples the reported caret rect after one pumped step and checks the live
/// composition state. `secure` receivers must never hold a pre-edit at all.
fn observe_step(
    replay: &mut TextReplay<'_>,
    runtime: &mut HeadlessRuntime,
    step: &FixtureStep,
    step_index: usize,
    secure: bool,
) {
    replay.track_events(step);
    let name = replay.name;
    let preedit = runtime.renderer().text_editing.ime_preedit.clone();
    if secure {
        assert!(
            preedit.is_none(),
            "{name} step {step_index}: a password-purpose field must never store \
             a pre-edit (got {preedit:?})"
        );
    } else {
        let expected = replay.composing.as_ref().map(|(text, _)| text.as_str());
        assert_eq!(
            preedit.as_deref(),
            expected,
            "{name} step {step_index}: the live composition must be the last \
             pre-edit the platform sent"
        );
    }
    let state = runtime
        .focused_text_input_state()
        .unwrap_or_else(|| panic!("{name} step {step_index}: focused field reports no caret rect"));
    assert_eq!(
        state.purpose,
        if secure {
            TextInputPurpose::Password
        } else {
            TextInputPurpose::Normal
        },
        "{name} step {step_index}: wrong input purpose"
    );
    assert!(
        state.height > 5.0,
        "{name} step {step_index}: the caret rect must span the composing \
         line so a candidate window can anchor below it (got {})",
        state.height
    );
    if !secure && let Some((text, Some(caret))) = replay.composing.clone() {
        replay.caret_samples.push((text, caret, state.x));
    }
}

/// Two composition updates on the same marked text whose caret moved must
/// move the reported rect in the same direction — the #25 check.
fn assert_caret_tracks_preedit(replay: &TextReplay<'_>) {
    for pair in replay.caret_samples.windows(2) {
        let [(text_a, caret_a, x_a), (text_b, caret_b, x_b)] = pair else {
            continue;
        };
        if text_a != text_b {
            continue;
        }
        match caret_b.cmp(caret_a) {
            core::cmp::Ordering::Greater => assert!(
                x_b > x_a,
                "{}: caret moved {caret_a} -> {caret_b} inside {text_a:?} but the \
                 reported rect stayed at x={x_a}",
                replay.name
            ),
            core::cmp::Ordering::Less => assert!(
                x_b < x_a,
                "{}: caret moved {caret_a} -> {caret_b} inside {text_a:?} but the \
                 reported rect stayed at x={x_a}",
                replay.name
            ),
            core::cmp::Ordering::Equal => {}
        }
    }
}

fn selection_of(runtime: &HeadlessRuntime) -> (usize, usize) {
    let targets = &runtime.renderer().text_editing.text_input_targets;
    assert_eq!(targets.len(), 1, "the fixture harness mounts one field");
    let slot = targets[0].selection.borrow();
    (slot.anchor, slot.focus)
}

fn replay_against_text_field(loaded: &LoadedFixture, multiline: bool) {
    let name = loaded.name.as_str();
    let receiver = if multiline {
        "multi-line editor"
    } else {
        "single-line TextField"
    };
    let value = Binding::container(Str::default());
    let submitted = Binding::bool(false);
    let view = {
        let value_for_view = value.clone();
        let submitted_for_action = submitted.clone();
        let field_view = field("Name", &value_for_view);
        let field_view = if multiline {
            field_view.disable_line_limit()
        } else {
            field_view
        };
        AnyView::new(vstack((
            field_view.size(
                FIELD_WIDTH,
                if multiline {
                    EDITOR_HEIGHT
                } else {
                    FIELD_HEIGHT
                },
            ),
            button("Submit").action(move || submitted_for_action.set(true)),
        )))
    };
    let mut runtime = runtime_with(view);
    let start = Instant::now();
    settled(&mut runtime, start);
    let mut now = start;

    press_text_input(&mut runtime, 0);
    now += Duration::from_millis(16);
    let _ = runtime.pump_at(false, now);
    assert!(
        runtime.focused_text_input_state().is_some(),
        "{name} ({receiver}): pressing the field must focus it"
    );

    let mut replay = TextReplay::new(name);
    for (index, step) in loaded.fixture.steps.iter().enumerate() {
        push_step(&mut runtime, step);
        now += Duration::from_millis(16);
        let _ = runtime.pump_at(false, now);
        observe_step(&mut replay, &mut runtime, step, index, false);
        assert_eq!(
            value.get().to_string().as_str(),
            replay.committed,
            "{name} ({receiver}) step {index}: committed text diverged from \
             the fixture's commits"
        );
    }

    assert_eq!(
        value.get().to_string().as_str(),
        loaded.fixture.final_text,
        "{name} ({receiver}): final text"
    );
    assert_eq!(
        selection_of(&runtime),
        (loaded.fixture.selection[0], loaded.fixture.selection[1]),
        "{name} ({receiver}): final selection"
    );
    assert!(
        runtime.renderer().text_editing.ime_preedit.is_none(),
        "{name} ({receiver}): a finished fixture leaves no composition"
    );
    if loaded.fixture.committing_key_swallowed {
        assert!(
            !submitted.get(),
            "{name} ({receiver}): the confirming key must not activate the form"
        );
    }
    assert_caret_tracks_preedit(&replay);
}

fn replay_against_secure_field(loaded: &LoadedFixture) {
    let name = loaded.name.as_str();
    let secret = Binding::container(Secure::new(String::new()));
    let view = {
        let secret_for_view = secret.clone();
        AnyView::new(vstack((
            secure("Password", &secret_for_view).size(FIELD_WIDTH, FIELD_HEIGHT),
        )))
    };
    let mut runtime = runtime_with(view);
    let start = Instant::now();
    settled(&mut runtime, start);
    let mut now = start;

    press_text_input(&mut runtime, 0);
    now += Duration::from_millis(16);
    let _ = runtime.pump_at(false, now);
    let state = runtime
        .focused_text_input_state()
        .unwrap_or_else(|| panic!("{name} (SecureField): pressing the field must focus it"));
    assert_eq!(
        state.purpose,
        TextInputPurpose::Password,
        "{name} (SecureField): IME must be told the field is a password"
    );

    let mut replay = TextReplay::new(name);
    for (index, step) in loaded.fixture.steps.iter().enumerate() {
        push_step(&mut runtime, step);
        now += Duration::from_millis(16);
        let _ = runtime.pump_at(false, now);
        observe_step(&mut replay, &mut runtime, step, index, true);
    }

    assert_eq!(
        secret.get().expose(),
        loaded.fixture.final_text,
        "{name} (SecureField): commits land as plain text edits"
    );
    assert_eq!(
        selection_of(&runtime),
        (loaded.fixture.selection[0], loaded.fixture.selection[1]),
        "{name} (SecureField): final selection"
    );
}

/// Records every event its surface receives, so a test can read them after
/// the frame that delivered them.
#[derive(Clone, Default)]
struct ProbeLog(Rc<RefCell<Vec<SurfaceInputEvent>>>);

impl ProbeLog {
    fn drain(&self) -> Vec<SurfaceInputEvent> {
        core::mem::take(&mut *self.0.borrow_mut())
    }
}

/// A caret rect the embedded view reports in its own logical coordinates —
/// the point `focused_text_input_state` projects into the window.
fn probe_caret() -> Option<vello::kurbo::Rect> {
    Some(vello::kurbo::Rect::new(10.0, 20.0, 12.0, 38.0))
}

struct InputProbe {
    log: ProbeLog,
}

impl GpuView for InputProbe {
    async fn setup(&mut self, _ctx: &GpuContext<'_>, _env: &mut waterui_core::Environment) {}

    fn render(&mut self, _frame: &mut GpuFrame) {}

    fn wants_input_events(&self) -> bool {
        true
    }

    fn input(&mut self, event: &SurfaceInputEvent) {
        self.log.0.borrow_mut().push(event.clone());
    }

    fn ime_caret(&self) -> Option<vello::kurbo::Rect> {
        probe_caret()
    }
}

struct SceneProbe {
    log: ProbeLog,
}

impl SceneContent for SceneProbe {
    fn build_scene(&mut self, _scene: &mut dyn Scene2D, _width: f32, _height: f32) -> bool {
        false
    }

    fn wants_input_events(&self) -> bool {
        true
    }

    fn input(&mut self, event: &SurfaceInputEvent) {
        self.log.0.borrow_mut().push(event.clone());
    }

    fn ime_caret(&self) -> Option<vello::kurbo::Rect> {
        probe_caret()
    }
}

fn surface_view(scene: bool, log: ProbeLog) -> AnyView {
    let surface = if scene {
        AnyView::new(SceneView::new(SceneProbe { log }))
    } else {
        AnyView::new(GpuSurface::new(InputProbe { log }))
    };
    AnyView::new(vstack((
        vstack((text("header"),)).size(WINDOW_WIDTH as f32, HEADER_HEIGHT),
        surface.size(SURFACE_WIDTH, SURFACE_HEIGHT),
    )))
}

/// The composition session a surface should observe, derived mechanically
/// from the fixture's IME events — never from its key/text events: those are
/// the platform's composing keystrokes and stay inside the IME.
fn expected_surface_events(fixture: &Fixture) -> Vec<SurfaceInputEvent> {
    let mut composing = false;
    let mut expected = Vec::new();
    for step in &fixture.steps {
        for event in &step.events {
            match event {
                FixtureEvent::Preedit { preedit } if preedit.text.is_empty() => {
                    if composing {
                        composing = false;
                        expected.push(SurfaceInputEvent::CompositionCancel);
                    }
                }
                FixtureEvent::Preedit { preedit } => {
                    if !composing {
                        composing = true;
                        expected.push(SurfaceInputEvent::CompositionStart);
                    }
                    expected.push(SurfaceInputEvent::CompositionUpdate {
                        text: preedit.text.clone().into(),
                        caret: preedit.caret,
                    });
                }
                FixtureEvent::Commit { commit } => {
                    if !composing {
                        expected.push(SurfaceInputEvent::CompositionStart);
                    }
                    composing = false;
                    expected.push(SurfaceInputEvent::CompositionCommit(commit.clone().into()));
                }
                FixtureEvent::Disabled { .. } => {
                    if composing {
                        composing = false;
                        expected.push(SurfaceInputEvent::CompositionCancel);
                    }
                }
                FixtureEvent::Key { .. } | FixtureEvent::Text { .. } => {}
            }
        }
    }
    expected
}

fn replay_against_surface(loaded: &LoadedFixture, scene: bool) {
    let name = loaded.name.as_str();
    let receiver = if scene { "SceneView" } else { "GpuSurface" };
    let log = ProbeLog::default();
    let mut runtime = runtime_with(surface_view(scene, log.clone()));
    let start = Instant::now();
    settled(&mut runtime, start);
    let _ = log.drain();

    press(
        &mut runtime,
        (SURFACE_ORIGIN_X + 20.0) as f32,
        (SURFACE_ORIGIN_Y + 20.0) as f32,
    );
    let mut now = start + Duration::from_millis(100);
    let _ = runtime.pump_at(false, now);
    let _ = log.drain();
    assert!(
        runtime.focused_text_input_state().is_some(),
        "{name} ({receiver}): pressing the surface must focus it"
    );

    for step in &loaded.fixture.steps {
        push_step(&mut runtime, step);
        now += Duration::from_millis(16);
        let _ = runtime.pump_at(false, now);
    }

    assert_eq!(
        log.drain(),
        expected_surface_events(&loaded.fixture),
        "{name} ({receiver}): the surface must see the composition session, \
         never the platform's composing keystrokes"
    );
}

#[test]
fn fixtures_replay_against_a_single_line_text_field() {
    for loaded in load_fixtures() {
        replay_against_text_field(&loaded, false);
    }
}

#[test]
fn fixtures_replay_against_a_multi_line_editor() {
    for loaded in load_fixtures() {
        replay_against_text_field(&loaded, true);
    }
}

#[test]
fn fixtures_replay_against_a_secure_field() {
    for loaded in load_fixtures() {
        replay_against_secure_field(&loaded);
    }
}

#[test]
fn fixtures_replay_against_an_input_wanting_gpu_surface() {
    for loaded in load_fixtures() {
        replay_against_surface(&loaded, false);
    }
}

#[test]
fn fixtures_replay_against_an_input_wanting_scene_view() {
    for loaded in load_fixtures() {
        replay_against_surface(&loaded, true);
    }
}

/// Moving focus to another field — or the window unfocusing — mid-composition
/// must cancel it: the marked text is dropped, never committed.
#[test]
fn focus_move_or_window_unfocus_cancels_the_composition() {
    let first = Binding::container(Str::default());
    let second = Binding::container(Str::default());
    let view = {
        let first_for_view = first.clone();
        let second_for_view = second.clone();
        AnyView::new(vstack((
            field("First", &first_for_view).size(FIELD_WIDTH, FIELD_HEIGHT),
            field("Second", &second_for_view).size(FIELD_WIDTH, FIELD_HEIGHT),
        )))
    };
    let mut runtime = runtime_with(view);
    let start = Instant::now();
    settled(&mut runtime, start);
    let mut now = start;

    press_text_input(&mut runtime, 0);
    now += Duration::from_millis(16);
    let _ = runtime.pump_at(false, now);
    runtime.push_input_event(InputEvent::ImePreedit {
        text: "にほ".to_owned(),
        caret: Some(6),
    });
    now += Duration::from_millis(16);
    let _ = runtime.pump_at(false, now);
    assert!(
        runtime.renderer().text_editing.ime_preedit.is_some(),
        "composition must be live before the focus move"
    );

    // Clicking the second field cancels the first's composition.
    press_text_input(&mut runtime, 1);
    now += Duration::from_millis(16);
    let _ = runtime.pump_at(false, now);
    assert!(
        runtime.renderer().text_editing.ime_preedit.is_none(),
        "moving focus must cancel the composition, not commit it"
    );
    assert!(first.get().to_string().is_empty());

    // A window unfocus (Ime::Disabled) mid-composition cancels it too.
    runtime.push_input_event(InputEvent::ImePreedit {
        text: "かな".to_owned(),
        caret: Some(6),
    });
    now += Duration::from_millis(16);
    let _ = runtime.pump_at(false, now);
    assert!(runtime.renderer().text_editing.ime_preedit.is_some());
    runtime.push_input_event(InputEvent::ImeDisabled);
    now += Duration::from_millis(16);
    let _ = runtime.pump_at(false, now);
    assert!(
        runtime.renderer().text_editing.ime_preedit.is_none(),
        "window unfocus must cancel the composition"
    );
    assert!(second.get().to_string().is_empty());
}

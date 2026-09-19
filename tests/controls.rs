//! Renderer presentation tests for controls: pointer routing around disabled
//! controls and the disabled scope's reactive re-enable.
//!
//! Received from water-rs/waterui under water-rs/waterui#1130 (class 2 —
//! renderer presentation); every case names its origin file and asserts what
//! it asserted there, mounted under `Material3::defaults()` on the rendered
//! runtime.

use waterui::Binding;
use waterui::View;
use waterui::ViewExt as _;
use waterui::component::vstack;
use waterui::graphics::color::Srgb;
use waterui_controls::{Menu, button, label, slider::slider, toggle};
use waterui_testing::{OffscreenApp, Role, Styled, UiBuilder};

fn control_shell<V: View>(content: V) -> impl View {
    vstack((content,))
        .spacing(12.0)
        .padding_with(16.0)
        .background(Srgb::BLACK)
}

/// Asserts that an interaction panics because the runtime rejected it —
/// the contract for actions on disabled or clamped controls.
fn assert_rejected(context: &str, action: impl FnOnce()) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
    assert!(
        outcome.is_err(),
        "{context}: the runtime should reject this action"
    );
}

fn assert_close(actual: f64, expected: f64, epsilon: f64, context: &str) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= epsilon,
        "{context}: expected {expected:.4}, got {actual:.4}, delta={delta:.4}, epsilon={epsilon:.4}"
    );
}

// Origin: waterui `components/foundation/controls/tests/e2e_semantics.rs`.
#[waterui::test(theme = hydrolysis_m3::Material3::defaults(), viewport = (320, 240))]
fn disabled_toggle_ignores_input_and_reports_disabled(
    ui: UiBuilder<Styled<hydrolysis_m3::Material3>>,
) {
    let enabled = Binding::bool(false);
    let enabled_for_view = enabled.clone();

    let mut app = ui
        .mount_offscreen(move || control_shell(toggle("Wi-Fi", &enabled_for_view).disabled(true)));

    let element = app.query().role(Role::SWITCH).label("Wi-Fi").single();
    assert!(
        !element.node().enabled(),
        "disabled-toggle: switch should expose disabled accessibility state"
    );
    assert_rejected(
        "disabled-toggle: accessibility tap should be rejected",
        || {
            app.query().role(Role::SWITCH).label("Wi-Fi").tap();
        },
    );
    // The pointer event dispatches into the window but must not hit the
    // disabled control: the binding stays unchanged.
    app.query()
        .role(Role::SWITCH)
        .label("Wi-Fi")
        .tap_at(0.5, 0.5);
    assert!(
        !enabled.get(),
        "disabled-toggle: binding must stay unchanged"
    );
}

// Origin: waterui `components/foundation/controls/tests/e2e_semantics.rs`.
#[waterui::test(theme = hydrolysis_m3::Material3::defaults(), viewport = (320, 240))]
fn disabled_scope_cascades_and_reenables_reactively(
    ui: UiBuilder<Styled<hydrolysis_m3::Material3>>,
) {
    let enabled = Binding::bool(false);
    let enabled_for_view = enabled.clone();
    let form_locked = Binding::bool(true);
    let form_locked_for_view = form_locked.clone();

    let mut app = ui.mount_offscreen(move || {
        control_shell(
            vstack((toggle("Notifications", &enabled_for_view),))
                .disabled(form_locked_for_view.clone()),
        )
    });

    let element = app
        .query()
        .role(Role::SWITCH)
        .label("Notifications")
        .single();
    assert!(
        !element.node().enabled(),
        "disabled-scope: toggle inside a disabled container must report disabled"
    );
    assert_rejected(
        "disabled-scope: tap inside a disabled container must be rejected",
        || {
            app.query().role(Role::SWITCH).label("Notifications").tap();
        },
    );
    assert!(
        !enabled.get(),
        "disabled-scope: binding must stay unchanged"
    );

    form_locked.set(false);
    assert!(
        app.query()
            .role(Role::SWITCH)
            .label("Notifications")
            .enabled(true)
            .wait_for_existence(core::time::Duration::from_secs(2)),
        "disabled-scope: re-enabling the container must re-enable the toggle"
    );
    app.query().role(Role::SWITCH).label("Notifications").tap();
    assert!(
        enabled.get(),
        "disabled-scope: tap after re-enable must flip the binding"
    );
}

// Origin: waterui `components/foundation/controls/tests/e2e_semantics.rs`.
#[waterui::test(theme = hydrolysis_m3::Material3::defaults(), viewport = (320, 240))]
fn disabled_slider_ignores_value_actions(ui: UiBuilder<Styled<hydrolysis_m3::Material3>>) {
    let value = Binding::f64(0.5);
    let value_for_view = value.clone();

    let mut app =
        ui.mount_offscreen(move || control_shell(slider("Volume", &value_for_view).disabled(true)));

    let element = app.query().role(Role::SLIDER).label("Volume").single();
    assert!(
        !element.node().enabled(),
        "disabled-slider: slider should expose disabled accessibility state"
    );
    assert_rejected("disabled-slider: increment must be rejected", || {
        app.query().role(Role::SLIDER).label("Volume").increment();
    });
    // The pointer drag dispatches into the window but must not hit the
    // disabled control: the value stays unchanged.
    app.query()
        .role(Role::SLIDER)
        .label("Volume")
        .drag_by(60.0, 0.0);
    assert_close(
        value.get(),
        0.5,
        0.0001,
        "disabled-slider: value must stay unchanged",
    );
}

// Origin: waterui `components/foundation/controls/tests/e2e_semantics.rs`.
#[waterui::test(theme = hydrolysis_m3::Material3::defaults(), viewport = (320, 240))]
fn disabled_button_ignores_action(ui: UiBuilder<Styled<hydrolysis_m3::Material3>>) {
    let count = Binding::i32(0);
    let count_for_view = count.clone();

    let mut app = ui.mount_offscreen(move || {
        control_shell(
            button("Submit")
                .action(|waterui::State(count): waterui::State<Binding<i32>>| {
                    *count.get_mut() += 1;
                })
                .disabled(true)
                .state(&count_for_view),
        )
    });

    assert_rejected(
        "disabled-button: accessibility tap should be rejected",
        || {
            app.query().role(Role::BUTTON).label("Submit").tap();
        },
    );
    // The pointer event dispatches into the window but must not hit the
    // disabled control: the action never runs.
    app.query()
        .role(Role::BUTTON)
        .label("Submit")
        .tap_at(0.5, 0.5);
    assert_eq!(count.get(), 0, "disabled-button: action must not run");
}

fn actions_menu_view() -> impl waterui::View {
    control_shell(Menu::new(
        label("Actions").icon(()),
        (
            button("Refresh").action(|| {}),
            Menu::new("Advanced", (button("Archive").action(|| {}),)),
        ),
    ))
}

// Origin: waterui `components/foundation/controls/tests/e2e_semantics.rs` —
// the geometric half of `menu_button_exposes_accessible_name`; the accessible
// name half stays in waterui as a semantic test.
#[waterui::test(actions_menu_view, theme = hydrolysis_m3::Material3::defaults(), viewport = (320, 240), offscreen)]
fn menu_button_exposes_accessible_name(app: &mut OffscreenApp) {
    let menu = app.query().role(Role::BUTTON).label("Actions").single();
    let bounds = menu.bounds();
    assert!(
        bounds.width() > 0.0 && bounds.height() > 0.0,
        "menu-button-exposes-accessible-name: menu trigger bounds must be non-zero"
    );
}

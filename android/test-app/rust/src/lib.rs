//! The debug app the Android landing tests run: a scrollable column of
//! text, a counter driven by a real tap, a toggle, a slider, a text field
//! and a secure field for IME/accessibility/autofill coverage, plus one
//! `Native<PlatformView>` leaf that mounts the registry's WebView between
//! GPU-drawn controls for the z-order and event-ownership checks.

use std::rc::Rc;

use waterui::app::App;
use waterui::form::secure::Secure;
use waterui::graphics::color::Srgb;
use waterui::prelude::*;
use waterui::reactive::binding;
use waterui::window::{Window, WindowState};
use waterui_core::Native;

fn app_view(
    count: Binding<i32>,
    enabled: Binding<bool>,
    name: Binding<Str>,
    password: Binding<Secure>,
    volume: Binding<f64>,
) -> impl View {
    scroll(
        vstack((
            text("Hydrolysis on Android").title(),
            "Vello paints this UI through the Kotlin host's SurfaceView band.",
            Divider,
            text!("Tap count: {count}", count = count),
            button("Increment")
                .action(move |State(count): State<Binding<i32>>| {
                    count.with_mut(|value| *value += 1);
                })
                .state(&count),
            Toggle::new("Enable notifications", &enabled),
            text!("Notifications: {enabled}", enabled = enabled),
            slider("Volume", &volume),
            text!("Volume: {volume}", volume = volume),
            TextField::new("Email address", &name).prompt("Autofill: email"),
            SecureField::new("Password", &password),
            text!("Hello, {name}!", name = name),
            Divider,
            layout::frame::Frame::new(Native::new(hydrolysis::PlatformView::new("webview")))
                .height(220.0),
            text("Resize + recreation keep this tree alive.").foreground(Srgb::from_hex("#6B6B70")),
        ))
        .padding(),
    )
}

fn build_app() -> App {
    App::new_with_windows(
        [Window::new(
            "Hydrolysis Test App",
            binding(WindowState::Normal),
            move || {
                let count = binding(0);
                let enabled = binding(false);
                let name = binding("");
                let password = binding(waterui::form::secure::Secure::new(String::new()));
                let volume = binding(0.5);
                app_view(count, enabled, name, password, volume)
            },
        )],
        Environment::new(),
    )
}

/// Entry from the JVM: installs logging, registers the app factory the
/// Kotlin host mounts per session create.
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(
    _vm: *mut std::ffi::c_void,
    _reserved: *mut std::ffi::c_void,
) -> i32 {
    hydrolysis::android::init_logging();
    hydrolysis::android::register_app(|| {
        (
            build_app(),
            Rc::new(hydrolysis_m3::Material3::defaults()) as Rc<dyn hydrolysis::Style>,
        )
    });
    jni::sys::JNI_VERSION_1_6
}

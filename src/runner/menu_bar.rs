//! `App::menu_bar` installation (watergram DOGFOOD r43-1).
//!
//! Every runner that destructures the app used to drop `menu_bar`, so an
//! app-level menu bar rendered nothing and armed no shortcuts. The runner
//! now resolves the menus once — `resolve_menu_bar_items` keeps the signal
//! reactive, so item edits apply on the next lookup — and, where the
//! platform exposes no menu-bar surface of its own, registers their
//! command chords on the window-shared [`MenuShortcutRegistry`] as an
//! app-scoped source (live for the app's duration, answering to whichever
//! window dispatches — the same behaviour a mounted `Menu` gives, minus
//! the window affinity).
//!
//! What hydrolysis renders per platform:
//!
//! - **winit on macOS** — `NSApp.mainMenu` is the system menu bar: the
//!   menus become real `NSMenu` items via `muda`, commands carrying their
//!   `shortcut` as the AppKit key equivalent. AppKit matches a key
//!   equivalent in `-[NSApplication sendEvent]` before the event is
//!   delivered as `keyDown` to the window (winit's view does not override
//!   `performKeyEquivalent`), so a claimed accelerator never reaches the
//!   registry. The registry stays armed as the fallback for chords muda
//!   could not express or AppKit did not claim — the two paths see
//!   disjoint keys, so a chord still fires exactly once.
//! - **winit on Windows** — every window owns a Win32 menu bar: `muda`
//!   builds the `HMENU` and attaches it to each application window's
//!   `HWND`, with the shortcuts as accelerator text. Stock winit's message
//!   pump never calls `TranslateAcceleratorW`, so the accelerators are
//!   display-only and the registry owns the chords: click through muda,
//!   chord through the registry — exactly once.
//! - **winit on Linux, the web runner, and the headless hosts** — no
//!   menu-bar surface (winit exposes none on Linux; a browser page cannot
//!   own the browser's menus; a headless host has no chrome at all).
//!   Shortcuts arm on every window and nothing renders — matching the
//!   gtk-backend, which also does not surface `menu_bar` on Linux.
//!
//! Whichever path renders a native menu, choosing an item posts a
//! `muda::MenuEvent` on muda's channel; the winit runner drains it once
//! per event-loop pass (`pump_menu_events`) and runs the command's
//! `SharedAction` through `call_action_discarding_result` with the app
//! env — the same dispatch the registry uses for chords.

use waterui_controls::menu::{Menu, resolve_menu_bar_items};
use waterui_core::Environment;

use crate::renderer::{MISSING_MENU_SHORTCUT_REGISTRY, MenuShortcutRegistry};

/// What installing the app's menu bar set up: chords on the registry plus
/// a native menu-bar surface where the platform has one (winit on macOS
/// and Windows — see the module docs for how chord dispatch stays
/// exactly-once on each). The runner keeps the returned value alive for
/// the app's duration.
pub(crate) struct MenuBarInstall {
    #[cfg(all(feature = "winit", any(target_os = "macos", target_os = "windows")))]
    native: crate::platform::native_menu_bar::NativeMenuBar,
}

impl MenuBarInstall {
    /// Drains muda's `MenuEvent` channel and dispatches each command's
    /// action through the same path the registry uses. A no-op where no
    /// native surface exists. The winit runner calls this once per
    /// `about_to_wait` pass — muda posts events from the main thread.
    #[cfg(feature = "winit")]
    pub(crate) fn pump_menu_events(&self) {
        #[cfg(all(feature = "winit", any(target_os = "macos", target_os = "windows")))]
        self.native.pump_menu_events();
    }

    /// Attaches the native bar to a freshly created application window
    /// (Windows `HWND`; a no-op everywhere else).
    #[cfg(all(feature = "winit", target_os = "windows"))]
    pub(crate) fn attach_hwnd(&self, hwnd: isize) {
        self.native.attach_hwnd(hwnd);
    }

    /// Forgets an application window's `HWND` before the window is
    /// destroyed, so the native bar never touches a dead handle (a no-op
    /// everywhere else).
    #[cfg(all(feature = "winit", target_os = "windows"))]
    pub(crate) fn detach_hwnd(&self, hwnd: isize) {
        self.native.detach_hwnd(hwnd);
    }
}

/// Resolves `menu_bar` against `env`, registers the app-scoped chord
/// source, and installs the native surface where one exists. The app `env`
/// is what the commands' actions are invoked with, layered over whichever
/// window dispatches (same as a mounted `Menu`'s).
pub(crate) fn install_menu_bar(
    menu_bar: &nami::Computed<Vec<Menu>>,
    env: &Environment,
) -> MenuBarInstall {
    let items = resolve_menu_bar_items(menu_bar, env);
    env.get::<MenuShortcutRegistry>()
        .expect(MISSING_MENU_SHORTCUT_REGISTRY)
        .register_menu_bar(items.clone(), env.clone());
    MenuBarInstall {
        #[cfg(all(feature = "winit", any(target_os = "macos", target_os = "windows")))]
        native: crate::platform::native_menu_bar::NativeMenuBar::install(items, env),
    }
}

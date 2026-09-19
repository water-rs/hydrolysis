//! Renderer presentation tests for scroll geometry.
//!
//! Received from water-rs/waterui under water-rs/waterui#1130 (class 2 —
//! renderer presentation); every case names its origin file and asserts what
//! it asserted there, mounted under `Material3::defaults()` on the rendered
//! runtime.

use waterui::View;
use waterui::ViewExt as _;
use waterui::graphics::color::Srgb;
use waterui::prelude::*;
use waterui_layout::scroll;
use waterui_testing::{OffscreenApp, Role};

fn visual_shell<V: View>(content: V) -> impl View {
    content.padding_with(20.0).background(Srgb::BLACK)
}

fn labeled_card(label: &'static str, width: f32, height: f32, color: Srgb) -> impl View {
    text(label)
        .body()
        .foreground(Srgb::WHITE)
        .size(width, height)
        .background(color)
}

fn scroll_content_view() -> impl View {
    visual_shell(
        scroll(
            vstack((
                labeled_card("First item", 120.0, 48.0, Srgb::new(1.0, 0.1, 0.1)),
                labeled_card("Second item", 120.0, 48.0, Srgb::new(0.1, 1.0, 0.1)),
                labeled_card("Third item", 120.0, 48.0, Srgb::new(0.1, 0.1, 1.0)),
                labeled_card("Fourth item", 120.0, 48.0, Srgb::new(1.0, 0.8, 0.1)),
            ))
            .spacing(12.0),
        )
        .size(120.0, 120.0)
        .a11y_label("scroll-layout"),
    )
}

// Origin: waterui `testing/tests/layout.rs`.
#[waterui::test(scroll_content_view, theme = hydrolysis_m3::Material3::defaults(), offscreen, viewport = (180, 180))]
fn scroll_view_scroll_down_changes_content(app: &mut OffscreenApp) {
    let second_before = app
        .query()
        .role(Role::LABEL)
        .label("Second item")
        .single()
        .bounds();
    app.query().label("scroll-layout").scroll_down();

    let second_after = app
        .query()
        .role(Role::LABEL)
        .label("Second item")
        .single()
        .bounds();
    app.query()
        .role(Role::LABEL)
        .label("Third item")
        .assert_exists();
    assert!(
        second_after.y() < second_before.y(),
        "scrolling down should move earlier content upward: before={second_before:?} after={second_after:?}"
    );
}

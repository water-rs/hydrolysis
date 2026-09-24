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
use waterui_layout::scroll::{ScrollView, scroll_horizontal};
use waterui_testing::{OffscreenApp, Role, ui};

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

// Defect reproduction: a Material filter-chip row clipped by its scroll rail
// inside a 340 pt sidebar ("Archive" rendered "Archiv" while ~40 pt of free
// space remains before the trailing icon). The rail is a `scroll()` —
// default `Axis::Vertical` — wrapping the horizontal chip row next to a
// trailing icon button.
//
// Root cause is in `RenderNode::Scroll::measure`: it answered
// `proposal.unwrap_or(0)` on both axes, so the `0` probe on the scroll's
// non-scrolling axis reported `0` instead of the content's intrinsic extent —
// layout-spec.md §6 ("only a `0` proposal measures the content, answering its
// intrinsic extent on the non-scrolling axis and `0` on the scrolling
// axis"). The row's negotiator (§4.2) then treated the rail as a 0-minimum
// flexible member: the icon kept its 48 pt target and the rail absorbed the
// whole deficit, clipping the last chip's label inside the viewport —
// unreachable, since a vertical scroll cannot scroll horizontally.
fn chips() -> impl View {
    hstack((
        hydrolysis_m3::filter_chip("All", &Binding::bool(false)),
        hydrolysis_m3::filter_chip("Work", &Binding::bool(false)),
        hydrolysis_m3::filter_chip("Personal", &Binding::bool(false)),
        hydrolysis_m3::filter_chip("Archive", &Binding::bool(false)),
    ))
    .spacing(8.0)
}

fn chip_row(rail: ScrollView) -> impl View {
    vstack((hstack((
        rail.a11y_label("chip-rail"),
        hydrolysis_m3::icon_button("More filters", text("F")),
    ))
    .spacing(8.0),))
}

// The chips' intrinsic row extent: 4 chips plus 3 gaps at spacing 8
// (48.8 + 64.7 + 88.3 + 79.9 + 24.0).
const CHIP_ROW_EXTENT: f32 = 305.6;
// The rail's slot when it absorbs the row deficit: 340 - 8 spacing - 48 icon.
const SQUEEZED_RAIL: f32 = 284.0;

/// §6 scroll contract on the non-scrolling axis: the vertical rail's minimum
/// is the chips' intrinsic width, so at 340 pt the rail keeps 305.6 pt and
/// the row overflows past the trailing icon instead of clipping "Archive".
#[test]
fn vertical_scroll_rail_reports_content_minimum_on_non_scrolling_axis() {
    let mut app = ui()
        .viewport(340, 300)
        .theme(hydrolysis_m3::Material3::defaults())
        .mount_offscreen(|| chip_row(scroll(chips())));
    app.settle();
    let rail = app.query().label("chip-rail").single().bounds();
    assert!(
        (rail.width() - CHIP_ROW_EXTENT).abs() < 1.0,
        "vertical rail should report the content's intrinsic minimum on its \
         non-scrolling axis (§6), got rail={rail:?} (bug: squeezed to {SQUEEZED_RAIL})"
    );
    let archive = app
        .query()
        .role(Role::BUTTON)
        .label("Archive")
        .single()
        .bounds();
    assert!(
        archive.x() + archive.width() <= rail.x() + rail.width() + 0.1,
        "Archive chip should lie fully inside the rail: archive={archive:?} rail={rail:?}"
    );
    let icon = app
        .query()
        .role(Role::BUTTON)
        .label("More filters")
        .single()
        .bounds();
    assert!(
        icon.x() >= rail.width() - 0.1,
        "the trailing icon should be pushed past the rail by the overflow: {icon:?}"
    );
}

/// The scrolling axis is the one that takes `0`: at 340 pt a horizontal rail
/// legitimately negotiates down and clips its overflow — scrollable, by
/// design. This pins the spec-correct side of the same rule.
#[test]
fn horizontal_scroll_rail_keeps_zero_minimum_on_scrolling_axis() {
    let mut app = ui()
        .viewport(340, 300)
        .theme(hydrolysis_m3::Material3::defaults())
        .mount_offscreen(|| chip_row(scroll_horizontal(chips())));
    app.settle();
    let rail = app.query().label("chip-rail").single().bounds();
    assert!(
        (rail.width() - SQUEEZED_RAIL).abs() < 1.0,
        "horizontal rail should absorb the row deficit (§6: 0 on the \
         scrolling axis), got rail={rail:?}"
    );
}

/// At 600 pt the row fits either way: the rail claims the offer on both
/// variants, every chip is whole, and the icon sits at the row's right edge.
#[test]
fn chip_rail_fills_offer_when_row_fits() {
    for horizontal in [false, true] {
        let mut app = ui()
            .viewport(600, 300)
            .theme(hydrolysis_m3::Material3::defaults())
            .mount_offscreen(move || {
                chip_row(if horizontal {
                    scroll_horizontal(chips())
                } else {
                    scroll(chips())
                })
            });
        app.settle();
        let archive = app
            .query()
            .role(Role::BUTTON)
            .label("Archive")
            .single()
            .bounds();
        assert!(
            archive.x() + archive.width() <= CHIP_ROW_EXTENT + 0.1,
            "chips render whole: archive={archive:?}"
        );
        let icon = app
            .query()
            .role(Role::BUTTON)
            .label("More filters")
            .single()
            .bounds();
        assert!(
            (icon.x() - 552.0_f32).abs() < 1.0,
            "icon fills the row's trailing edge at 600 pt: {icon:?}"
        );
    }
}

//! Text measure-versus-paint fidelity on the real font stack.
//!
//! `HeadlessRuntime::new_for_tests` shapes text with the bundled deterministic
//! fonts, which measure snugly; a divergence between the measured advance and
//! the painted ink extent only shows on the fonts the windowed runners load
//! (system collection + `resources/fonts`, including fallback). These tests
//! run the winit runner's font path via
//! [`HeadlessRuntime::new_for_tests_native_fonts`].

use std::time::Instant;

use nami::Binding;
use nami::collection::SignalCollection;
use waterui::ViewExt as _;
use waterui::graphics::color::Srgb;
use waterui::prelude::text;
use waterui::shape::RoundedRectangle;
use waterui::theme::color::Surface;
use waterui_core::AnyView;
use waterui_core::handler::AnyViewBuilder;
use waterui_core::id::SelfId;
use waterui_layout::stack::{VStack, vstack};

use super::{MinimalTestTheme, test_environment};
use crate::HeadlessRuntime;

const W: u32 = 320;
const H: u32 = 240;

fn rows(rgba8: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    rgba8
        .as_chunks::<{ W as usize * 4 }>()
        .0
        .iter()
        .map(<[u8; W as usize * 4]>::as_slice)
        .enumerate()
}

fn px(row: &[u8], x: usize) -> [u8; 3] {
    row[x * 4..x * 4 + 3].try_into().unwrap()
}

fn is_white(px: [u8; 3]) -> bool {
    px[0] > 240 && px[1] > 240 && px[2] > 240
}

/// A pixel that is neither the white pill/ink nor the blue canvas: glyph ink
/// (solid #1B1B1F or its antialiased blend over the canvas).
fn is_dark_ink(px: [u8; 3]) -> bool {
    px[0] < 0x80 && px[1] < 0x80 && px[2] < 0xB4
}

/// Bounding box of the capsule: for every row, the span of its widest
/// contiguous white run is part of the pill (pill rows have a single wide run;
/// twin ink rows only scatter thin strokes).
fn capsule_bounds(rgba8: &[u8]) -> Option<(usize, usize, usize, usize)> {
    let mut x0 = usize::MAX;
    let mut x1 = 0usize;
    let mut y0 = usize::MAX;
    let mut y1 = 0usize;
    for (y, row) in rows(rgba8) {
        // widest contiguous white run on this row
        let mut best = 0usize;
        let mut cur = 0usize;
        for x in 0..W as usize {
            if is_white(px(row, x)) {
                cur += 1;
                best = best.max(cur);
            } else {
                cur = 0;
            }
        }
        if best >= 40 {
            x0 = x0.min((0..W as usize).find(|&x| is_white(px(row, x))).unwrap());
            x1 = x1.max(
                (0..W as usize)
                    .rev()
                    .find(|&x| is_white(px(row, x)))
                    .unwrap(),
            );
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
    }
    (x0 <= x1 && y0 <= y1).then_some((x0, x1, y0, y1))
}

fn row_span<F: Fn([u8; 3]) -> bool>(
    rgba8: &[u8],
    y_band: core::ops::Range<usize>,
    pred: F,
) -> Option<(usize, usize, usize)> {
    let mut x0 = usize::MAX;
    let mut x1 = 0usize;
    let mut n = 0usize;
    for (y, row) in rows(rgba8) {
        if !y_band.contains(&y) {
            continue;
        }
        for x in 0..W as usize {
            if pred(px(row, x)) {
                x0 = x0.min(x);
                x1 = x1.max(x);
                n += 1;
            }
        }
    }
    (n > 0).then_some((x0, x1, n))
}

/// Issue #237: a capsule-clipped text stack must not paint ink outside the
/// frame its measure produced. The clip shape can hide overflow (it trims to
/// exactly the measured bounds), so the check pairs each clipped pill with an
/// *unclipped* twin carrying the same rows in white ink on the canvas: the
/// twin's ink span is the true painted extent and must fit inside the pill's
/// measured bounds.
///
/// `padding_with((4.0, 0.0))` gives the capsule exactly the measured advance
/// horizontally, so `ink ⊆ capsule` is precisely `ink ⊆ advance`.
#[test]
fn painted_ink_stays_within_measured_frame_native_fonts() {
    let builder = AnyViewBuilder::<AnyView>::new(|| {
        let texts = [
            "suggestion 1",
            "WAVy jiggle",
            "clipped pill row",
            "suggestion 4",
        ];
        let pill = VStack::for_each(
            SignalCollection::new(Binding::container(
                (0..texts.len() as u64).map(SelfId::new).collect::<Vec<_>>(),
            )),
            move |id: SelfId<u64>| text(texts[id.into_inner() as usize]).body(),
        )
        .spacing(0.0)
        .padding_with((4.0, 0.0))
        .foreground(Srgb::from_hex("#1B1B1F"))
        .background(Surface)
        .clip(RoundedRectangle::new(6.0));
        // Unclipped twin in white ink, same text and padding: its painted ink
        // extent is directly visible on the blue canvas.
        let twin = VStack::for_each(
            SignalCollection::new(Binding::container(
                (0..texts.len() as u64).map(SelfId::new).collect::<Vec<_>>(),
            )),
            move |id: SelfId<u64>| text(texts[id.into_inner() as usize]).body(),
        )
        .spacing(0.0)
        .padding_with((4.0, 0.0))
        .foreground(Srgb::from_hex("#FFFFFF"));
        AnyView::new(vstack((pill, twin)).background(Srgb::from_hex("#0000FF")))
    });

    let mut runtime = HeadlessRuntime::new_for_tests_native_fonts(
        test_environment(),
        builder,
        W,
        H,
        MinimalTestTheme::default(),
    );
    let at = Instant::now();
    for _ in 0..4 {
        runtime.pump_at(false, at);
    }
    let snapshot = runtime.pump_at(true, at).snapshot.expect("capture");

    let (cx0, cx1, cy0, cy1) = capsule_bounds(&snapshot.rgba8).expect("capsule pill painted");
    let capsule_w = cx1 - cx0 + 1;
    let capsule_h = cy1 - cy0 + 1;
    // Four ~19 pt rows + 8 pt vertical padding: proves the fonts loaded and the
    // rows laid out at their measured heights.
    assert!(
        capsule_w >= 60 && capsule_h >= 60,
        "capsule should wrap all four rows, got {capsule_w}x{capsule_h}"
    );

    // Unclipped twin ink: white pixels strictly below the capsule.
    let (ix0, ix1, ink_px) = row_span(&snapshot.rgba8, cy1 + 4..H as usize, is_white)
        .expect("twin ink painted — native fonts must render the rows");
    assert!(
        ink_px > 200,
        "twin ink too sparse ({ink_px}px): fonts may not have loaded"
    );

    // The invariant: the true painted ink extent fits inside the frame the
    // measure produced (the capsule). 1 px of slack for AA coverage edges.
    assert!(
        ix0 + 1 >= cx0 && ix1 <= cx1 + 1,
        "twin ink x {ix0}..={ix1} escapes measured frame x {cx0}..={cx1}"
    );

    // No ink-colored pixel outside the capsule's measured bounds inside its
    // neighborhood either — ink clipped by the stadium caps may not paint, but
    // none may overpaint the frame.
    let mut escaped = 0usize;
    for (y, row) in rows(&snapshot.rgba8) {
        if y + 4 < cy0 || y > cy1 + 4 {
            continue;
        }
        for x in 0..W as usize {
            let p = px(row, x);
            if is_dark_ink(p) && (x + 1 < cx0 || x > cx1 + 1) {
                escaped += 1;
            }
        }
    }
    assert_eq!(escaped, 0, "glyph ink painted outside the measured frame");
}

/// Same invariant for a single plain `Text` — no collection, no clipping
/// container beyond the capsule itself.
#[test]
fn painted_ink_stays_within_measured_frame_single_text() {
    let builder = AnyViewBuilder::<AnyView>::new(|| {
        AnyView::new(
            vstack((
                text("clipped pill row")
                    .body()
                    .foreground(Srgb::from_hex("#1B1B1F"))
                    .background(Surface)
                    .clip(RoundedRectangle::new(6.0)),
                text("clipped pill row")
                    .body()
                    .foreground(Srgb::from_hex("#FFFFFF")),
            ))
            .background(Srgb::from_hex("#0000FF")),
        )
    });

    let mut runtime = HeadlessRuntime::new_for_tests_native_fonts(
        test_environment(),
        builder,
        W,
        H,
        MinimalTestTheme::default(),
    );
    let at = Instant::now();
    for _ in 0..4 {
        runtime.pump_at(false, at);
    }
    let snapshot = runtime.pump_at(true, at).snapshot.expect("capture");

    let (cx0, cx1, _cy0, cy1) = capsule_bounds(&snapshot.rgba8).expect("capsule painted");
    let (ix0, ix1, _) =
        row_span(&snapshot.rgba8, cy1 + 2..H as usize, is_white).expect("twin ink painted");
    assert!(
        ix0 + 1 >= cx0 && ix1 <= cx1 + 1,
        "twin ink x {ix0}..={ix1} escapes measured frame x {cx0}..={cx1}"
    );
}

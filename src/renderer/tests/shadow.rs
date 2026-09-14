//! Pixel regression for `Metadata<Shadow>` corner geometry.
//!
//! `apply_shadow` rasterizes the shadow as a blurred rounded rect. The rect's
//! corner radius must come from `Shadow::corner_radius` — the caster's shape —
//! not from the blur radius. A rounded surface whose shadow used the blur as
//! its corner radius renders a square-ish blob that pokes into the surface's
//! rounded notch: along the corner diagonal an `R`-radius arc starts covering
//! at `t = R(√2−1)`, so a probe ~4px inside the corner is covered by a small
//! buggy radius but cleanly outside the true 16px arc.

use std::time::Instant;

use waterui::graphics::Color;
use waterui::style::{Shadow, Vector};
use waterui::{Binding, View, ViewExt as _};
use waterui_core::handler::AnyViewBuilder;
use waterui_layout::stack::{hstack, vstack};
use waterui_shape::{FixedRoundedRectangle, ShapeExt as _};

use super::pumped_test_environment;
use crate::HeadlessRuntime;

const SURFACE_RGB: [u8; 3] = [60, 120, 200];

/// A 56×56 surface with 16px corners at (20,20)–(76,76), casting a tight black
/// shadow (blur 1, no offset, corner radius 16).
fn caster() -> impl View {
    ().size(56.0, 56.0)
        .background(
            FixedRoundedRectangle::new(16.0).fill(Color::srgb(
                SURFACE_RGB[0],
                SURFACE_RGB[1],
                SURFACE_RGB[2],
            )),
        )
        .shadow(Shadow::new(
            Color::srgb(0, 0, 0),
            Vector::new(0.0, 0.0),
            1.0,
            16.0,
        ))
}

fn scene() -> impl View {
    vstack((
        ().size(120.0, 20.0),
        hstack((().size(20.0, 56.0), caster())),
    ))
}

fn pixel(snapshot: &crate::runner::HeadlessSnapshot, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * snapshot.width + x) * 4) as usize;
    snapshot.rgba8[offset..offset + 4]
        .try_into()
        .expect("pixel in bounds")
}

#[test]
fn shadow_corner_follows_caster_radius() {
    let value = Binding::container(0_i32);
    let builder = AnyViewBuilder::<waterui_core::AnyView>::new(move || {
        let _ = &value;
        waterui_core::AnyView::new(scene())
    });
    let env = pumped_test_environment();
    let mut runtime = HeadlessRuntime::new_for_tests(env, builder, 120, 120);
    let snapshot = runtime
        .pump_at(true, Instant::now())
        .snapshot
        .expect("frame must produce a snapshot");

    // Control: the top edge of the surface is painted with the surface color.
    let edge = pixel(&snapshot, 48, 21);
    assert_eq!(
        edge[..3],
        SURFACE_RGB,
        "the surface must paint its top edge; got {edge:?}"
    );

    // The probe sits inside the surface rect but outside its 16px corner arc
    // ((20+4.24/√2≈23, 23) is 4.24px along the diagonal; the 16px arc begins
    // covering at 6.6px). The surface is transparent there, so the pixel shows
    // only whatever the shadow paints. With the blur-radius-as-corner-radius
    // bug the shadow's 1px arc already covers 2.1px in, so the probe lands
    // deep inside the shadow and darkens to near-black.
    let notch = pixel(&snapshot, 23, 23);
    let background = pixel(&snapshot, 110, 110);
    let darkening = background[0].abs_diff(notch[0]);
    assert!(
        darkening < 40,
        "corner-notch pixel must stay near the background: the shadow silhouette \
         must follow the caster's 16px corner radius (notch={notch:?} \
         background={background:?} darkening={darkening})"
    );
}

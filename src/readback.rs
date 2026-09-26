//! Surface readback for offscreen export paths.
//!
//! Used only by snapshot/export consumers — the headless runtime's
//! [`HeadlessSnapshot`](crate::HeadlessSnapshot) capture and
//! [`HydrolysisViewRenderer`](crate::HydrolysisViewRenderer)'s
//! `render_to_rgba` — never by the interactive frame loop, which stays
//! GPU-resident end to end.

use cherenkov::{Readback, WorkingColor};
use waterui_graphics::color::working::to_srgb;

/// The pixels of a readable surface after its last render, as
/// straight-alpha gamma-encoded sRGB bytes, row-major.
///
/// # Panics
/// Panics when the surface cannot be read back: only offscreen targets can.
pub(crate) fn readback_rgba8(surface: &cherenkov::Surface<cherenkov_gpu::Gpu>) -> Vec<u8> {
    let readback = surface
        .readback()
        .expect("hydrolysis readback: the surface could not be read back");
    readback_to_rgba8(&readback)
}

/// Converts premultiplied linear working-space pixels to straight-alpha
/// sRGB bytes.
pub(crate) fn readback_to_rgba8(readback: &Readback) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(readback.pixels.len() * 4);
    for &[red, green, blue, alpha] in &readback.pixels {
        let alpha = alpha.clamp(0.0, 1.0);
        let straight = if alpha > 0.0 {
            [red / alpha, green / alpha, blue / alpha]
        } else {
            [0.0, 0.0, 0.0]
        };
        let srgb = to_srgb(WorkingColor::new([straight[0], straight[1], straight[2], alpha]));
        bytes.extend_from_slice(&[
            encode(srgb.red),
            encode(srgb.green),
            encode(srgb.blue),
            encode(alpha),
        ]);
    }
    bytes
}

fn encode(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

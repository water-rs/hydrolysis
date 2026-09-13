//! Deterministic environment setup for Hydrolysis-backed tests.

use waterui::{
    Environment, Plugin,
    color::{ResolvedColor, Srgb},
    theme::{ColorScheme, ColorSettings, FontSettings, Theme},
};

fn color(rgb: u32) -> ResolvedColor {
    ResolvedColor::from_srgb(Srgb::from_u32(rgb))
}

/// Installs every theme token required by Hydrolysis rendering.
pub fn install_theme(env: &mut Environment) {
    Theme::new()
        .color_scheme(ColorScheme::Light)
        .colors(
            ColorSettings::new()
                .background(color(0xFF_FF_FF))
                .surface(color(0xFF_FF_FF))
                .surface_variant(color(0xF3_F4_F6))
                .border(color(0xD1_D5_DB))
                .foreground(color(0x11_18_27))
                .muted_foreground(color(0x4B_55_63))
                .accent(color(0x25_63_EB))
                .accent_container(color(0xDB_EA_FE))
                .accent_foreground(color(0xFF_FF_FF))
                .tertiary(color(0x7C_3A_ED))
                .tertiary_container(color(0xED_E9_FE))
                .selection_container(color(0x25_63_EB))
                .selection_foreground(color(0xFF_FF_FF)),
        )
        .fonts(FontSettings::default_scale())
        .install(env);
}

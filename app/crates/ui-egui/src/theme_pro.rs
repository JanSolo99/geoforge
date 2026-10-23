//! UniGIS design tokens, "Pro" palette: neutral greys and an Esri-style blue accent, plus the
//! spacing, radius and type scale from `docs/DESIGN.md`. In use unless the classic theme is chosen
//! (`theme::pro`). Colours are approximations of ArcGIS Pro's
//! look from public screenshots and documentation, not measured values.
//! Added by UniGIS.

use crate::theme::{CONTEXT, Tokens};
use egui::Color32;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Spacing scale (points). Use these instead of literals.
pub mod space {
    pub const XS: f32 = 2.0;
    pub const S: f32 = 4.0;
    pub const M: f32 = 8.0;
    pub const L: f32 = 12.0;
    pub const XL: f32 = 16.0;
}

/// Corner radii (points). Pro is nearly square: 2 for controls, 4 for popups, 0 for panes.
pub mod radius {
    pub const NONE: u8 = 0;
    pub const CONTROL: u8 = 2;
    pub const POPUP: u8 = 4;
}

/// Type scale (points). Pro uses Segoe UI 9 pt body, about 12 px; we use the system UI face.
pub mod type_size {
    pub const SMALL: f32 = 10.5;
    pub const BODY: f32 = 12.0;
    pub const PANE_TITLE: f32 = 14.0;
    pub const HEADING: f32 = 16.0;
}

/// Esri-style blue accent.
pub const PRO_BLUE: Color32 = rgb(0x0079C1);

pub fn light() -> Tokens {
    Tokens {
        dark: false,
        header: rgb(0xF1F1F1),
        ribbon: rgb(0xFAFAFA),
        tab_strip: rgb(0xF1F1F1),
        canvas: rgb(0xD9D9D9),
        pane: rgb(0xFFFFFF),
        pane_header: rgb(0xF1F1F1),
        border: rgb(0xD5D5D5),
        border_strong: rgb(0xB5B5B5),
        text: rgb(0x2B2B2B),
        text_dim: rgb(0x595959),
        text_disabled: rgb(0xA6A6A6),
        icon: rgb(0x4A4A4A),
        hover: rgb(0xE5F1F9),
        pressed: rgb(0xC7E0F4),
        checked: rgb(0xD6E8F5),
        accent: PRO_BLUE,
        accent_text: rgb(0x00659F),
        on_accent: Color32::WHITE,
        input: Color32::WHITE,
        input_border: rgb(0xB5B5B5),
        group_label: rgb(0x6B6B6B),
        status_bar: rgb(0xF1F1F1),
        selection: rgb(0xC7E0F4),
        menu: Color32::WHITE,
        shadow: Color32::from_black_alpha(36),
        green: rgb(0x2E8B3D),
        red: rgb(0xC62828),
        amber: rgb(0xD98E04),
        context: CONTEXT,
        context_tint: rgb(0xE3F5F2),
    }
}

pub fn dark() -> Tokens {
    Tokens {
        dark: true,
        header: rgb(0x262626),
        ribbon: rgb(0x2F2F2F),
        tab_strip: rgb(0x262626),
        canvas: rgb(0x1B1B1B),
        pane: rgb(0x303030),
        pane_header: rgb(0x383838),
        border: rgb(0x454545),
        border_strong: rgb(0x5E5E5E),
        text: rgb(0xEDEDED),
        text_dim: rgb(0xB3B3B3),
        text_disabled: rgb(0x707070),
        icon: rgb(0xD9D9D9),
        hover: rgb(0x3D4A55),
        pressed: rgb(0x2A4A63),
        checked: rgb(0x2B4C66),
        accent: rgb(0x3AA0E0),
        accent_text: rgb(0x6DBDEE),
        on_accent: Color32::BLACK,
        input: rgb(0x242424),
        input_border: rgb(0x5E5E5E),
        group_label: rgb(0xA0A0A0),
        status_bar: rgb(0x262626),
        selection: rgb(0x2A4A63),
        menu: rgb(0x383838),
        shadow: Color32::from_black_alpha(120),
        green: rgb(0x4CC25D),
        red: rgb(0xF06B6B),
        amber: rgb(0xF2B848),
        context: rgb(0x2CC2AE),
        context_tint: rgb(0x1B3331),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(c: u8) -> f32 {
        let s = c as f32 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    }
    fn lum(c: Color32) -> f32 {
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }
    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn text_is_readable_on_every_surface() {
        for t in [light(), dark()] {
            for bg in [t.header, t.ribbon, t.pane, t.pane_header, t.status_bar, t.menu, t.input, t.selection, t.hover] {
                assert!(contrast(t.text, bg) >= 6.5, "text on {bg:?} dark={}", t.dark);
                if bg != t.selection && bg != t.hover {
                    assert!(contrast(t.text_dim, bg) >= 4.5, "dim text on {bg:?} dark={}", t.dark);
                }
            }
            assert!(contrast(t.accent_text, t.pane) >= 4.5, "accent text dark={}", t.dark);
            assert!(contrast(t.on_accent, t.accent) >= 4.5, "text on accent dark={}", t.dark);
        }
    }

    #[test]
    fn light_accent_is_esri_blue_and_themes_differ() {
        assert_eq!(light().accent, PRO_BLUE);
        assert!(!light().dark && dark().dark);
        assert!(lum(dark().pane) < lum(light().pane));
    }

    #[test]
    fn spacing_scale_is_monotonic() {
        let s = [space::XS, space::S, space::M, space::L, space::XL];
        assert!(s.windows(2).all(|w| w[0] < w[1]));
    }
}

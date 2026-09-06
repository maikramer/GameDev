//! Resolution-independent HUD size.
//!
//! Every number in the stylesheets is a logical pixel authored against a 720p
//! reference frame. Left alone, that HUD is comfortable at 1280×720 and turns
//! into a row of ants on a 4K panel. Bevy's [`UiScale`] multiplies the whole UI
//! tree — declarative HUD, menus and the native widgets alike — so one factor
//! per frame is all the scaling this engine needs.
//!
//! The factor is deliberately *sub-linear*: doubling the vertical resolution
//! does not double the HUD, because a bigger screen is also a screen you want
//! more world on. `GAIN` is how much of the resolution change the HUD follows.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Vertical resolution the stylesheets are authored against.
pub const REFERENCE_HEIGHT: f32 = 720.0;
/// Fraction of the resolution change the HUD follows (1 = fully proportional).
pub const GAIN: f32 = 0.62;
/// Clamp: below this the HUD stops shrinking, above it stops growing.
pub const MIN_SCALE: f32 = 0.72;
pub const MAX_SCALE: f32 = 2.0;

/// HUD scale factor for a window height.
pub fn hud_scale_for_height(height: f32) -> f32 {
    if height <= 0.0 {
        return 1.0;
    }
    let ratio = height / REFERENCE_HEIGHT;
    (1.0 + (ratio - 1.0) * GAIN).clamp(MIN_SCALE, MAX_SCALE)
}

/// Portrait windows must not enlarge a HUD that already has less horizontal room.
pub fn hud_scale_for_window(width: f32, height: f32) -> f32 {
    let by_height = hud_scale_for_height(height);
    if width > 0.0 {
        by_height.min((width / 720.0).clamp(MIN_SCALE, MAX_SCALE))
    } else {
        by_height
    }
}

/// A janela no **espaço autoral**: os píxeis que o CSS escreve, já divididos
/// pela escala do HUD. É o ÚNICO espaço da UI — píxeis autorais, unidades de
/// viewport (`vw`/`vh`/`vmin`/`vmax`) e media queries avaliam todos nele, como
/// num browser, onde o CSS só tem um espaço. Um `width: 426` e um
/// `@media (max-width: 900)` falam por isso do mesmo metro.
pub fn ui_viewport(width: f32, height: f32, scale: f32) -> (f32, f32) {
    if scale > 1e-3 {
        (width / scale, height / scale)
    } else {
        (width, height)
    }
}

/// Keeps [`UiScale`] in step with the primary window.
pub fn sync_ui_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
    mut minimaps: Query<&mut Node, With<crate::hud::minimap::MinimapRange>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let (width, height) = (window.resolution.width(), window.resolution.height());
    let wanted = hud_scale_for_window(width, height);
    if (scale.0 - wanted).abs() > 1e-3 {
        scale.0 = wanted;
    }
    // A faixa inferior fica reservada às ações nas janelas estreitas — medida
    // no mesmo espaço autoral que as media queries do CSS (ver ui_viewport).
    let bottom = Val::Px(minimap_bottom(ui_viewport(width, height, wanted).0));
    for mut map in &mut minimaps {
        if map.bottom != bottom {
            map.bottom = bottom;
        }
    }
}

fn minimap_bottom(ui_width: f32) -> f32 {
    if ui_width <= 900.0 { 142.0 } else { 18.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_scale_is_bounded_by_width() {
        assert!(hud_scale_for_window(600.0, 1000.0) < 1.0);
        assert_eq!(
            hud_scale_for_window(1920.0, 1080.0),
            hud_scale_for_height(1080.0)
        );
        assert_eq!(hud_scale_for_window(0.0, 720.0), 1.0);
    }

    #[test]
    fn ui_viewport_is_the_authored_pixel_space() {
        assert_eq!(ui_viewport(1280.0, 720.0, 1.0), (1280.0, 720.0));
        // Retrato 1000×1600: a escala limita pela largura e o espaço autoral
        // fica estreito (720×1152) — é nele que `max-width: 900` avaliará.
        let (w, h) = ui_viewport(1000.0, 1600.0, hud_scale_for_window(1000.0, 1600.0));
        assert!((w - 720.0).abs() < 0.5, "ui width {w}");
        assert!((h - 1152.0).abs() < 1.0, "ui height {h}");
        assert_eq!(
            ui_viewport(600.0, 1000.0, 0.0),
            (600.0, 1000.0),
            "degenerate scale"
        );
    }

    #[test]
    fn narrow_minimap_reserves_the_action_band() {
        assert_eq!(minimap_bottom(1280.0), 18.0);
        assert_eq!(minimap_bottom(720.0), 142.0);
        let ui = ui_viewport(1000.0, 1600.0, hud_scale_for_window(1000.0, 1600.0));
        assert_eq!(minimap_bottom(ui.0), 142.0);
    }

    #[test]
    fn test_reference_resolution_is_unscaled() {
        assert!((hud_scale_for_height(REFERENCE_HEIGHT) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_scale_follows_resolution_sub_linearly() {
        let hd = hud_scale_for_height(1080.0);
        // 1.5x the pixels, but not 1.5x the HUD.
        assert!(hd > 1.0 && hd < 1.5, "1080p scale = {hd}");
        let uhd = hud_scale_for_height(2160.0);
        assert!(uhd > hd);
        // Small windows shrink but never vanish.
        let tiny = hud_scale_for_height(360.0);
        assert!(tiny >= MIN_SCALE && tiny < 1.0, "360p scale = {tiny}");
    }

    #[test]
    fn test_degenerate_heights_do_not_produce_a_zero_hud() {
        assert_eq!(hud_scale_for_height(0.0), 1.0);
        assert_eq!(hud_scale_for_height(-100.0), 1.0);
        assert!(hud_scale_for_height(100_000.0) <= MAX_SCALE);
    }
}

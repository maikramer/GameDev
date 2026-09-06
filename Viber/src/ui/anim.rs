//! Declarative motion: `anim="spin 2"` on any element.
//!
//! A HUD lives and dies by small movements — a loader that spins, a quest
//! marker that pulses, a heart that bobs, a panel that shakes when the hero is
//! hit. Instead of a tween engine, four *kinds* of looping motion cover the
//! vocabulary a game HUD actually uses, each one number away from its default:
//!
//! | spec | efeito | números |
//! |------|--------|---------|
//! | `spin`   | rotação contínua              | segundos/volta (2) |
//! | `pulse`  | escala sobe e desce           | período (1.2), amplitude (0.08) |
//! | `bob`    | flutua para cima e para baixo | período (2.0), amplitude px (6) |
//! | `shake`  | tremor curto e nervoso        | período (0.4), amplitude px (3) |
//!
//! The animation drives [`UiTransform`] directly and runs AFTER the style
//! pass, so it wins over `rotate` / `scale` / `translate` from the stylesheet
//! for the element it is on. It also ignores (and is ignored by) the cascade —
//! motion is not paint.

use bevy::prelude::*;

/// Looping motion on one element, parsed from the `anim="…"` attribute.
#[derive(Debug, Clone, Component, PartialEq)]
pub struct UiAnim {
    pub kind: AnimKind,
    /// Seconds per cycle.
    pub period: f32,
    /// Kind-dependent amplitude: scale delta for `pulse`, pixels for `bob` and
    /// `shake`, unused for `spin`.
    pub amount: f32,
    /// Seconds elapsed, for the phase of the oscillation.
    t: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimKind {
    Spin,
    Pulse,
    Bob,
    Shake,
}

impl UiAnim {
    /// `spin`, `pulse 1.5`, `bob 2 8` — kind first, then period and amount.
    pub fn parse(spec: &str) -> Option<Self> {
        let mut words = spec.split_whitespace();
        let kind = match words.next()?.to_ascii_lowercase().as_str() {
            "spin" => AnimKind::Spin,
            "pulse" => AnimKind::Pulse,
            "bob" => AnimKind::Bob,
            "shake" => AnimKind::Shake,
            _ => return None,
        };
        let (default_period, default_amount) = match kind {
            AnimKind::Spin => (2.0, 0.0),
            AnimKind::Pulse => (1.2, 0.08),
            AnimKind::Bob => (2.0, 6.0),
            AnimKind::Shake => (0.4, 3.0),
        };
        let mut numbers = words
            .filter_map(|w| w.parse::<f32>().ok())
            .filter(|n| n.is_finite());
        let period = numbers.next().unwrap_or(default_period).max(1e-3);
        let amount = numbers.next().unwrap_or(default_amount);
        Some(Self {
            kind,
            period,
            amount,
            t: 0.0,
        })
    }

    /// Advances the phase and writes the transform for this frame.
    pub fn step(&mut self, dt: f32) -> (Rot2, f32, Vec2) {
        self.t = (self.t + dt) % self.period;
        let phase = self.t / self.period * std::f32::consts::TAU;
        match self.kind {
            AnimKind::Spin => (Rot2::radians(phase), 1.0, Vec2::ZERO),
            // A sine keeps pulse smooth at both ends; the delta is symmetric
            // around 1, so `pulse 1.2 0.08` breathes between 0.92 and 1.08.
            AnimKind::Pulse => (Rot2::IDENTITY, 1.0 + phase.sin() * self.amount, Vec2::ZERO),
            AnimKind::Bob => (Rot2::IDENTITY, 1.0, Vec2::Y * phase.sin() * self.amount),
            // A shake is two out-of-phase sines — jitter, not a loop anyone
            // should be able to name after watching it twice.
            AnimKind::Shake => (
                Rot2::IDENTITY,
                1.0,
                Vec2::new(phase.sin(), (phase * 1.7).sin()) * self.amount,
            ),
        }
    }
}

/// Advances every animation and writes its [`UiTransform`].
///
/// Runs last in [`crate::ui::UiSet::Style`]: the cascade may legitimately set
/// a rotation on the same element, and the animation is the more specific
/// intent, so it gets the last word.
pub fn drive_ui_anims(time: Res<Time>, mut anims: Query<(&mut UiAnim, &mut UiTransform)>) {
    let dt = time.delta_secs();
    for (mut anim, mut transform) in &mut anims {
        let (rotation, scale, translation) = anim.step(dt);
        transform.rotation = rotation;
        transform.scale = Vec2::splat(scale);
        // `UiTransform.translation` is a `Val2`; the animation speaks pixels.
        transform.translation = bevy::ui::Val2::px(translation.x, translation.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_reads_kind_defaults_and_numbers() {
        let spin = UiAnim::parse("spin").expect("spin");
        assert_eq!(spin.kind, AnimKind::Spin);
        assert!((spin.period - 2.0).abs() < 1e-6);
        let pulse = UiAnim::parse("pulse 1.5 0.2").expect("pulse");
        assert_eq!(pulse.kind, AnimKind::Pulse);
        assert!((pulse.period - 1.5).abs() < 1e-6);
        assert!((pulse.amount - 0.2).abs() < 1e-6);
        assert!(UiAnim::parse("swim").is_none());
        assert!(UiAnim::parse("").is_none());
        // Um período zero ou negativo não pode dividir por zero no passo.
        assert!(UiAnim::parse("spin 0").is_some());
        // NaN/inf (`1e400` também é inf) caem no omissão — não chegam ao
        // UiTransform como escala/fase não finita.
        let clean = UiAnim::parse("pulse 1 nan").expect("pulse");
        assert!((clean.amount - 0.08).abs() < 1e-6);
        let clean = UiAnim::parse("spin 1e400").expect("spin");
        assert!((clean.period - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_spin_rotates_continuously_and_wraps() {
        let mut spin = UiAnim::parse("spin 4").expect("spin");
        let (rotation, _, _) = spin.step(1.0);
        assert!((rotation.as_radians() - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        for _ in 0..10 {
            spin.step(1.0);
        }
        // `t` mantém-se dentro do período — a fase nunca cresce sem limite.
        assert!(spin.t < spin.period);
    }

    #[test]
    fn test_pulse_oscillates_around_one() {
        let mut pulse = UiAnim::parse("pulse 1 0.1").expect("pulse");
        let mut min: f32 = f32::MAX;
        let mut max: f32 = f32::MIN;
        for _ in 0..100 {
            let (_, scale, _) = pulse.step(0.01);
            min = min.min(scale);
            max = max.max(scale);
        }
        // Folga de float: as amostras passam quase exactamente pelos picos.
        assert!(min < 0.925 && max > 1.075, "min={min} max={max}");
    }

    #[test]
    fn test_bob_moves_only_vertically() {
        let mut bob = UiAnim::parse("bob 2 6").expect("bob");
        let mut saw_x: f32 = 0.0;
        let mut saw_y: f32 = 0.0;
        for _ in 0..60 {
            let (_, _, translation) = bob.step(0.033);
            saw_x = saw_x.max(translation.x.abs());
            saw_y = saw_y.max(translation.y.abs());
        }
        assert_eq!(saw_x, 0.0);
        assert!(saw_y > 3.0, "amplitude 6 tem de se ver: y={saw_y}");
    }
}

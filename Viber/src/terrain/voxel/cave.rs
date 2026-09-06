//! `<Cave>` — a tunnel bored through the terrain.
//!
//! The first world feature that a heightfield cannot express at all. A cave is
//! not a carve: it is a chain of subtractive [`CapsuleMod`]s that removes rock
//! the base field says is solid, leaving a roof over the player's head.
//!
//! # Authoring
//!
//! ```xml
//! <Cave name="mina" path="-40 10  0 4  38 -6" radius="3.5" depth="10" />
//! ```
//!
//! `path` is XZ, like every other ground feature — the tunnel's height comes
//! from the terrain, not from the author, so a cave drawn across a hillside
//! follows it instead of needing hand-placed Y at every bend.
//!
//! `depth` is how far the tube centre sits **below the surface**. With
//! `open-ends` (the default) that depth ramps to zero at both ends, so the
//! tube breaks out of the hillside and the cave is enterable. A cave at a
//! constant depth is sealed rock with a hole inside it that nobody can reach.

use bevy::math::{Vec2, Vec3};

use super::super::mesh::HeightField;
use super::super::paths::{chaikin_smooth, resample};
use super::mods::{CapsuleMod, ModOp, VoxelMod};

/// Default tunnel radius (meters) — wide enough to walk through.
pub const DEFAULT_CAVE_RADIUS: f32 = 3.0;
/// Default depth of the tube centre below the surface (meters).
pub const DEFAULT_CAVE_DEPTH: f32 = 8.0;
/// Spacing between capsule stations (meters).
///
/// Short enough that a bend reads as a curve, long enough that a 100 m cave is
/// ~25 mods rather than hundreds — the index buckets them per column, so this
/// is what a density query inside the cave actually pays for.
const STATION_SPACING: f32 = 4.0;
/// Fraction of the cave length over which the ends ramp up to the surface.
const MOUTH_FRACTION: f32 = 0.18;

/// Declarative `<Cave>` spec.
#[derive(Debug, Clone, PartialEq)]
pub struct CaveSpec {
    pub name: Option<String>,
    /// Tunnel centreline in world XZ.
    pub path: Vec<Vec2>,
    /// Tube radius (meters).
    pub radius: f32,
    /// Depth of the tube centre below the terrain surface (meters).
    pub depth: f32,
    /// Ramp the ends up to the surface so the cave has mouths.
    pub open_ends: bool,
}

impl Default for CaveSpec {
    fn default() -> Self {
        Self {
            name: None,
            path: Vec::new(),
            radius: DEFAULT_CAVE_RADIUS,
            depth: DEFAULT_CAVE_DEPTH,
            open_ends: true,
        }
    }
}

impl CaveSpec {
    /// Depth of the tube centre at path fraction `t` (0..1).
    ///
    /// Flat in the middle, ramping to zero at each mouth. `smoothstep` rather
    /// than a straight ramp so the mouth flares instead of ending in a cone.
    fn depth_at(&self, t: f32) -> f32 {
        if !self.open_ends {
            return self.depth;
        }
        let m = MOUTH_FRACTION.clamp(1e-3, 0.5);
        let edge = t.min(1.0 - t) / m;
        if edge >= 1.0 {
            return self.depth;
        }
        let s = edge.clamp(0.0, 1.0);
        self.depth * (s * s * (3.0 - 2.0 * s))
    }

    /// Builds the subtractive mods for this cave against the carved terrain.
    ///
    /// Takes the height field because the tunnel follows the ground: this runs
    /// in the bootstrap **after** pads/lakes/rivers/roads have carved, so a
    /// cave under a road bed sits under the road bed as built, not under the
    /// terrain as generated.
    pub fn build(&self, base: &dyn HeightField) -> Vec<Box<dyn VoxelMod>> {
        if self.path.len() < 2 || self.radius <= 0.0 || !self.radius.is_finite() {
            return Vec::new();
        }
        // Same treatment every other path feature gets: smooth the authored
        // polyline, then walk it at a fixed spacing.
        let smoothed = chaikin_smooth(&self.path, 2, false);
        let stations = resample(&smoothed, STATION_SPACING);
        if stations.len() < 2 {
            return Vec::new();
        }

        let label = self.name.clone().unwrap_or_else(|| "cave".to_string());
        let last = (stations.len() - 1) as f32;
        let centre = |i: usize| -> Vec3 {
            let p = stations[i];
            let t = i as f32 / last;
            Vec3::new(p.x, base.sample(p.x, p.y) - self.depth_at(t), p.y)
        };

        let mut mods: Vec<Box<dyn VoxelMod>> = Vec::with_capacity(stations.len());
        for i in 0..stations.len() - 1 {
            mods.push(Box::new(CapsuleMod::new(
                format!("{label}:{i}"),
                centre(i),
                centre(i + 1),
                self.radius,
                ModOp::Subtract,
            )));
        }
        mods
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Flat(f32);

    impl HeightField for Flat {
        fn sample(&self, _x: f32, _z: f32) -> f32 {
            self.0
        }
        fn sample_normal(&self, _x: f32, _z: f32, _e: f32) -> Vec3 {
            Vec3::Y
        }
        fn max_height(&self) -> f32 {
            self.0
        }
    }

    fn spec() -> CaveSpec {
        CaveSpec {
            name: Some("mina".into()),
            path: vec![
                Vec2::new(-40.0, 0.0),
                Vec2::new(0.0, 0.0),
                Vec2::new(40.0, 0.0),
            ],
            radius: 3.0,
            depth: 10.0,
            open_ends: true,
        }
    }

    #[test]
    fn test_a_short_or_degenerate_path_builds_nothing() {
        let mut s = spec();
        s.path = vec![Vec2::ZERO];
        assert!(s.build(&Flat(50.0)).is_empty());
        let mut s = spec();
        s.radius = 0.0;
        assert!(s.build(&Flat(50.0)).is_empty());
    }

    #[test]
    fn test_the_tunnel_is_subtractive_and_follows_the_path() {
        let mods = spec().build(&Flat(50.0));
        assert!(mods.len() > 10, "80 m at 4 m spacing, got {}", mods.len());
        assert!(mods.iter().all(|m| m.op() == ModOp::Subtract));
        // The whole chain stays inside the authored XZ corridor.
        for m in &mods {
            let b = m.bounds();
            assert!(b.min.x >= -50.0 && b.max.x <= 50.0, "strayed: {b:?}");
        }
    }

    #[test]
    fn test_open_ends_bring_the_mouths_up_to_the_surface() {
        let mods = spec().build(&Flat(50.0));
        let first = mods.first().unwrap().bounds();
        let last = mods.last().unwrap().bounds();
        // A mouth reaches the surface (ground 50, radius 3): its box must
        // clear the ground, otherwise the cave is a sealed bubble.
        assert!(
            first.max.y >= 50.0,
            "entrance is buried: {first:?} — the cave would be unreachable"
        );
        assert!(last.max.y >= 50.0, "exit is buried: {last:?}");
    }

    #[test]
    fn test_the_middle_of_the_cave_stays_at_the_authored_depth() {
        let s = spec();
        assert!((s.depth_at(0.5) - 10.0).abs() < 1e-6);
        assert_eq!(s.depth_at(0.0), 0.0, "the mouth is at the surface");
        assert_eq!(s.depth_at(1.0), 0.0);
        // Monotone ramp out of the mouth.
        assert!(s.depth_at(0.05) < s.depth_at(0.12));
    }

    #[test]
    fn test_closed_ends_keep_the_full_depth_everywhere() {
        let mut s = spec();
        s.open_ends = false;
        assert_eq!(s.depth_at(0.0), 10.0);
        assert_eq!(s.depth_at(0.5), 10.0);
        let mods = s.build(&Flat(50.0));
        let first = mods.first().unwrap().bounds();
        assert!(
            first.max.y < 50.0,
            "a closed cave must stay buried, got {first:?}"
        );
    }

    #[test]
    fn test_build_is_deterministic() {
        let a = spec().build(&Flat(50.0));
        let b = spec().build(&Flat(50.0));
        let bounds = |m: &Vec<Box<dyn VoxelMod>>| m.iter().map(|x| x.bounds()).collect::<Vec<_>>();
        assert_eq!(bounds(&a), bounds(&b));
    }
}

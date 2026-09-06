//! `<Arch>` — a free-standing rock portal (`src/terrain/voxel/mods.rs::
//! ArchMod`).
//!
//! Like `<Cave>`, this never touches the heightfield: the arch is a union
//! solid in the voxel field, which is the only representation that can hold
//! rock above and air below at the same XZ. The whole point of the tag is
//! the walk-through: a column under the opening resolves to TWO spans
//! (ground + the arch band), and gameplay asks
//! [`super::field::VoxelField::surface_below`] which of them to stand on.

use bevy::math::{Vec2, Vec3};

use super::super::mesh::HeightField;
use super::mods::{ArchMod, ModOp, VoxelMod};

/// Default clear span of the opening (meters).
pub const DEFAULT_ARCH_SPAN: f32 = 8.0;
/// Default clear opening height at the crown (meters).
pub const DEFAULT_ARCH_HEIGHT: f32 = 6.0;
/// Default leg thickness beyond the opening on each side (meters).
pub const DEFAULT_ARCH_THICKNESS: f32 = 2.5;
/// Default block depth along the arch axis (meters).
pub const DEFAULT_ARCH_DEPTH: f32 = 4.0;

/// Declarative `<Arch>` spec.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchSpec {
    pub name: Option<String>,
    /// Centre of the block footprint, world XZ.
    pub at: Vec2,
    /// Clear opening width (meters).
    pub span: f32,
    /// Clear opening height at the crown (meters).
    pub height: f32,
    /// Leg thickness beyond the opening on each side (meters).
    pub thickness: f32,
    /// Block depth along the arch axis (meters).
    pub depth: f32,
    /// Rotation of the arch axis about the centre, degrees.
    pub yaw: f32,
}

impl Default for ArchSpec {
    fn default() -> Self {
        Self {
            name: None,
            at: Vec2::ZERO,
            span: DEFAULT_ARCH_SPAN,
            height: DEFAULT_ARCH_HEIGHT,
            thickness: DEFAULT_ARCH_THICKNESS,
            depth: DEFAULT_ARCH_DEPTH,
            yaw: 0.0,
        }
    }
}

impl ArchSpec {
    /// Builds the union mods for this arch against the carved terrain.
    ///
    /// The base comes from the heightfield at `at` minus a metre of embed,
    /// exactly the treatment every ground-anchored feature gets — and like
    /// every mod build, it reads the grid up front and never again.
    pub fn build(&self, base: &dyn HeightField) -> Vec<Box<dyn VoxelMod>> {
        if self.span <= 0.0
            || self.height <= 0.0
            || self.thickness <= 0.0
            || self.depth <= 0.0
            || !self.span.is_finite()
            || !self.height.is_finite()
            || !self.thickness.is_finite()
            || !self.depth.is_finite()
        {
            return Vec::new();
        }
        let span_half = self.span * 0.5;
        // Jamb height = total opening minus the rounded crown. A span wider
        // than the opening is tall turns the arch into a dome — two clamps
        // keep the contract: the jamb keeps at least a token straight section,
        // and the OPENING is clamped to 3/4 of the height so the crown's apex
        // (jamb + span half) lands at the authored `height` instead of poking
        // above it. Legs keep their full authored thickness either way.
        let jamb = (self.height - span_half).max(self.height * 0.25);
        let span_half_eff = span_half.min(self.height * 0.75);
        // Rock cap above the crown, proportionate to the legs.
        let cap = self.thickness * 0.8;
        let ground = base.sample(self.at.x, self.at.y);
        let label = self.name.clone().unwrap_or_else(|| "arch".to_string());
        vec![Box::new(ArchMod::new(
            label,
            Vec3::new(self.at.x, ground - 1.0, self.at.y),
            span_half_eff,
            jamb,
            self.thickness,
            self.depth,
            cap,
            self.yaw.to_radians(),
            ModOp::Union,
        ))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat;

    impl HeightField for Flat {
        fn sample(&self, _x: f32, _z: f32) -> f32 {
            10.0
        }
        fn sample_normal(&self, _x: f32, _z: f32, _e: f32) -> Vec3 {
            Vec3::Y
        }
        fn max_height(&self) -> f32 {
            10.0
        }
    }

    #[test]
    fn test_arch_builds_one_union_mod_on_the_ground() {
        let spec = ArchSpec::default();
        let mods = spec.build(&Flat);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].op(), ModOp::Union);
        let b = mods[0].bounds();
        // Base sits 1 m under ground (10), block ~9 m tall: bounds straddle.
        assert!(b.min.y < 10.0 && b.max.y > 10.0);
    }

    #[test]
    fn test_invalid_specs_build_nothing() {
        let mut spec = ArchSpec::default();
        spec.span = 0.0;
        assert!(spec.build(&Flat).is_empty());
        spec.span = f32::NAN;
        assert!(spec.build(&Flat).is_empty());
    }

    #[test]
    fn test_a_span_taller_than_the_arch_stays_under_the_authored_height() {
        // span 10 vs height 6: the naive apex (jamb 1.5 + span half 5) would
        // poke 6.5 m above the base — above the authored `height`. The
        // opening is clamped to 3/4 of the height so the crown lands exactly
        // at 6.
        let spec = ArchSpec {
            span: 10.0,
            height: 6.0,
            ..ArchSpec::default()
        };
        let mods = spec.build(&Flat);
        let field = super::super::field::VoxelField::new(mods, 256.0, 64.0);
        // Base sits at ground 10 − 1 = 9; crown apex = 9 + jamb 1.5 + 4.5 = 15.
        let under_crown = field.density(&Flat, Vec3::new(0.0, 14.7, 0.0));
        assert!(under_crown > 0.0, "just under the crown must still be air");
        let above_crown = field.density(&Flat, Vec3::new(0.0, 15.3, 0.0));
        assert!(
            above_crown < 0.0,
            "rock must start at the authored height (apex 15) — the opening \
             poked above it ({under_crown} at 14.7, {above_crown} at 15.3)"
        );
    }

    #[test]
    fn test_opening_column_has_two_spans_of_air_between() {
        let spec = ArchSpec::default();
        let mods = spec.build(&Flat);
        let field = super::super::field::VoxelField::new(mods, 256.0, 64.0);
        let at = spec.at;
        // Through the opening: ground at 10, arch band above. Top of the
        // column is the arch crown, NOT the flat 10.
        let top = field.surface_top(&Flat, at.x, at.y);
        assert!(top > 12.0, "crown must stand above the flat ground: {top}");
        // And the ground right below the walker is still the flat 10.
        let ground = field
            .surface_below(&Flat, at.x, at.y, top - 0.5)
            .expect("solid below the crown");
        assert!(ground < 11.0, "walker's floor stays the ground: {ground}");
    }
}

//! Solids contributed to the voxel field — the 3D counterpart of a carve.
//!
//! A heightfield carve is destructive: [`super::super::cliffs::carve_cliff`]
//! writes a steeper gradient into the shared `u16` grid and the fact that a
//! wall *is* a wall survives only as a statistical property of the numbers.
//! Six downstream systems then have to re-derive it (`CliffMask`'s
//! erode/dilate/BFS pipeline).
//!
//! A [`VoxelMod`] is the opposite: the solid stays an **object** for the whole
//! life of the world. It answers a signed distance, it knows its own bounds,
//! and it says how it combines with what is under it. Nothing is thrown away,
//! so nothing has to be guessed back.
//!
//! # Sign convention
//!
//! **Negative is solid.** `distance(p) < 0` means `p` is inside the rock.
//! This matches the base term `p.y - height(p.xz)`, which is negative below
//! ground.

use bevy::math::Vec3;

/// World-space axis-aligned bounds (meters).
///
/// A tiny local type rather than `bevy::math::bounding::Aabb3d`: that one is
/// `Vec3A`-based and every query here would pay a conversion at the call site
/// for no gain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds3 {
    /// Bounds from any two opposite corners.
    pub fn from_corners(a: Vec3, b: Vec3) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }

    /// Empty bounds that contain nothing and absorb any union.
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    /// Smallest bounds containing both.
    pub fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Grows the box by `m` meters on every side.
    pub fn expanded(self, m: f32) -> Self {
        Self {
            min: self.min - Vec3::splat(m),
            max: self.max + Vec3::splat(m),
        }
    }

    /// True when the boxes share any volume (touching faces count).
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// True when the boxes overlap when projected onto the XZ plane.
    ///
    /// The chunk classifier uses this: a mod that lives above or below a
    /// terrain column still makes that column volumetric.
    pub fn intersects_xz(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Distance from `p` to the box; 0 inside.
    pub fn distance_to(&self, p: Vec3) -> f32 {
        let d = (self.min - p).max(p - self.max).max(Vec3::ZERO);
        d.length()
    }
}

/// How a mod combines with the field accumulated beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModOp {
    /// Adds rock: `d = min(d, mod)`. Cliff bodies, talus cones, arch legs.
    Union,
    /// Removes rock: `d = max(d, -mod)`. Cave tubes, undercuts, arch spans.
    Subtract,
}

/// A solid that contributes to the voxel field.
///
/// Implementors must be **deterministic**: the same world authored twice has
/// to produce byte-identical geometry. That is the same contract the current
/// cliff carve keeps by probing the grid *before* opening its stroke
/// (`cliffs.rs:374-388`) — here it is structural, because a mod never reads
/// the mutable grid at all.
pub trait VoxelMod: Send + Sync + std::fmt::Debug {
    /// Signed distance in meters at a world point; negative inside the solid.
    ///
    /// It does not have to be a metrically exact distance — surface nets only
    /// needs the sign and a well-behaved zero crossing — but it must be
    /// continuous, and it must not lie about the sign inside [`Self::bounds`].
    fn distance(&self, p: Vec3) -> f32;

    /// World bounds outside which [`Self::distance`] is guaranteed positive
    /// (for [`ModOp::Union`]) or irrelevant (for [`ModOp::Subtract`]).
    ///
    /// The field never evaluates a mod outside these bounds, so a wrong box
    /// shows up as a solid that gets clipped, not as a slow one.
    fn bounds(&self) -> Bounds3;

    /// How this mod combines with the field beneath it.
    fn op(&self) -> ModOp;

    /// Debug label, used in `analyze` reports and test failures.
    fn label(&self) -> &str;
}

// --------------------------------------------------------------- primitives

/// Axis-aligned box of rock — the simplest useful mod, and the one the
/// Phase A tests drive an overhang with.
#[derive(Debug, Clone)]
pub struct BoxMod {
    pub bounds: Bounds3,
    pub op: ModOp,
    pub label: String,
}

impl BoxMod {
    pub fn new(label: impl Into<String>, bounds: Bounds3, op: ModOp) -> Self {
        Self {
            bounds,
            op,
            label: label.into(),
        }
    }
}

impl VoxelMod for BoxMod {
    fn distance(&self, p: Vec3) -> f32 {
        let c = (self.bounds.min + self.bounds.max) * 0.5;
        let h = (self.bounds.max - self.bounds.min) * 0.5;
        let q = (p - c).abs() - h;
        // Exact box SDF: outside distance plus the (negative) inside depth.
        q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
    }

    fn bounds(&self) -> Bounds3 {
        // Subtractive mods must stay evaluable slightly outside their own
        // volume so the field can round the lip of the cut.
        self.bounds.expanded(1.0)
    }

    fn op(&self) -> ModOp {
        self.op
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// A capsule of air or rock swept along a segment — the primitive behind
/// `<Cave>` tubes and arch spans.
#[derive(Debug, Clone)]
pub struct CapsuleMod {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    pub op: ModOp,
    pub label: String,
}

impl CapsuleMod {
    pub fn new(label: impl Into<String>, a: Vec3, b: Vec3, radius: f32, op: ModOp) -> Self {
        Self {
            a,
            b,
            radius: radius.max(0.01),
            op,
            label: label.into(),
        }
    }
}

impl VoxelMod for CapsuleMod {
    fn distance(&self, p: Vec3) -> f32 {
        let pa = p - self.a;
        let ba = self.b - self.a;
        let denom = ba.dot(ba);
        // A degenerate segment collapses to a sphere rather than dividing by 0.
        let h = if denom > 1e-12 {
            (pa.dot(ba) / denom).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (pa - ba * h).length() - self.radius
    }

    fn bounds(&self) -> Bounds3 {
        Bounds3::from_corners(self.a, self.b).expanded(self.radius + 1.0)
    }

    fn op(&self) -> ModOp {
        self.op
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// A rock portal: one solid block in a local frame (yaw about its centre
/// XZ), minus an arched void — a box opening capped by a half-cylinder. One
/// mod, so a free-standing `<Arch>` costs one index entry per column it
/// covers.
///
/// The composite SDF is not a metric distance (the void is the min of two
/// child distances), but it is continuous and never lies about the sign
/// inside [`Self::bounds`] — which is the whole [`VoxelMod`] contract.
#[derive(Debug, Clone)]
pub struct ArchMod {
    /// Block base centre (the legs' foot line, world space).
    pub base: Vec3,
    /// Clear opening half-width (local X).
    pub span_half: f32,
    /// Height of the STRAIGHT jambs (local Y, from the base); the rounded
    /// crown adds `span_half` on top of this.
    pub height: f32,
    /// Leg thickness beyond the opening on each side (local X).
    pub thickness: f32,
    /// Block depth along the arch axis (local Z, full extent).
    pub depth: f32,
    /// Rock cap above the crown, as part of the same block.
    pub cap: f32,
    /// Rotation about the base centre, radians.
    pub yaw: f32,
    pub op: ModOp,
    pub label: String,
}

impl ArchMod {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        label: impl Into<String>,
        base: Vec3,
        span_half: f32,
        height: f32,
        thickness: f32,
        depth: f32,
        cap: f32,
        yaw: f32,
        op: ModOp,
    ) -> Self {
        Self {
            base,
            span_half: span_half.max(0.1),
            height: height.max(0.1),
            thickness: thickness.max(0.1),
            depth: depth.max(0.1),
            cap: cap.max(0.1),
            yaw,
            op,
            label: label.into(),
        }
    }

    /// Local frame point: yaw about the base centre, origin at the base.
    fn to_local(&self, p: Vec3) -> Vec3 {
        let rel = p - self.base;
        let (s, c) = self.yaw.sin_cos();
        // R(-yaw) about Y.
        Vec3::new(c * rel.x - s * rel.z, rel.y, s * rel.x + c * rel.z)
    }

    /// Exact SDF of the solid block (axis-aligned in the local frame). The
    /// total height encloses the crown: jamb + span_half + cap.
    fn solid(&self, q: Vec3) -> f32 {
        let hx = self.span_half + self.thickness;
        let total = self.height + self.span_half + self.cap;
        let centre = Vec3::new(0.0, total * 0.5, 0.0);
        let half = Vec3::new(hx, total * 0.5, self.depth * 0.5);
        let d = (q - centre).abs() - half;
        d.max(Vec3::ZERO).length() + d.max_element().min(0.0)
    }

    /// Union of the two void children (jamb box + crown capsule). Negative
    /// where the opening is.
    fn void(&self, q: Vec3) -> f32 {
        // Jambs: a box slightly over-wide and reaching below the base, so
        // the opening cuts clean through the legs' foot.
        let jamb_centre = Vec3::new(0.0, self.height * 0.5 - 1.0, 0.0);
        let jamb_half = Vec3::new(self.span_half, self.height * 0.5 + 1.0, self.depth);
        let dj = (q - jamb_centre).abs() - jamb_half;
        let jamb = dj.max(Vec3::ZERO).length() + dj.max_element().min(0.0);
        // Crown: a capsule overlong along the arch axis, radius = half span,
        // centred at jamb height — the rounded top of the opening.
        let a = Vec3::new(0.0, self.height, -self.depth);
        let b = Vec3::new(0.0, self.height, self.depth);
        let crown = capsule_distance(q, a, b, self.span_half);
        jamb.min(crown)
    }
}

impl VoxelMod for ArchMod {
    fn distance(&self, p: Vec3) -> f32 {
        let q = self.to_local(p);
        self.solid(q).max(-self.void(q))
    }

    fn bounds(&self) -> Bounds3 {
        // Conservative world AABB of the yawed block: the XZ half-diagonal.
        let hx = self.span_half + self.thickness;
        let hz = self.depth * 0.5;
        let r = (hx * hx + hz * hz).sqrt();
        let total = self.height + self.span_half + self.cap;
        let half = Vec3::new(r, total * 0.5, r);
        let centre = self.base + Vec3::new(0.0, half.y, 0.0);
        Bounds3::from_corners(centre - half, centre + half).expanded(1.0)
    }

    fn op(&self) -> ModOp {
        self.op
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// Signed distance to a capsule (segment `a`–`b`, radius), shared helper.
fn capsule_distance(p: Vec3, a: Vec3, b: Vec3, radius: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let denom = ba.dot(ba);
    let h = if denom > 1e-12 {
        (pa.dot(ba) / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (pa - ba * h).length() - radius
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_box() -> BoxMod {
        BoxMod::new(
            "b",
            Bounds3::from_corners(Vec3::splat(-1.0), Vec3::splat(1.0)),
            ModOp::Union,
        )
    }

    #[test]
    fn test_box_distance_is_negative_inside_and_positive_outside() {
        let b = unit_box();
        assert!(b.distance(Vec3::ZERO) < 0.0, "center must be solid");
        assert!(b.distance(Vec3::new(3.0, 0.0, 0.0)) > 0.0);
        // On the face the distance is zero.
        assert!(b.distance(Vec3::new(1.0, 0.0, 0.0)).abs() < 1e-6);
    }

    #[test]
    fn test_box_distance_matches_euclidean_outside() {
        let b = unit_box();
        // Straight out from a face: 4 - 1 = 3 m.
        assert!((b.distance(Vec3::new(4.0, 0.0, 0.0)) - 3.0).abs() < 1e-5);
        // Off a corner: the diagonal of a (2,2,2) offset.
        let d = b.distance(Vec3::splat(3.0));
        assert!((d - (12.0f32).sqrt()).abs() < 1e-5, "corner distance {d}");
    }

    #[test]
    fn test_box_center_depth_is_the_half_extent() {
        // Inside, the SDF reads the distance to the nearest face.
        let b = BoxMod::new(
            "b",
            Bounds3::from_corners(Vec3::new(-4.0, -1.0, -4.0), Vec3::new(4.0, 1.0, 4.0)),
            ModOp::Union,
        );
        assert!((b.distance(Vec3::ZERO) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_box_bounds_are_padded_so_the_lip_stays_evaluable() {
        let b = unit_box();
        assert!(b.bounds().contains(Vec3::new(1.5, 0.0, 0.0)));
    }

    #[test]
    fn test_capsule_distance_along_and_off_the_segment() {
        let c = CapsuleMod::new(
            "c",
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            2.0,
            ModOp::Subtract,
        );
        assert!(c.distance(Vec3::new(5.0, 0.0, 0.0)) < 0.0);
        assert!((c.distance(Vec3::new(5.0, 3.0, 0.0)) - 1.0).abs() < 1e-5);
        // Past the cap the distance measures from the endpoint.
        assert!((c.distance(Vec3::new(13.0, 0.0, 0.0)) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_degenerate_capsule_is_a_sphere_not_a_nan() {
        let c = CapsuleMod::new("c", Vec3::ZERO, Vec3::ZERO, 2.0, ModOp::Union);
        let d = c.distance(Vec3::new(3.0, 0.0, 0.0));
        assert!(d.is_finite(), "degenerate segment produced {d}");
        assert!((d - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_bounds_intersects_xz_ignores_vertical_separation() {
        let a = Bounds3::from_corners(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 1.0, 10.0));
        let b = Bounds3::from_corners(Vec3::new(5.0, 900.0, 5.0), Vec3::new(6.0, 901.0, 6.0));
        assert!(!a.intersects(&b), "boxes are vertically apart");
        assert!(a.intersects_xz(&b), "but they share a column");
    }

    #[test]
    fn test_bounds_union_and_empty() {
        let e = Bounds3::empty();
        let a = Bounds3::from_corners(Vec3::ZERO, Vec3::splat(1.0));
        assert_eq!(e.union(a), a, "empty must be the union identity");
    }

    #[test]
    fn test_bounds_distance_to_is_zero_inside() {
        let a = Bounds3::from_corners(Vec3::ZERO, Vec3::splat(2.0));
        assert_eq!(a.distance_to(Vec3::splat(1.0)), 0.0);
        assert!((a.distance_to(Vec3::new(5.0, 1.0, 1.0)) - 3.0).abs() < 1e-6);
    }

    fn arch() -> ArchMod {
        ArchMod::new(
            "arch",
            Vec3::new(100.0, 5.0, 100.0),
            4.0, // span_half
            4.0, // jamb height
            2.5, // thickness
            4.0, // depth
            2.0, // cap
            0.0, // yaw
            ModOp::Union,
        )
    }

    #[test]
    fn test_arch_legs_solid_opening_void_crown_solid() {
        let a = arch();
        // Inside a leg (legs span local x in [4, 6.5]): solid.
        assert!(
            a.distance(Vec3::new(105.2, 9.0, 100.0)) < 0.0,
            "leg interior must be solid"
        );
        // Middle of the opening (jambs reach 8 m, span ±4): void.
        assert!(
            a.distance(Vec3::new(100.0, 9.0, 100.0)) > 0.0,
            "opening centre must be air"
        );
        // Through the opening along the arch axis: still air at ±1.5 depth.
        assert!(
            a.distance(Vec3::new(100.0, 8.0, 101.5)) > 0.0,
            "the opening must run through the block"
        );
        // Crown rock: opening apex = jamb 4 + span 4 = 8 (world 13), cap
        // tops out at 10 (world 15) — 13.5 is inside the cap.
        assert!(
            a.distance(Vec3::new(100.0, 13.5, 100.0)) < 0.0,
            "rock above the crown must be solid"
        );
        // High inside a leg, above the crown: still solid.
        assert!(a.distance(Vec3::new(105.2, 13.5, 100.0)) < 0.0);
    }

    #[test]
    fn test_arch_yaw_rotates_the_solid() {
        let mut a = arch();
        a.yaw = std::f32::consts::FRAC_PI_2;
        // R(-yaw) maps local +X (span axis) to world -Z... either way the
        // opening now runs along world Z and the legs sit at world z ±[4, 6.5].
        assert!(
            a.distance(Vec3::new(100.0, 9.0, 103.5)) > 0.0,
            "opening must follow the yaw"
        );
        assert!(
            a.distance(Vec3::new(100.0, 9.0, 105.2)) < 0.0,
            "leg rock must follow the yaw"
        );
        assert!(
            a.distance(Vec3::new(100.0, 9.0, 100.0)) > 0.0,
            "world centre stays air"
        );
    }

    #[test]
    fn test_arch_bounds_cover_the_block() {
        let a = arch();
        let b = a.bounds();
        assert!(b.contains(Vec3::new(100.0, 6.0, 100.0)));
        assert!(
            b.contains(Vec3::new(100.0 + 5.2, 9.0, 100.0)),
            "leg in bounds"
        );
        assert!(b.contains(Vec3::new(100.0, 13.5, 100.0)), "cap in bounds");
        assert!(
            !b.contains(Vec3::new(100.0, 30.0, 100.0)),
            "nothing solid above the block"
        );
    }
}

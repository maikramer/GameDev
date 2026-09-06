//! The layered signed-distance field — the shape authority.
//!
//! ```text
//! density(p) = base(p) ⊕ mods(p)          // negative = solid
//! base(p)    = p.y - height(p.x, p.z)     // the existing heightfield
//! ```
//!
//! The heightfield is not replaced, it is **demoted to a term**. Pads, lakes,
//! rivers and roads keep carving the same `BrushGrid` through the same
//! journalled brush engine, and that grid is what `base` reads. Everything the
//! 2.5D system does well — a 4 km world, LOD out to 950 m, splat, water, road
//! ribbons — keeps working untouched, because for a column with no mod over it
//! this field *is* the heightfield, bit for bit.
//!
//! What changes is that a column can now have more than one surface.

use bevy::math::Vec3;

use super::super::mesh::HeightField;
use super::index::{ChunkClass, ModIndex};
use super::mods::{Bounds3, ModOp, VoxelMod};

/// A solid interval in a column, in world meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    /// Upper surface (the one you stand on).
    pub top: f32,
    /// Lower surface (the ceiling of whatever is underneath).
    pub bottom: f32,
}

impl Span {
    pub fn thickness(&self) -> f32 {
        self.top - self.bottom
    }

    pub fn contains(&self, y: f32) -> bool {
        y >= self.bottom && y <= self.top
    }
}

/// How finely a column march steps before bisecting, in meters.
///
/// Half the 1 m voxel: a solid thinner than this can be missed by the march,
/// which is the same resolution limit the mesher has anyway.
pub const COLUMN_STEP: f32 = 0.5;

/// Bisection passes used to refine a bracketed crossing.
///
/// 12 halvings of a 0.5 m bracket land at ~0.12 mm — far below anything the
/// mesh or the collider can express, and cheap enough for the spawner ring.
const REFINE_ITERS: usize = 12;

/// Padding above/below the searched column, in meters.
const COLUMN_PAD: f32 = 1.0;

/// The voxel field: the mods, plus the index that keeps them cheap to ignore.
///
/// It deliberately does **not** own the [`HeightField`]. `TerrainRuntime` owns
/// the one `BrushGrid`, and the field borrows it per query, so there is
/// physically no way for a second height authority to appear.
#[derive(Debug)]
pub struct VoxelField {
    mods: Vec<Box<dyn VoxelMod>>,
    index: ModIndex,
}

impl VoxelField {
    /// Builds a field from authored mods over a world of `world_size` meters.
    pub fn new(mods: Vec<Box<dyn VoxelMod>>, world_size: f32, cell_size: f32) -> Self {
        let index = ModIndex::build(&mods, world_size, cell_size);
        Self { mods, index }
    }

    /// A field with no 3D features: every query falls straight through to the
    /// heightfield. This is what a world with no `<Cliff>`/`<Cave>` gets, and
    /// it allocates nothing.
    pub fn flat(world_size: f32, cell_size: f32) -> Self {
        Self::new(Vec::new(), world_size, cell_size)
    }

    pub fn index(&self) -> &ModIndex {
        &self.index
    }

    pub fn mods(&self) -> &[Box<dyn VoxelMod>] {
        &self.mods
    }

    /// True when no mod exists anywhere in the world.
    pub fn is_flat(&self) -> bool {
        self.index.is_empty()
    }

    /// True when no mod covers this column, so the heightfield is the whole
    /// answer here.
    pub fn is_flat_at(&self, x: f32, z: f32) -> bool {
        !self.index.is_volumetric_at(x, z)
    }

    /// How a chunk covering `bounds` must be meshed.
    pub fn classify(&self, bounds: &Bounds3) -> ChunkClass {
        self.index.classify(bounds)
    }

    /// How the terrain chunk covering this XZ rectangle must be meshed.
    ///
    /// Classification is **XZ-only** — a cave 40 m under a hill claims the
    /// whole column, because the heightfield mesher would otherwise draw the
    /// ground straight through the roof of it.
    ///
    /// Both the bootstrap and the LOD plugin have to agree on this, and they
    /// have to agree *exactly*: the plugin respawns any chunk that has no
    /// entity, so a chunk the bootstrap skips and the plugin does not is
    /// spawned right on top of the voxel mesh. That is a full-screen moiré of
    /// z-fighting, not a subtle seam. One method, two callers, no drift.
    pub fn classify_terrain_chunk(&self, min_x: f32, min_z: f32, edge: f32) -> ChunkClass {
        self.classify(&Bounds3::from_corners(
            Vec3::new(min_x, 0.0, min_z),
            Vec3::new(min_x + edge, 0.0, min_z + edge),
        ))
    }

    /// True when the terrain chunk at this XZ rectangle belongs to the voxel
    /// mesher rather than the heightfield one.
    pub fn is_volumetric_chunk(&self, min_x: f32, min_z: f32, edge: f32) -> bool {
        self.classify_terrain_chunk(min_x, min_z, edge) == ChunkClass::Volumetric
    }

    /// This is what makes a volumetric column affordable. A 64 m terrain chunk
    /// over 200 m of relief is ~28 voxel chunks; meshing all of them blind
    /// would be ~1M density evaluations for one chunk, inline, in a frame. The
    /// heightfield already answers min/max over an XZ box in O(1)
    /// (`BrushGrid::range_over`), so every voxel chunk that is plainly buried
    /// or plainly in the sky is rejected before a single sample is taken.
    pub fn region_state(&self, base: &dyn HeightField, bounds: &Bounds3) -> Option<bool> {
        // Any mod in range and the cheap argument no longer holds: a cave can
        // hollow out rock that the heightfield swears is solid.
        for &i in self.index.candidates_in(bounds).iter() {
            if self.mods[i as usize].bounds().intersects(bounds) {
                return None;
            }
        }
        let (gmin, gmax) =
            base.range_over(bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z)?;
        if bounds.min.y >= gmax {
            return Some(false);
        }
        if bounds.max.y <= gmin {
            return Some(true);
        }
        None
    }

    /// Signed distance at a world point: **negative inside rock**.
    pub fn density(&self, base: &dyn HeightField, p: Vec3) -> f32 {
        let mut d = p.y - base.sample(p.x, p.z);
        for &i in self.index.candidates_at(p.x, p.z) {
            let m = &self.mods[i as usize];
            // The bounds gate is not only a speed win: it stops a subtractive
            // mod from eroding the field in a region it does not reach.
            if !m.bounds().contains(p) {
                continue;
            }
            let md = m.distance(p);
            d = match m.op() {
                ModOp::Union => d.min(md),
                ModOp::Subtract => d.max(-md),
            };
        }
        d
    }

    /// Surface normal from the field gradient, by central differences.
    ///
    /// The gradient of a continuous field is continuous, so two chunks meeting
    /// at a border compute the same normal without the frontier-normal trick
    /// the heightfield mesher needs.
    pub fn gradient(&self, base: &dyn HeightField, p: Vec3, eps: f32) -> Vec3 {
        let e = if eps.is_finite() && eps > 0.0 {
            eps
        } else {
            COLUMN_STEP
        };
        let dx = self.density(base, p + Vec3::X * e) - self.density(base, p - Vec3::X * e);
        let dy = self.density(base, p + Vec3::Y * e) - self.density(base, p - Vec3::Y * e);
        let dz = self.density(base, p + Vec3::Z * e) - self.density(base, p - Vec3::Z * e);
        // The gradient points out of the rock, which is already the outward
        // normal. A degenerate cell (all-equal samples) falls back to up
        // rather than emitting a zero normal the renderer would choke on.
        Vec3::new(dx, dy, dz).try_normalize().unwrap_or(Vec3::Y)
    }

    /// The vertical window a column march has to cover here.
    fn column_range(&self, base: &dyn HeightField, x: f32, z: f32) -> Option<(f32, f32)> {
        let ground = base.sample(x, z);
        let (lo, hi) = self.index.column_span_at(x, z, ground)?;
        Some((lo - COLUMN_PAD, hi.max(ground) + COLUMN_PAD))
    }

    /// Refines a bracketed sign change to the surface height between `lo`
    /// (solid) and `hi` (empty).
    fn bisect(&self, base: &dyn HeightField, x: f32, z: f32, mut lo: f32, mut hi: f32) -> f32 {
        for _ in 0..REFINE_ITERS {
            let mid = (lo + hi) * 0.5;
            if self.density(base, Vec3::new(x, mid, z)) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) * 0.5
    }

    /// Height of the **topmost** solid surface at this column.
    ///
    /// This is what `TerrainRuntime::sample` keeps meaning. On a flat column it
    /// returns the heightfield value and costs one heightfield sample — which
    /// is what protects the 44 existing call sites and the 4 km budget.
    pub fn surface_top(&self, base: &dyn HeightField, x: f32, z: f32) -> f32 {
        let Some((lo, hi)) = self.column_range(base, x, z) else {
            return base.sample(x, z);
        };
        let mut y = hi;
        // Already inside rock at the very top: the mod stacks above anything we
        // could search, so that is the surface.
        if self.density(base, Vec3::new(x, y, z)) < 0.0 {
            return y;
        }
        while y > lo {
            let next = (y - COLUMN_STEP).max(lo);
            if self.density(base, Vec3::new(x, next, z)) < 0.0 {
                return self.bisect(base, x, z, next, y);
            }
            y = next;
        }
        // Nothing solid anywhere in the column (a shaft cut clean through):
        // fall back to the heightfield so callers still get a finite number.
        base.sample(x, z)
    }

    /// Topmost solid surface **at or below** `from_y` — the query a creature
    /// walking inside a cave needs, and the one `surface_top` cannot answer.
    ///
    /// Returns `None` when there is no surface down there: either the column is
    /// empty, or `from_y` is buried in rock that continues all the way down.
    /// Answering `Some(from_y)` in that second case would let a caller stand an
    /// entity inside a hill, so the absence is reported instead.
    pub fn surface_below(
        &self,
        base: &dyn HeightField,
        x: f32,
        z: f32,
        from_y: f32,
    ) -> Option<f32> {
        let Some((lo, _hi)) = self.column_range(base, x, z) else {
            let h = base.sample(x, z);
            return (h <= from_y).then_some(h);
        };
        let mut y = from_y;
        if y <= lo {
            return None;
        }
        // Walk down out of any rock we start inside, so a query from within a
        // solid returns the floor below it rather than the point itself.
        while y > lo && self.density(base, Vec3::new(x, y, z)) < 0.0 {
            y -= COLUMN_STEP;
        }
        while y > lo {
            let next = (y - COLUMN_STEP).max(lo);
            if self.density(base, Vec3::new(x, next, z)) < 0.0 {
                return Some(self.bisect(base, x, z, next, y));
            }
            y = next;
        }
        None
    }

    /// Every solid interval in this column, top-down.
    ///
    /// One span is ordinary ground. Two mean an overhang, an arch or a cave
    /// roof — the thing a heightfield cannot represent at all.
    pub fn column(&self, base: &dyn HeightField, x: f32, z: f32) -> Vec<Span> {
        let Some((lo, hi)) = self.column_range(base, x, z) else {
            return vec![Span {
                top: base.sample(x, z),
                bottom: f32::NEG_INFINITY,
            }];
        };

        let mut spans = Vec::new();
        let mut y = hi;
        let mut inside = self.density(base, Vec3::new(x, y, z)) < 0.0;
        let mut top = if inside { hi } else { f32::NAN };

        while y > lo {
            let next = (y - COLUMN_STEP).max(lo);
            let solid = self.density(base, Vec3::new(x, next, z)) < 0.0;
            if solid != inside {
                let crossing = if solid {
                    self.bisect(base, x, z, next, y)
                } else {
                    // Leaving rock going down: bracket is (solid=y, empty=next).
                    self.bisect(base, x, z, y, next)
                };
                if solid {
                    top = crossing;
                } else {
                    spans.push(Span {
                        top,
                        bottom: crossing,
                    });
                }
                inside = solid;
            }
            y = next;
        }
        if inside {
            spans.push(Span {
                top,
                bottom: f32::NEG_INFINITY,
            });
        }
        spans
    }
}

impl Default for VoxelField {
    /// The mod-less field. Cheap enough to be the default for any world or
    /// test fixture that has no 3D terrain features.
    fn default() -> Self {
        Self::flat(0.0, super::index::DEFAULT_CELL_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::voxel::mods::{BoxMod, CapsuleMod};

    #[test]
    fn test_default_field_is_flat_and_defers_to_the_heightfield() {
        let field = VoxelField::default();
        let base = Flat(3.0);
        assert!(field.is_flat());
        assert_eq!(field.surface_top(&base, 900.0, -900.0), 3.0);
    }

    /// A dead-flat analytic heightfield at a fixed height.
    #[derive(Debug)]
    struct Flat(f32);

    impl HeightField for Flat {
        fn sample(&self, _x: f32, _z: f32) -> f32 {
            self.0
        }
        fn sample_normal(&self, _x: f32, _z: f32, _eps: f32) -> Vec3 {
            Vec3::Y
        }
        fn max_height(&self) -> f32 {
            self.0
        }
    }

    /// A ramp climbing along +X, to prove the base term is really consulted.
    #[derive(Debug)]
    struct Ramp;

    impl HeightField for Ramp {
        fn sample(&self, x: f32, _z: f32) -> f32 {
            x * 0.25
        }
        fn sample_normal(&self, _x: f32, _z: f32, _eps: f32) -> Vec3 {
            Vec3::Y
        }
        fn max_height(&self) -> f32 {
            100.0
        }
    }

    fn union_box(label: &str, min: Vec3, max: Vec3) -> Box<dyn VoxelMod> {
        Box::new(BoxMod::new(
            label,
            Bounds3::from_corners(min, max),
            ModOp::Union,
        ))
    }

    fn cut_box(label: &str, min: Vec3, max: Vec3) -> Box<dyn VoxelMod> {
        Box::new(BoxMod::new(
            label,
            Bounds3::from_corners(min, max),
            ModOp::Subtract,
        ))
    }

    #[test]
    fn test_field_without_mods_is_exactly_the_heightfield() {
        let field = VoxelField::flat(512.0, 64.0);
        let base = Flat(12.5);
        assert!(field.is_flat());
        for (x, z) in [(0.0, 0.0), (100.0, -80.0), (-250.0, 250.0)] {
            assert_eq!(
                field.surface_top(&base, x, z),
                base.sample(x, z),
                "flat field must not move the surface at {x},{z}"
            );
        }
        // And the sign convention holds: below ground is solid.
        assert!(field.density(&base, Vec3::new(0.0, 10.0, 0.0)) < 0.0);
        assert!(field.density(&base, Vec3::new(0.0, 20.0, 0.0)) > 0.0);
    }

    #[test]
    fn test_flat_field_tracks_a_sloped_base() {
        let field = VoxelField::flat(512.0, 64.0);
        let base = Ramp;
        assert!((field.surface_top(&base, 40.0, 0.0) - 10.0).abs() < 1e-4);
        assert!((field.surface_top(&base, -40.0, 0.0) + 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_union_mod_raises_the_surface_above_the_ground() {
        // A block of rock floating from y=20 to y=30 over flat ground at 0.
        let field = VoxelField::new(
            vec![union_box(
                "block",
                Vec3::new(-10.0, 20.0, -10.0),
                Vec3::new(10.0, 30.0, 10.0),
            )],
            512.0,
            64.0,
        );
        let base = Flat(0.0);
        let top = field.surface_top(&base, 0.0, 0.0);
        assert!(
            (top - 30.0).abs() < 0.01,
            "expected the block top, got {top}"
        );
        // Outside the block the ground is untouched.
        assert!((field.surface_top(&base, 100.0, 0.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_overhang_column_has_two_spans_and_a_gap_between_them() {
        let field = VoxelField::new(
            vec![union_box(
                "shelf",
                Vec3::new(-10.0, 20.0, -10.0),
                Vec3::new(10.0, 30.0, 10.0),
            )],
            512.0,
            64.0,
        );
        let base = Flat(0.0);
        let spans = field.column(&base, 0.0, 0.0);
        assert_eq!(
            spans.len(),
            2,
            "overhang column: shelf + ground, got {spans:?}"
        );
        assert!((spans[0].top - 30.0).abs() < 0.01);
        assert!((spans[0].bottom - 20.0).abs() < 0.01);
        assert!((spans[1].top - 0.0).abs() < 0.01);
        assert!(
            (spans[0].bottom - spans[1].top) > 15.0,
            "there must be walkable air under the shelf"
        );
    }

    #[test]
    fn test_surface_below_finds_the_floor_under_an_overhang() {
        let field = VoxelField::new(
            vec![union_box(
                "shelf",
                Vec3::new(-10.0, 20.0, -10.0),
                Vec3::new(10.0, 30.0, 10.0),
            )],
            512.0,
            64.0,
        );
        let base = Flat(0.0);
        // Standing on top of the shelf: the top query answers the shelf.
        assert!((field.surface_top(&base, 0.0, 0.0) - 30.0).abs() < 0.01);
        // Standing under it: the floor is the ground, not the shelf.
        let under = field
            .surface_below(&base, 0.0, 0.0, 10.0)
            .expect("ground under the shelf");
        assert!((under - 0.0).abs() < 0.01, "expected ground, got {under}");
    }

    #[test]
    fn test_surface_below_reports_no_floor_when_buried_in_solid_ground() {
        let field = VoxelField::flat(512.0, 64.0);
        let base = Flat(10.0);
        // From above the ground, the ground is the floor.
        let y = field
            .surface_below(&base, 0.0, 0.0, 40.0)
            .expect("the ground is below y=40");
        assert!((y - 10.0).abs() < 1e-4, "got {y}");
        // Buried at y=5 there is no surface below at all — the rock continues
        // down forever. `None` is the answer, not the point itself: a caller
        // that got `Some(5.0)` would happily stand the player inside a hill.
        assert_eq!(field.surface_below(&base, 0.0, 0.0, 5.0), None);
    }

    #[test]
    fn test_subtract_mod_opens_a_cave_with_a_roof_over_it() {
        // Solid ground to y=40, with a horizontal tube bored through it.
        let field = VoxelField::new(
            vec![Box::new(CapsuleMod::new(
                "tunnel",
                Vec3::new(-40.0, 10.0, 0.0),
                Vec3::new(40.0, 10.0, 0.0),
                4.0,
                ModOp::Subtract,
            ))],
            512.0,
            64.0,
        );
        let base = Flat(40.0);
        let spans = field.column(&base, 0.0, 0.0);
        assert_eq!(spans.len(), 2, "roof + floor, got {spans:?}");
        // Roof span sits on top, its underside at the tube ceiling (~14).
        assert!((spans[0].top - 40.0).abs() < 0.01);
        assert!(
            (spans[0].bottom - 14.0).abs() < 0.2,
            "roof at {:?}",
            spans[0]
        );
        // Floor span starts at the tube's bottom (~6).
        assert!((spans[1].top - 6.0).abs() < 0.2, "floor at {:?}", spans[1]);

        let floor = field
            .surface_below(&base, 0.0, 0.0, 10.0)
            .expect("cave floor");
        assert!((floor - 6.0).abs() < 0.2, "cave floor at {floor}");
        // The top query still reports the hill, unchanged for every other system.
        assert!((field.surface_top(&base, 0.0, 0.0) - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_subtract_mod_does_not_erode_terrain_it_never_reaches() {
        let field = VoxelField::new(
            vec![cut_box(
                "pit",
                Vec3::new(-5.0, 0.0, -5.0),
                Vec3::new(5.0, 20.0, 5.0),
            )],
            512.0,
            64.0,
        );
        let base = Flat(30.0);
        // Far from the cut the surface is untouched.
        assert!((field.surface_top(&base, 200.0, 200.0) - 30.0).abs() < 0.01);
        // Deep under the cut, still solid rock.
        assert!(field.density(&base, Vec3::new(0.0, -50.0, 0.0)) < 0.0);
    }

    #[test]
    fn test_gradient_points_up_on_flat_ground_and_sideways_on_a_wall() {
        let field = VoxelField::new(
            vec![union_box(
                "wall",
                Vec3::new(0.0, 0.0, -50.0),
                Vec3::new(20.0, 40.0, 50.0),
            )],
            512.0,
            64.0,
        );
        let base = Flat(0.0);
        let up = field.gradient(&base, Vec3::new(-100.0, 0.0, 0.0), 0.25);
        assert!(up.y > 0.9, "flat ground normal should be up, got {up}");
        // On the -X face of the wall, halfway up: the normal faces -X.
        let side = field.gradient(&base, Vec3::new(0.0, 20.0, 0.0), 0.25);
        assert!(side.x < -0.8, "wall face normal should be -X, got {side}");
        assert!(
            side.y.abs() < 0.3,
            "a vertical wall must not read as ground"
        );
    }

    #[test]
    fn test_gradient_is_finite_in_a_dead_region() {
        let field = VoxelField::flat(512.0, 64.0);
        let base = Flat(0.0);
        // Far above the terrain the field is a smooth ramp in y, never zero.
        let n = field.gradient(&base, Vec3::new(0.0, 500.0, 0.0), 0.5);
        assert!(n.is_finite(), "gradient produced {n}");
        assert!((n.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_flat_column_returns_one_span_without_marching() {
        let field = VoxelField::flat(512.0, 64.0);
        let base = Flat(7.0);
        let spans = field.column(&base, 0.0, 0.0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].top, 7.0);
        assert_eq!(spans[0].bottom, f32::NEG_INFINITY);
    }

    /// A heightfield that answers range queries, like the real `BrushGrid`.
    #[derive(Debug)]
    struct RangedFlat(f32);

    impl HeightField for RangedFlat {
        fn sample(&self, _x: f32, _z: f32) -> f32 {
            self.0
        }
        fn sample_normal(&self, _x: f32, _z: f32, _eps: f32) -> Vec3 {
            Vec3::Y
        }
        fn max_height(&self) -> f32 {
            self.0
        }
        fn range_over(&self, _a: f32, _b: f32, _c: f32, _d: f32) -> Option<(f32, f32)> {
            Some((self.0, self.0))
        }
    }

    #[test]
    fn test_region_state_rejects_sky_and_bedrock_without_sampling() {
        let field = VoxelField::flat(512.0, 64.0);
        let base = RangedFlat(20.0);
        let sky = Bounds3::from_corners(Vec3::new(0.0, 40.0, 0.0), Vec3::new(32.0, 72.0, 32.0));
        let rock = Bounds3::from_corners(Vec3::new(0.0, -40.0, 0.0), Vec3::new(32.0, 0.0, 32.0));
        let surface = Bounds3::from_corners(Vec3::new(0.0, 0.0, 0.0), Vec3::new(32.0, 32.0, 32.0));
        assert_eq!(field.region_state(&base, &sky), Some(false), "sky is empty");
        assert_eq!(
            field.region_state(&base, &rock),
            Some(true),
            "deep is solid"
        );
        assert_eq!(field.region_state(&base, &surface), None, "must be meshed");
    }

    #[test]
    fn test_region_state_refuses_to_shortcut_where_a_mod_reaches() {
        // Bedrock that a cave passes through must still be meshed.
        let field = VoxelField::new(
            vec![Box::new(CapsuleMod::new(
                "tunnel",
                Vec3::new(-40.0, -20.0, 16.0),
                Vec3::new(40.0, -20.0, 16.0),
                4.0,
                ModOp::Subtract,
            ))],
            512.0,
            64.0,
        );
        let base = RangedFlat(20.0);
        let rock = Bounds3::from_corners(Vec3::new(0.0, -40.0, 0.0), Vec3::new(32.0, 0.0, 32.0));
        assert_eq!(
            field.region_state(&base, &rock),
            None,
            "rock with a tunnel through it is not uniform"
        );
    }

    #[test]
    fn test_region_state_is_none_when_the_base_cannot_answer_ranges() {
        // `HeightField::range_over` defaults to None; the mesher must then do
        // the work rather than guess.
        let field = VoxelField::flat(512.0, 64.0);
        let base = Flat(20.0);
        let sky = Bounds3::from_corners(Vec3::new(0.0, 400.0, 0.0), Vec3::new(32.0, 432.0, 32.0));
        assert_eq!(field.region_state(&base, &sky), None);
    }

    #[test]
    fn test_chunk_classification_is_xz_only_and_shared_by_both_meshers() {
        // A cave far under a hill still claims the surface chunk: otherwise
        // the heightfield mesher draws ground through the cave roof.
        let field = VoxelField::new(
            vec![Box::new(CapsuleMod::new(
                "deep",
                Vec3::new(0.0, -80.0, 0.0),
                Vec3::new(40.0, -80.0, 0.0),
                4.0,
                ModOp::Subtract,
            ))],
            512.0,
            64.0,
        );
        assert!(field.is_volumetric_chunk(0.0, -32.0, 64.0));
        assert!(!field.is_volumetric_chunk(200.0, 200.0, 64.0));
    }

    #[test]
    fn test_is_flat_at_is_false_only_over_authored_features() {
        let field = VoxelField::new(
            vec![union_box(
                "block",
                Vec3::new(-10.0, 0.0, -10.0),
                Vec3::new(10.0, 10.0, 10.0),
            )],
            512.0,
            64.0,
        );
        assert!(!field.is_flat_at(0.0, 0.0));
        assert!(field.is_flat_at(200.0, 200.0));
        assert!(!field.is_flat());
    }
}

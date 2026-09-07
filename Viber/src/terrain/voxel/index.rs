//! Spatial index over the field's mods — the thing that keeps a voxel terrain
//! affordable in a 4 km world.
//!
//! The whole hybrid design rests on one question being cheap to answer: *does
//! anything three-dimensional touch this column?* For the overwhelming
//! majority of the world the answer is no, and those columns must cost exactly
//! what they cost today — a heightfield sample and nothing else.
//!
//! So mods are bucketed into a uniform XZ grid. A point query hits one cell,
//! finds it empty, and returns an empty slice: no AABB tests, no iteration
//! over the mod list. Only the authored neighbourhoods pay.
//!
//! The grid is indexed in **XZ only**. A cave 40 m under a hill and the hill
//! itself share a column, and both make it volumetric — the vertical extent is
//! resolved later, by the mods' own bounds.

use super::mods::{Bounds3, VoxelMod};

/// Verdict for one terrain chunk: how its mesh has to be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkClass {
    /// No mod touches this chunk — build it with the existing heightfield
    /// mesher, unchanged. This is >95% of the resident chunks.
    Flat,
    /// A mod overlaps this chunk — it needs surface nets.
    Volumetric,
}

/// Default XZ cell size (meters) when the caller has no better idea.
///
/// Matching the terrain `chunk-size` default keeps one index cell aligned with
/// one chunk, which makes [`ModIndex::classify`] a single lookup.
pub const DEFAULT_CELL_SIZE: f32 = 64.0;

/// Uniform XZ bucket grid over the mods of a [`super::field::VoxelField`].
#[derive(Debug)]
pub struct ModIndex {
    /// Bucketed mod indices, row-major `[z * cols + x]`.
    cells: Vec<Vec<u32>>,
    cols: usize,
    rows: usize,
    cell_size: f32,
    /// World XZ origin of cell (0, 0).
    origin_x: f32,
    origin_z: f32,
    /// Union of every mod's bounds; empty when there are no mods.
    bounds: Bounds3,
    /// Per-mod bounds snapshot, parallel to the indices stored in `cells`.
    /// `column_span_at` marches only the candidates' own Y ranges — a few
    /// meters around the authored feature — instead of the world-wide union.
    mod_bounds: Vec<Bounds3>,
}

/// Finest bucket pitch worth building (meters). Below this the grid costs more
/// to walk than the candidates it saves.
const MIN_CELL_SIZE: f32 = 8.0;
/// Cells per world edge the grid will not exceed, so the bucket array stays
/// small on a 4 km world.
const MAX_CELLS_PER_EDGE: f32 = 256.0;
/// Below this many mods the caller's hint is kept: a world with three cliffs
/// pays nothing for a coarse grid, and a fine one would be mostly empty.
const REFINE_ABOVE_MODS: usize = 16;

/// The bucket pitch actually used.
///
/// The caller hints the terrain chunk size, which is the right pitch for a
/// world whose only mods are a couple of cliffs. It is the wrong one for a
/// world with bridges: forty deck boxes land in one 64 m bucket, and
/// [`super::field::VoxelField::density`] then AABB-tests all forty at every
/// one of the ~39 k samples a LOD0 box takes. Refining the pitch is what
/// keeps that list short — measured at ~3.5x on the bridge column of
/// `worlds/qa-pontes.xml` (`tests/chunk_build_bench.rs`).
fn resolve_cell_size(hint: f32, world_size: f32, mods: usize) -> f32 {
    if mods < REFINE_ABOVE_MODS {
        return hint;
    }
    let floor = (world_size.max(1.0) / MAX_CELLS_PER_EDGE).max(MIN_CELL_SIZE);
    // Refine to the floor, but never coarsen a caller that already asked finer.
    hint.min(floor)
}

impl ModIndex {
    /// Builds the index for `mods` over a world of `world_size` meters
    /// centred on the origin (the convention every other terrain module uses).
    pub fn build(mods: &[Box<dyn VoxelMod>], world_size: f32, cell_size: f32) -> Self {
        let hint = if cell_size.is_finite() && cell_size > 0.0 {
            cell_size
        } else {
            DEFAULT_CELL_SIZE
        };
        let cell_size = resolve_cell_size(hint, world_size, mods.len());
        let world_size = world_size.max(cell_size);
        let half = world_size * 0.5;
        // A world with no mods allocates nothing at all. Every query early-outs
        // on the empty bounds, so the bucket grid would be 3969 empty `Vec`s of
        // pure waste in a 4 km world — and worlds with no 3D features are the
        // common case, including every world authored before this system.
        let span = if mods.is_empty() {
            0
        } else {
            (world_size / cell_size).ceil().max(1.0) as usize
        };

        let mut index = Self {
            cells: vec![Vec::new(); span * span],
            cols: span,
            rows: span,
            cell_size,
            origin_x: -half,
            origin_z: -half,
            bounds: Bounds3::empty(),
            mod_bounds: Vec::with_capacity(mods.len()),
        };

        for (i, m) in mods.iter().enumerate() {
            let b = m.bounds();
            index.bounds = index.bounds.union(b);
            index.mod_bounds.push(b);
            let (x0, z0) = index.cell_of(b.min.x, b.min.z);
            let (x1, z1) = index.cell_of(b.max.x, b.max.z);
            for cz in z0..=z1 {
                for cx in x0..=x1 {
                    index.cells[cz * index.cols + cx].push(i as u32);
                }
            }
        }
        index
    }

    /// An index with no mods — the fast path for a world that authored none.
    pub fn empty(world_size: f32, cell_size: f32) -> Self {
        Self::build(&[], world_size, cell_size)
    }

    /// True when no mod exists at all, so every query can skip the field.
    pub fn is_empty(&self) -> bool {
        self.bounds.min.x > self.bounds.max.x
    }

    /// Union of every mod's bounds.
    pub fn bounds(&self) -> Bounds3 {
        self.bounds
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Clamped cell coordinates for a world XZ position.
    fn cell_of(&self, x: f32, z: f32) -> (usize, usize) {
        let cx = ((x - self.origin_x) / self.cell_size).floor();
        let cz = ((z - self.origin_z) / self.cell_size).floor();
        let cx = cx.clamp(0.0, (self.cols - 1) as f32) as usize;
        let cz = cz.clamp(0.0, (self.rows - 1) as f32) as usize;
        (cx, cz)
    }

    /// Mod indices whose bounds may cover this column. Usually empty.
    pub fn candidates_at(&self, x: f32, z: f32) -> &[u32] {
        // Outside the authored envelope there is nothing to test at all. This
        // guard is what makes a 4 km world of untouched terrain free.
        if self.is_empty()
            || x < self.bounds.min.x
            || x > self.bounds.max.x
            || z < self.bounds.min.z
            || z > self.bounds.max.z
        {
            return &[];
        }
        let (cx, cz) = self.cell_of(x, z);
        &self.cells[cz * self.cols + cx]
    }

    /// Mod indices whose bounds overlap `bounds` in XZ, de-duplicated.
    pub fn candidates_in(&self, bounds: &Bounds3) -> Vec<u32> {
        if self.is_empty() || !self.bounds.intersects_xz(bounds) {
            return Vec::new();
        }
        let (x0, z0) = self.cell_of(bounds.min.x, bounds.min.z);
        let (x1, z1) = self.cell_of(bounds.max.x, bounds.max.z);
        let mut out: Vec<u32> = Vec::new();
        for cz in z0..=z1 {
            for cx in x0..=x1 {
                for &i in &self.cells[cz * self.cols + cx] {
                    if !out.contains(&i) {
                        out.push(i);
                    }
                }
            }
        }
        // Deterministic order: two runs of the same world must mesh the same.
        out.sort_unstable();
        out
    }

    /// How a chunk covering `bounds` has to be meshed.
    ///
    /// Deliberately conservative — it tests XZ overlap of the mods' padded
    /// bounds, so a chunk adjacent to a cliff is classified volumetric and
    /// meshes the transition itself rather than leaving a seam for the
    /// heightfield path to explain.
    pub fn classify(&self, bounds: &Bounds3) -> ChunkClass {
        if self.candidates_in(bounds).is_empty() {
            ChunkClass::Flat
        } else {
            ChunkClass::Volumetric
        }
    }

    /// True when any mod covers this column.
    pub fn is_volumetric_at(&self, x: f32, z: f32) -> bool {
        !self.candidates_at(x, z).is_empty()
    }

    /// Vertical world range a column query has to march, given the ground
    /// height there. `None` when the column is flat and `sample()` can answer
    /// from the heightfield alone.
    pub fn column_span_at(&self, x: f32, z: f32, ground: f32) -> Option<(f32, f32)> {
        let candidates = self.candidates_at(x, z);
        if candidates.is_empty() {
            return None;
        }
        // Only the CANDIDATES' own Y ranges: accumulating the index-wide union
        // made every column of a volumetric region march the whole world
        // vertically (a cave at −60 m stretched the surface march by its full
        // drop) at 0.5 m steps.
        let mut lo = ground;
        let mut hi = ground;
        for &i in candidates {
            let b = self.mod_bounds[i as usize];
            lo = lo.min(b.min.y);
            hi = hi.max(b.max.y);
        }
        Some((lo, hi))
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;

    use super::*;
    use crate::terrain::voxel::mods::{BoxMod, ModOp};

    #[test]
    fn test_an_empty_index_allocates_no_buckets() {
        let idx = ModIndex::empty(4000.0, 64.0);
        assert_eq!(idx.cells.len(), 0, "a mod-less world must cost nothing");
    }

    fn boxed(label: &str, min: Vec3, max: Vec3) -> Box<dyn VoxelMod> {
        Box::new(BoxMod::new(
            label,
            Bounds3::from_corners(min, max),
            ModOp::Union,
        ))
    }

    #[test]
    fn test_empty_index_reports_every_column_flat() {
        let idx = ModIndex::empty(512.0, 64.0);
        assert!(idx.is_empty());
        assert!(idx.candidates_at(0.0, 0.0).is_empty());
        assert!(!idx.is_volumetric_at(100.0, -30.0));
        assert_eq!(
            idx.classify(&Bounds3::from_corners(
                Vec3::splat(-10.0),
                Vec3::splat(10.0)
            )),
            ChunkClass::Flat
        );
    }

    #[test]
    fn test_column_over_a_mod_is_volumetric_and_others_are_not() {
        let mods = vec![boxed(
            "wall",
            Vec3::new(20.0, 0.0, 20.0),
            Vec3::new(40.0, 10.0, 40.0),
        )];
        let idx = ModIndex::build(&mods, 512.0, 64.0);
        assert!(idx.is_volumetric_at(30.0, 30.0));
        // Far away, outside the authored envelope: the early-out fires.
        assert!(!idx.is_volumetric_at(-200.0, -200.0));
    }

    #[test]
    fn test_lookup_far_outside_the_envelope_costs_no_cell_visit() {
        let mods = vec![boxed("a", Vec3::ZERO, Vec3::splat(4.0))];
        let idx = ModIndex::build(&mods, 4000.0, 64.0);
        // The clamp in cell_of would otherwise map every far point onto a
        // border cell; the bounds guard has to reject it first.
        assert!(idx.candidates_at(-1900.0, -1900.0).is_empty());
        assert!(idx.candidates_at(1900.0, 1900.0).is_empty());
    }

    #[test]
    fn test_a_mod_spanning_cells_is_found_from_every_one_of_them() {
        // 200 m wide over 64 m cells: it must be bucketed into all of them.
        let mods = vec![boxed(
            "ridge",
            Vec3::new(-100.0, 0.0, -5.0),
            Vec3::new(100.0, 30.0, 5.0),
        )];
        let idx = ModIndex::build(&mods, 512.0, 64.0);
        for x in [-90.0, -40.0, 0.0, 40.0, 90.0] {
            assert!(idx.is_volumetric_at(x, 0.0), "missed the ridge at x={x}");
        }
    }

    #[test]
    fn test_candidates_in_dedupes_and_sorts_for_determinism() {
        let mods = vec![
            boxed(
                "a",
                Vec3::new(-50.0, 0.0, -50.0),
                Vec3::new(50.0, 5.0, 50.0),
            ),
            boxed(
                "b",
                Vec3::new(-10.0, 0.0, -10.0),
                Vec3::new(10.0, 5.0, 10.0),
            ),
        ];
        let idx = ModIndex::build(&mods, 512.0, 32.0);
        let found = idx.candidates_in(&Bounds3::from_corners(
            Vec3::splat(-60.0),
            Vec3::splat(60.0),
        ));
        assert_eq!(found, vec![0, 1], "both mods, once each, in order");
    }

    #[test]
    fn test_classify_flags_only_chunks_that_overlap() {
        let mods = vec![boxed(
            "wall",
            Vec3::new(100.0, 0.0, 100.0),
            Vec3::new(120.0, 20.0, 120.0),
        )];
        let idx = ModIndex::build(&mods, 512.0, 64.0);
        let near = Bounds3::from_corners(Vec3::new(96.0, 0.0, 96.0), Vec3::new(160.0, 60.0, 160.0));
        let far = Bounds3::from_corners(
            Vec3::new(-160.0, 0.0, -160.0),
            Vec3::new(-96.0, 60.0, -96.0),
        );
        assert_eq!(idx.classify(&near), ChunkClass::Volumetric);
        assert_eq!(idx.classify(&far), ChunkClass::Flat);
    }

    #[test]
    fn test_a_mod_buried_far_below_still_makes_the_column_volumetric() {
        // A cave under a hill: no vertical overlap with the surface chunk,
        // but the column must still be meshed volumetrically.
        let mods = vec![boxed(
            "cave",
            Vec3::new(0.0, -60.0, 0.0),
            Vec3::new(20.0, -40.0, 20.0),
        )];
        let idx = ModIndex::build(&mods, 512.0, 64.0);
        assert!(idx.is_volumetric_at(10.0, 10.0));
        let surface = Bounds3::from_corners(Vec3::new(0.0, 0.0, 0.0), Vec3::new(64.0, 60.0, 64.0));
        assert_eq!(idx.classify(&surface), ChunkClass::Volumetric);
    }

    #[test]
    fn test_column_span_is_none_when_flat_and_covers_the_mod_when_not() {
        let mods = vec![boxed(
            "arch",
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(20.0, 30.0, 20.0),
        )];
        let idx = ModIndex::build(&mods, 512.0, 64.0);
        assert_eq!(idx.column_span_at(-200.0, -200.0, 5.0), None);
        let (lo, hi) = idx.column_span_at(10.0, 10.0, 5.0).expect("volumetric");
        assert!(lo <= 5.0, "span must reach the ground at {lo}");
        assert!(hi >= 30.0, "span must clear the mod top, got {hi}");
    }

    #[test]
    fn test_column_span_marches_the_candidates_not_the_world_union() {
        // A cave 60 m under one hill and a floating shelf over another: the
        // first column must not march the shelf's altitude and the second must
        // not march the cave's depth — only the union's Y would do that.
        let mods = vec![
            boxed(
                "cave",
                Vec3::new(-60.0, -60.0, -60.0),
                Vec3::new(-40.0, -40.0, -40.0),
            ),
            boxed(
                "shelf",
                Vec3::new(40.0, 120.0, 40.0),
                Vec3::new(60.0, 130.0, 60.0),
            ),
        ];
        let idx = ModIndex::build(&mods, 512.0, 64.0);
        let (_, hi) = idx.column_span_at(-50.0, -50.0, 2.0).expect("cave column");
        assert!(
            hi < 2.01,
            "the cave column marched up to {hi} — that is the world union, \
             not the candidates"
        );
        let (lo, hi) = idx.column_span_at(50.0, 50.0, 2.0).expect("shelf column");
        assert!(
            lo > 1.99 && hi > 119.0,
            "the shelf column spanned [{lo}, {hi}] — the floor is the ground \
             and the ceiling is the shelf, not the world union"
        );
    }

    #[test]
    fn test_degenerate_cell_size_falls_back_instead_of_dividing_by_zero() {
        let idx = ModIndex::empty(512.0, 0.0);
        assert_eq!(idx.cell_size(), DEFAULT_CELL_SIZE);
        let idx = ModIndex::empty(512.0, f32::NAN);
        assert_eq!(idx.cell_size(), DEFAULT_CELL_SIZE);
    }
}

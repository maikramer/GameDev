//! Ground decals — draped, alpha-feathered surface patches.
//!
//! A decal is the ground-cover primitive the world XML uses for a plaza
//! floor, a market apron or a fusion disc at a road junction. Every one of
//! them shares the same three contracts, which is why they live in one
//! generator instead of one hand-rolled mesh per caller:
//!
//! * **Draped, not planar.** A `<Plane>` decal is a single quad at a fixed
//!   `y`; on anything but a perfectly flat pad it clips into the ground on
//!   one side and floats on the other. A decal samples the [`BrushGrid`] at
//!   every vertex and carries **concentric rings** so the interior follows
//!   the terrain too — a centre fan alone is planar between the hub and the
//!   rim, which is what made the 16 m junction discs cut through the slope.
//! * **Organic rim.** The rim radius is modulated by three periodic
//!   harmonics (integer frequencies ⇒ the seam closes exactly), so a patch
//!   reads as a worn area of ground and not as a stamped circle.
//! * **Feathered, never hard-edged.** Alpha ramps 1 → 0 across the feather
//!   band with a smootherstep, so the patch dissolves into the terrain
//!   instead of ending on a cut line.
//!
//! Decals are transparent and **never cast shadows** — a shadow caster
//! hovering `LIFT` above the ground draws its own silhouette back onto the
//! ground, which is exactly the dark ring that made the north-gate junction
//! disc read as a separate object sitting on the grass.

use bevy::math::Vec2;

use super::brush::BrushGrid;
use super::mesh::ChunkMeshData;

/// Lift above the sampled ground (meters). Decals sit **below** the road
/// ribbon so that where the two overlap the ribbon wins deterministically
/// instead of z-fighting; see [`super::roads::RIBBON_LIFT`].
pub const DECAL_LIFT: f32 = 0.04;

/// Target arc length between rim samples (meters).
const RIM_ARC: f32 = 1.5;
/// Rim sample count bounds.
const MIN_SEGMENTS: usize = 24;
const MAX_SEGMENTS: usize = 96;
/// Concentric rings inside the opaque core bounds. The count scales with the
/// patch size so the drape resolution stays near the terrain texel.
const MIN_CORE_RINGS: usize = 2;
const MAX_CORE_RINGS: usize = 12;
/// Rings across the feather band — 3 keeps the alpha ramp curved rather than
/// showing the straight-line banding a single quad ring produces.
const FEATHER_RINGS: usize = 3;

/// A draped ground patch (`<GroundDecal>`, and the fusion disc at a road
/// junction).
#[derive(Debug, Clone, PartialEq)]
pub struct GroundDecalSpec {
    pub name: Option<String>,
    /// World XZ centre.
    pub at: Vec2,
    /// Ellipse radii in X and Z (meters) — equal radii give a circle.
    pub half_extent: Vec2,
    /// Alpha fade band outside the rim (meters).
    pub feather: f32,
    /// Rim wobble as a fraction of the radius (0 = a true ellipse).
    pub noise: f32,
    /// Determinism seed for the rim harmonics.
    pub seed: u32,
    /// Base color map (world asset path).
    pub texture: Option<String>,
    /// Meters per texture repeat — world-space UVs, so a decal and the road
    /// ribbons that cross it keep the same stone grid.
    pub texture_scale: f32,
    /// Material tint under the texture.
    pub base_color: [f32; 3],
    pub roughness: f32,
    /// Lift above the sampled ground (meters).
    pub lift: f32,
}

impl Default for GroundDecalSpec {
    fn default() -> Self {
        Self {
            name: None,
            at: Vec2::ZERO,
            half_extent: Vec2::splat(6.0),
            feather: 2.5,
            noise: 0.1,
            seed: 0,
            texture: None,
            texture_scale: 9.0,
            base_color: [0.62, 0.60, 0.56],
            roughness: 0.9,
            lift: DECAL_LIFT,
        }
    }
}

impl GroundDecalSpec {
    /// Outermost reach including the feather band (meters) — the radius a
    /// caller must clear to know the decal covers a point.
    pub fn outer_reach(&self) -> f32 {
        self.half_extent.max_element() * (1.0 + self.noise) + self.feather.max(0.0)
    }

    /// True when `p` lies inside the opaque core (rim wobble ignored, so the
    /// test is conservative: it only reports points the decal certainly
    /// covers). Used to suppress a junction disc a plaza floor already hides.
    pub fn covers(&self, p: Vec2) -> bool {
        let h = self.half_extent * (1.0 - self.noise).max(0.0);
        if h.x <= 0.0 || h.y <= 0.0 {
            return false;
        }
        let d = (p - self.at) / h;
        d.length_squared() <= 1.0
    }
}

/// Deterministic `[0, 1)` hash of an integer.
fn hash01(i: u32) -> f32 {
    let mut h = i.wrapping_mul(0x27d4_eb2d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x8549_5cb5);
    h ^= h >> 16;
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

/// Rim wobble in `[-1, 1]`, **exactly periodic** in `angle` (integer
/// harmonics), so the last rim sample meets the first without a crack.
fn rim_wobble(seed: u32, angle: f32) -> f32 {
    use std::f32::consts::TAU;
    let p1 = hash01(seed) * TAU;
    let p2 = hash01(seed ^ 0x9e37_79b9) * TAU;
    let p3 = hash01(seed ^ 0x85eb_ca6b) * TAU;
    0.55 * (angle * 3.0 + p1).sin()
        + 0.30 * (angle * 5.0 + p2).sin()
        + 0.15 * (angle * 8.0 + p3).sin()
}

/// `smootherstep` on `[0, 1]` — the alpha ramp across the feather band.
fn smootherstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Builds the decal mesh draped on `grid`.
///
/// Layout: vertex 0 is the centre hub, then `rings × segments` ring vertices.
/// Rings `1..=core_rings` climb from the hub to the wobbled rim at alpha 1;
/// rings beyond that walk outward across the feather band to alpha 0. The rim
/// is closed by modular indexing (no duplicated seam column) — world-space
/// UVs mean there is no UV seam to split for.
pub fn ground_decal_mesh(grid: &BrushGrid, spec: &GroundDecalSpec) -> ChunkMeshData {
    let mut mesh = ChunkMeshData::default();
    let hx = spec.half_extent.x;
    let hz = spec.half_extent.y;
    if hx <= 0.0 || hz <= 0.0 {
        return mesh;
    }
    let scale = if spec.texture_scale > 0.0 {
        spec.texture_scale
    } else {
        1.0
    };
    let feather = spec.feather.max(0.0);
    let noise = spec.noise.clamp(0.0, 0.9);
    let lift = spec.lift;

    let max_r = hx.max(hz);
    let segments = ((std::f32::consts::TAU * max_r / RIM_ARC).ceil() as usize)
        .clamp(MIN_SEGMENTS, MAX_SEGMENTS);
    let core_rings =
        ((max_r / grid.texel().max(0.5)).ceil() as usize).clamp(MIN_CORE_RINGS, MAX_CORE_RINGS);
    let feather_rings = if feather > 0.0 { FEATHER_RINGS } else { 0 };
    let rings = core_rings + feather_rings;

    let mut push = |x: f32, z: f32, alpha: f32| {
        let y = grid.sample(x, z) + lift;
        mesh.positions.push([x, y, z]);
        mesh.normals
            .push(grid.sample_normal(x, z, grid.texel()).to_array());
        mesh.uvs.push([x / scale, z / scale]);
        mesh.colors.push([1.0, 1.0, 1.0, alpha]);
    };

    // Hub.
    push(spec.at.x, spec.at.y, 1.0);

    // Ring vertices. `rim` is the wobbled ellipse point for this angle; core
    // rings scale it toward the hub, feather rings push it outward along the
    // same radial by `feather` meters in total.
    for j in 1..=rings {
        for k in 0..segments {
            let angle = (k as f32) / (segments as f32) * std::f32::consts::TAU;
            let (dx, dz) = (angle.cos(), angle.sin());
            let wobble = 1.0 + noise * rim_wobble(spec.seed, angle);
            let rim = Vec2::new(dx * hx, dz * hz) * wobble;
            let rim_len = rim.length().max(1e-4);
            let (offset, alpha) = if j <= core_rings {
                let t = (j as f32) / (core_rings as f32);
                (rim * t, 1.0)
            } else {
                let f = ((j - core_rings) as f32) / (feather_rings as f32);
                (rim * (1.0 + f * feather / rim_len), 1.0 - smootherstep01(f))
            };
            push(spec.at.x + offset.x, spec.at.y + offset.y, alpha);
        }
    }

    // Indices. Ring `j` starts at `1 + (j - 1) * segments`; `%segments`
    // closes the loop. Winding matches the road ribbon: +Y normal seen from
    // above.
    let idx = |j: usize, k: usize| -> u32 { (1 + (j - 1) * segments + (k % segments)) as u32 };
    for k in 0..segments {
        // Hub fan.
        mesh.indices
            .extend_from_slice(&[0, idx(1, k + 1), idx(1, k)]);
    }
    for j in 1..rings {
        for k in 0..segments {
            let (a0, a1) = (idx(j, k), idx(j, k + 1));
            let (b0, b1) = (idx(j + 1, k), idx(j + 1, k + 1));
            mesh.indices.extend_from_slice(&[a0, a1, b1, a0, b1, b0]);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 128×128 grid over 128 m with a rolling hill, so a decal that fails to
    /// drape shows up as a vertical error against the ground.
    fn test_grid() -> BrushGrid {
        let mut grid =
            BrushGrid::new(vec![0; 128 * 128], 128, 128, 128.0, 50.0, 0.0).expect("grid");
        grid.begin_stroke("base");
        for z in 0..128 {
            for x in 0..128 {
                let p = grid.cell_center(x, z);
                let h = 8.0 + 6.0 * (p.x * 0.05).sin() + 4.0 * (p.y * 0.04).cos();
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        grid
    }

    fn spec() -> GroundDecalSpec {
        GroundDecalSpec {
            half_extent: Vec2::new(8.0, 6.0),
            feather: 2.5,
            noise: 0.12,
            seed: 7,
            texture_scale: 9.0,
            ..GroundDecalSpec::default()
        }
    }

    #[test]
    fn test_decal_mesh_is_non_empty_and_indexed() {
        let mesh = ground_decal_mesh(&test_grid(), &spec());
        assert!(!mesh.positions.is_empty());
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        assert_eq!(mesh.positions.len(), mesh.uvs.len());
        assert_eq!(mesh.positions.len(), mesh.colors.len());
        assert_eq!(mesh.indices.len() % 3, 0);
        let max = *mesh.indices.iter().max().expect("indices");
        assert!(
            (max as usize) < mesh.positions.len(),
            "index {max} out of range for {} vertices",
            mesh.positions.len()
        );
    }

    /// The whole point of the concentric rings: every vertex hugs the ground,
    /// including the interior. A centre fan would only be exact at the hub
    /// and the rim.
    #[test]
    fn test_decal_drapes_on_the_terrain() {
        let grid = test_grid();
        let mesh = ground_decal_mesh(&grid, &spec());
        for p in &mesh.positions {
            let expected = grid.sample(p[0], p[2]) + DECAL_LIFT;
            assert!(
                (p[1] - expected).abs() < 1e-3,
                "vertex {p:?} off the ground (expected y {expected})"
            );
        }
    }

    /// Alpha must reach 0 on the outermost ring — that is what dissolves the
    /// patch into the terrain instead of ending it on a cut line.
    #[test]
    fn test_decal_feathers_to_zero_alpha() {
        let mesh = ground_decal_mesh(&test_grid(), &spec());
        let min = mesh
            .colors
            .iter()
            .map(|c| c[3])
            .fold(f32::INFINITY, f32::min);
        let max = mesh
            .colors
            .iter()
            .map(|c| c[3])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(min.abs() < 1e-6, "outer ring alpha should be 0, got {min}");
        assert!(
            (max - 1.0).abs() < 1e-6,
            "core alpha should be 1, got {max}"
        );
    }

    /// No feather ⇒ no feather rings, and the rim stays opaque.
    #[test]
    fn test_decal_without_feather_stays_opaque() {
        let mesh = ground_decal_mesh(
            &test_grid(),
            &GroundDecalSpec {
                feather: 0.0,
                ..spec()
            },
        );
        assert!(mesh.colors.iter().all(|c| (c[3] - 1.0).abs() < 1e-6));
    }

    /// The rim must not be a circle — that is the "stamped disc" look.
    #[test]
    fn test_rim_wobble_breaks_the_circle() {
        let s = GroundDecalSpec {
            half_extent: Vec2::splat(8.0),
            noise: 0.15,
            ..spec()
        };
        let mesh = ground_decal_mesh(&test_grid(), &s);
        // Core rim ring: the last opaque ring. Collect its radii.
        let radii: Vec<f32> = mesh
            .positions
            .iter()
            .map(|p| Vec2::new(p[0], p[2]).distance(s.at))
            .collect();
        let spread = radii.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b))
            - radii.iter().fold(f32::INFINITY, |a, b| a.min(*b));
        assert!(spread > 1.0, "rim radii too uniform (spread {spread})");
    }

    /// Periodic harmonics: the wobble at angle 0 and at TAU must agree, or
    /// the rim shows a crack at the seam.
    #[test]
    fn test_rim_wobble_is_periodic() {
        for seed in [0u32, 7, 1234, 0xdead_beef] {
            let a = rim_wobble(seed, 0.0);
            let b = rim_wobble(seed, std::f32::consts::TAU);
            assert!((a - b).abs() < 1e-4, "seed {seed}: {a} != {b}");
        }
    }

    #[test]
    fn test_covers_is_conservative() {
        let s = spec();
        assert!(s.covers(s.at), "the centre is always covered");
        assert!(
            !s.covers(s.at + Vec2::new(s.outer_reach() + 1.0, 0.0)),
            "a point past the feather is not covered"
        );
    }

    #[test]
    fn test_degenerate_extent_yields_no_mesh() {
        let mesh = ground_decal_mesh(
            &test_grid(),
            &GroundDecalSpec {
                half_extent: Vec2::ZERO,
                ..spec()
            },
        );
        assert!(mesh.positions.is_empty());
        assert!(mesh.indices.is_empty());
    }
}

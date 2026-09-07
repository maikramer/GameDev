//! Buffer types and the height/slope tint shared by the voxel pipeline.
//!
//! O terreno é 100% volumétrico: o mesher é o transvoxel de
//! [`super::voxel::transvoxel_mesh`], e o que aqui sobrevive é a infraestrutura
//! comum — [`HeightField`] (o termo-base que o [`super::voxel::VoxelField`]
//! lê), [`ChunkMeshData`] (buffers que o pipeline voxel consome) e o tint de
//! altura/inclinação em vertex colors ([`tint_vertex_color`], o caminho
//! legado de look e a base do canal RGBA que o shader de layers
//! reinterpreta).

use bevy::math::Vec3;

/// Read-only height queries used by mesh/collider building.
///
/// Implemented by [`crate::terrain::sampler::HeightSampler`]. All coordinates
/// are world-space meters; heights are meters (`normalized * max_height`).
pub trait HeightField {
    /// Terrain height at a world XZ position (smoothed per the sampler mode).
    fn sample(&self, world_x: f32, world_z: f32) -> f32;
    /// Surface normal via central differences with the given world epsilon.
    /// Callers pass a terrain-wide constant epsilon (not per-chunk/per-LOD)
    /// so normals agree on shared chunk borders.
    fn sample_normal(&self, world_x: f32, world_z: f32, epsilon: f32) -> Vec3;
    /// Peak height of the heightfield (meters).
    fn max_height(&self) -> f32;
    /// Raw min/max height over the axis-aligned world XZ range (meters).
    /// `None` = the field cannot answer range queries (the mesh builder then
    /// falls back to plain point sampling).
    fn range_over(&self, _min_x: f32, _min_z: f32, _max_x: f32, _max_z: f32) -> Option<(f32, f32)> {
        None
    }
}

/// Vertex/index buffers for one terrain chunk mesh.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChunkMeshData {
    /// Vertex positions, world-origin relative to the chunk entity
    /// (x/z relative to the chunk center, y absolute meters).
    pub positions: Vec<[f32; 3]>,
    /// Smooth normals.
    pub normals: Vec<[f32; 3]>,
    /// UVs (world-space tiled, see [`build_chunk_mesh`]).
    pub uvs: Vec<[f32; 2]>,
    /// RGBA vertex colors from the height/slope tint.
    pub colors: Vec<[f32; 4]>,
    /// Triangle indices.
    pub indices: Vec<u32>,
}

/// Height/slope banding parameters for [`build_chunk_mesh`] (the render-side
/// subset of [`crate::terrain::spec::TerrainTint`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TintParams {
    pub base_color: [f32; 4],
    pub color_low: [f32; 4],
    pub color_mid: [f32; 4],
    pub color_high: [f32; 4],
    pub color_rock: [f32; 4],
    pub snow_height: f32,
    pub slope_threshold: f32,
    pub slope_softness: f32,
    /// `0` disables the banding entirely: every vertex gets `base_color`, so
    /// the surface is the material texture times the authored base colour and
    /// nothing else.
    pub height_blend_strength: f32,
}

impl Default for TintParams {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            color_low: [1.0; 4],
            color_mid: [1.0; 4],
            color_high: [1.0; 4],
            color_rock: [1.0; 4],
            snow_height: 1.0,
            slope_threshold: 1.0,
            slope_softness: 0.05,
            height_blend_strength: 0.0,
        }
    }
}

impl From<&crate::terrain::spec::TerrainTint> for TintParams {
    fn from(t: &crate::terrain::spec::TerrainTint) -> Self {
        // Linear, not sRGB. `Mesh::ATTRIBUTE_COLOR` is consumed by the PBR
        // shader as linear RGBA, so handing it sRGB components renders every
        // band far too bright (sRGB 0.29 read as linear is ~4x the intended
        // 0.068) — which is why the tint used to wash out to near-white.
        let conv = |c: bevy::color::Color| {
            let l = c.to_linear();
            [l.red, l.green, l.blue, l.alpha]
        };
        Self {
            base_color: conv(t.base_color),
            color_low: conv(t.color_low),
            color_mid: conv(t.color_mid),
            color_high: conv(t.color_high),
            color_rock: conv(t.color_rock),
            snow_height: t.snow_height,
            slope_threshold: t.slope_threshold,
            slope_softness: t.slope_softness,
            height_blend_strength: t.height_blend_strength,
        }
    }
}

const MID_BAND_CENTER: f32 = 0.35;
const HIGH_BAND_CENTER: f32 = 0.7;
/// Half-width of the smoothstep between two altitude bands (fraction of 0..1).
const BAND_HALF_WIDTH: f32 = 0.25;
/// Snow target color blended in above `snow_height`.
const SNOW_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Neutral the band colour is mixed against: white leaves the diffuse texture
/// untouched at `height-blend-strength: 0`.
const NEUTRAL_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// `mix(white, band, height_blend_strength) * base_color`. That matters
/// because the terrain also carries a diffuse texture, and the shader
/// multiplies the two. An earlier version handed the raw band colour to the
/// shader, so `color-low: #35502a` (linear ~0.04) multiplied the grass
/// texture down to near black and the whole tint had to be ripped out. With
/// the mix, `height-blend-strength` is exactly what its name says — how far
/// the surface is pushed towards the authored band colour — and `0` leaves
/// the texture times `base-color`, i.e. no tint at all.
pub fn tint_vertex_color(y: f32, normal_y: f32, max_height: f32, tint: &TintParams) -> [f32; 4] {
    let strength = tint.height_blend_strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return tint.base_color;
    }
    let h = if max_height > 0.0 {
        (y / max_height).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Slope: 0 flat, 1 vertical.
    let slope = 1.0 - normal_y.clamp(0.0, 1.0);
    let width = BAND_HALF_WIDTH;

    // Three-band altitude blend: low → mid → high, softstepped at the knots.
    let mid_band = smoothstep(MID_BAND_CENTER - width, MID_BAND_CENTER + width, h);
    let high_band = smoothstep(HIGH_BAND_CENTER - width, HIGH_BAND_CENTER + width, h);
    let mut color = mix4(tint.color_low, tint.color_mid, mid_band);
    color = mix4(color, tint.color_high, high_band);

    // Snow: above `snow_height` the high color fades to white.
    let snow_band = smoothstep(tint.snow_height - width, tint.snow_height + width, h);
    color = mix4(color, SNOW_COLOR, snow_band);

    // Rock on steep faces wins over altitude and snow (a cliff at the snow
    // line should not read as pure white).
    let rock_band = smoothstep(
        tint.slope_threshold - tint.slope_softness,
        tint.slope_threshold + tint.slope_softness,
        slope,
    );
    color = mix4(color, tint.color_rock, rock_band);

    // Modulate, then apply the authored base colour.
    let color = mix4(NEUTRAL_COLOR, color, strength);
    [
        color[0] * tint.base_color[0],
        color[1] * tint.base_color[1],
        color[2] * tint.base_color[2],
        color[3] * tint.base_color[3],
    ]
}

/// GLSL-style `smoothstep(edge0, edge1, x)` with a guard for degenerate edges
/// (`edge1 <= edge0` collapses to a hard step instead of dividing by zero).
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Linear blend of two RGBA colors: `a` at `t = 0`, `b` at `t = 1`.
fn mix4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tint_modulates_instead_of_replacing() {
        // The whole point of the mix: at full strength the band colour lands
        // as authored, but at the strengths worlds actually use it only
        // *pushes* the texture, instead of multiplying it down to black.
        let mut tint = TintParams {
            base_color: [1.0; 4],
            color_low: [0.04, 0.09, 0.02, 1.0],
            color_mid: [0.5, 0.5, 0.5, 1.0],
            color_high: [0.9, 0.9, 1.0, 1.0],
            color_rock: [0.2, 0.18, 0.15, 1.0],
            snow_height: 0.92,
            slope_threshold: 0.93,
            slope_softness: 0.05,
            height_blend_strength: 0.22,
        };
        // Valley floor, flat: the low band at 22% barely darkens the texture.
        let low = tint_vertex_color(10.0, 1.0, 200.0, &tint);
        assert!(
            low[1] > 0.7,
            "22% strength must not crush the texture: {low:?}"
        );
        assert!(
            low[0] < low[1],
            "and it still pushes towards green: {low:?}"
        );

        // Same vertex at full strength: the authored band colour, as-is.
        tint.height_blend_strength = 1.0;
        let full = tint_vertex_color(10.0, 1.0, 200.0, &tint);
        assert!((full[1] - tint.color_low[1]).abs() < 1e-5, "{full:?}");

        // Zero strength is an exact no-op: just the base colour.
        tint.height_blend_strength = 0.0;
        tint.base_color = [0.58, 0.55, 0.49, 1.0];
        assert_eq!(tint_vertex_color(10.0, 1.0, 200.0, &tint), tint.base_color);
    }

    #[test]
    fn test_tint_snow_on_peaks_and_rock_on_cliffs() {
        let tint = TintParams {
            base_color: [1.0; 4],
            color_low: [0.04, 0.09, 0.02, 1.0],
            color_mid: [0.2, 0.3, 0.1, 1.0],
            color_high: [0.3, 0.3, 0.35, 1.0],
            color_rock: [0.6, 0.1, 0.1, 1.0], // vivid, so the band is obvious
            snow_height: 0.8,
            slope_threshold: 0.5,
            slope_softness: 0.05,
            height_blend_strength: 1.0,
        };
        // High and flat → snow white.
        let peak = tint_vertex_color(199.0, 1.0, 200.0, &tint);
        assert!(peak.iter().take(3).all(|c| *c > 0.95), "snow: {peak:?}");
        // Just as high but vertical → rock wins over snow.
        let cliff = tint_vertex_color(199.0, 0.0, 200.0, &tint);
        assert!(cliff[0] > cliff[1], "rock beats snow on a cliff: {cliff:?}");
    }

}

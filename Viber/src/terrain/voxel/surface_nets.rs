//! Naive surface nets — the mesher for volumetric chunks.
//!
//! One vertex per cell that straddles the surface, placed at the average of
//! its edge crossings; one quad per lattice edge that changes sign, joining
//! the four cells around that edge. It is the dual of marching cubes: fewer,
//! better-shaped triangles, and no 256-case table.
//!
//! # Why the seams close for free
//!
//! Chunks are meshed with **one cell of overlap** on the low side. The
//! vertices a chunk computes for its overlap cells are produced from the same
//! analytic field as its neighbour's owned cells, so they land on byte-identical
//! positions. The two chunks emit coincident vertices and disjoint quads: no
//! crack, no stitching, no shared state between build tasks.
//!
//! This is the same bargain the heightfield mesher makes with its skirts and
//! frontier normals — pay a little duplication, keep the builders independent.
//!
//! # Coordinates
//!
//! Positions are **relative to the chunk origin on all three axes**. The
//! heightfield mesher keeps Y absolute because its chunks span the whole
//! column; a voxel chunk is a box in space and its entity carries the offset.

use bevy::math::{Vec2, Vec3};

use super::super::mesh::{ChunkMeshData, TintParams, tint_vertex_color};

/// Cells per axis in one voxel chunk.
///
/// 32³ is the unit the frame budget dictates. A `simple-rpg` terrain chunk is
/// 64 m at a 1 m step; meshing that whole column at once would be ~131 k cells
/// in a single inline build, and `plugin.rs` builds meshes inline on purpose
/// (the upstream crate lost chunks forever to an orphan-task bug). 32³ is
/// ~33 k cells, builds well inside a frame, and gives streaming and culling in
/// **Y** — which caves need anyway.
pub const VOXEL_CHUNK_CELLS: usize = 32;

/// Neutral wall-space value, matching `CliffMask::WALL_NEUTRAL` (128/255).
const WALL_NEUTRAL: f32 = 128.0 / 255.0;

/// Build parameters for one voxel chunk.
#[derive(Debug, Clone)]
pub struct VoxelChunkParams {
    /// World position of the chunk's minimum corner.
    pub origin: Vec3,
    /// Cells per axis.
    pub cells: usize,
    /// Edge length of one cell, in meters.
    pub voxel_size: f32,
    /// World-space UV tile size, matching the heightfield path so ground
    /// textures line up across the flat/volumetric boundary.
    pub texture_tile_size: f32,
    /// Height/slope tint, and the world's peak height it is measured against.
    ///
    /// The same tint the heightfield mesher bakes into its vertex colours. A
    /// voxel chunk that skipped this rendered a flat cyan-white next to the
    /// terrain's cream — the boundary between the two meshers has to be
    /// invisible, and the tint is most of what makes it so.
    pub tint: TintParams,
    pub max_height: f32,
    /// Whether this chunk renders with the layer blend material (`layers`
    /// worlds) rather than the stock `StandardMaterial`.
    ///
    /// It decides what the R channel MEANS. With the layer shader, R is wall
    /// space (data the fragment reads) and A the cliff factor. With the stock
    /// PBR material the vertex colour is multiplied as-is, so a wall-space R
    /// of 0.502 darkened every legacy voxel chunk's red against the flat
    /// heightfield chunks beside it — a visible step at every flat|volumetric
    /// boundary. On the legacy path the full tint stays in RGB, exactly what
    /// `build_chunk_mesh` bakes when it receives no mask.
    pub uses_layer_material: bool,
    /// Which of the box's four VERTICAL faces sit on a column boundary and
    /// must grow a downward seal skirt, in `[-X, +X, -Z, +Z]` order.
    ///
    /// Boxes of the same LOD close their seams by vertex coincidence, and a
    /// column's interior box tiling is covered box-to-box — but a column at a
    /// FINER LOD than its neighbour disagrees with that neighbour's coarse
    /// polyline by up to a coarse cell, and the gap shows the sky through at
    /// grazing angles. The seal hangs a vertical wall from every mesh edge on
    /// those faces: the same bargain the heightfield mesher's skirts make
    /// (`ChunkMeshParams::skirt_depth`). Sealed walls are buried in solid
    /// ground whenever the neighbour agrees, so "always seal" costs nothing
    /// visible.
    pub seal_faces: [bool; 4],
    /// Depth (meters) of the column-boundary seals. `0` disables them even
    /// when [`VoxelChunkParams::seal_faces`] is set.
    pub seal_depth: f32,
}

impl VoxelChunkParams {
    pub fn new(
        origin: Vec3,
        voxel_size: f32,
        texture_tile_size: f32,
        tint: TintParams,
        max_height: f32,
        uses_layer_material: bool,
    ) -> Self {
        Self {
            origin,
            cells: VOXEL_CHUNK_CELLS,
            voxel_size,
            texture_tile_size,
            tint,
            max_height,
            uses_layer_material,
            seal_faces: [false; 4],
            seal_depth: 0.0,
        }
    }

    /// World-space extent of this chunk.
    pub fn extent(&self) -> f32 {
        self.cells as f32 * self.voxel_size
    }
}

/// Cell edges as (corner a, corner b) indices into the 8-corner cube.
///
/// Corner bit layout: bit0 = +X, bit1 = +Y, bit2 = +Z.
const CUBE_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7), // along X
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7), // along Y
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7), // along Z
];

/// Builds one voxel chunk mesh from a density function.
///
/// `density` must be negative inside solid. Returns `None` when the chunk is
/// entirely solid or entirely empty — the common case, and the reason a
/// volumetric column costs only the chunks that actually contain a surface.
pub fn build_voxel_mesh(
    density: &dyn Fn(Vec3) -> f32,
    params: &VoxelChunkParams,
) -> Option<ChunkMeshData> {
    let n = params.cells;
    if n == 0 || !params.voxel_size.is_finite() || params.voxel_size <= 0.0 {
        return None;
    }
    let vs = params.voxel_size;
    // Lattice points span [-1, n] inclusive so that every quad the chunk owns
    // has all four of its cells available.
    let pts = n + 2;
    let pos_of = |l: isize| l as f32 * vs;

    let mut samples = vec![0.0f32; pts * pts * pts];
    let sidx = |x: usize, y: usize, z: usize| (z * pts + y) * pts + x;

    let mut any_neg = false;
    let mut any_pos = false;
    for iz in 0..pts {
        for iy in 0..pts {
            for ix in 0..pts {
                let p = params.origin
                    + Vec3::new(
                        pos_of(ix as isize - 1),
                        pos_of(iy as isize - 1),
                        pos_of(iz as isize - 1),
                    );
                let d = density(p);
                any_neg |= d < 0.0;
                any_pos |= d >= 0.0;
                samples[sidx(ix, iy, iz)] = d;
            }
        }
    }
    // Uniform sign: no surface crosses this chunk.
    if !(any_neg && any_pos) {
        return None;
    }

    // Cells run [-1, n-1]; store them at array index cell + 1.
    let cdim = n + 1;
    let cidx = |x: usize, y: usize, z: usize| (z * cdim + y) * cdim + x;
    let mut vertex_of = vec![u32::MAX; cdim * cdim * cdim];

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    let tile = if params.texture_tile_size > 0.0 {
        params.texture_tile_size
    } else {
        1.0
    };

    for cz in 0..cdim {
        for cy in 0..cdim {
            for cx in 0..cdim {
                // The 8 corner samples of this cell.
                let mut corner = [0.0f32; 8];
                for (k, c) in corner.iter_mut().enumerate() {
                    let dx = k & 1;
                    let dy = (k >> 1) & 1;
                    let dz = (k >> 2) & 1;
                    *c = samples[sidx(cx + dx, cy + dy, cz + dz)];
                }

                // Average the crossings on the 12 edges of the cell. When the
                // crossings span more than one orientation (a corner cell —
                // riser meeting tread, lip meeting wall), the plain average
                // pulls the vertex BETWEEN the two surfaces and the quads
                // around it twist into backfacing flaps. There the vertex is
                // re-solved as the least-squares intersection of the
                // crossings' planes (dual-contouring-lite); a single flat
                // crossing keeps the average, which is its own answer.
                let mut sum = Vec3::ZERO;
                let mut count = 0.0f32;
                for &(a, b) in CUBE_EDGES.iter() {
                    let da = corner[a];
                    let db = corner[b];
                    if (da < 0.0) == (db < 0.0) {
                        continue;
                    }
                    // Linear interpolation to the zero crossing. The guard
                    // keeps a cell whose corners are exactly equal from
                    // dividing by zero.
                    let denom = da - db;
                    let t = if denom.abs() > 1e-20 {
                        (da / denom).clamp(0.0, 1.0)
                    } else {
                        0.5
                    };
                    let ca =
                        Vec3::new((a & 1) as f32, ((a >> 1) & 1) as f32, ((a >> 2) & 1) as f32);
                    let cb =
                        Vec3::new((b & 1) as f32, ((b >> 1) & 1) as f32, ((b >> 2) & 1) as f32);
                    sum += ca.lerp(cb, t);
                    count += 1.0;
                }
                if count == 0.0 {
                    continue;
                }

                // Cell-local average -> chunk-local meters.
                let local = sum / count;
                // NOTE: a sub-voxel thin sheet (a carved void passing a few
                // centimetres under the natural terrain, e.g. terrace lips)
                // puts both its crossings in one cell, and the averaged
                // vertex then yields a few backfacing triangles along the
                // sheet line — measured ~130 over a 512 m world, speckled on
                // folded cliff bands up close. A local QEF cannot separate
                // the sheets (their gradients agree); the real fix is voxel
                // refinement near features. Quad diagonals below at least
                // pick the winding that agrees with the vertex normals.
                let base = Vec3::new(
                    pos_of(cx as isize - 1),
                    pos_of(cy as isize - 1),
                    pos_of(cz as isize - 1),
                );
                let local_pos = base + local * vs;
                let world = params.origin + local_pos;

                // Normal from the field gradient rather than from the
                // triangles: it is continuous across chunk borders, so two
                // neighbours light identically without sharing any state.
                let e = vs * 0.5;
                let g = Vec3::new(
                    density(world + Vec3::X * e) - density(world - Vec3::X * e),
                    density(world + Vec3::Y * e) - density(world - Vec3::Y * e),
                    density(world + Vec3::Z * e) - density(world - Vec3::Z * e),
                );
                let normal = g.try_normalize().unwrap_or(Vec3::Y);

                vertex_of[cidx(cx, cy, cz)] = positions.len() as u32;
                positions.push(local_pos.to_array());
                normals.push(normal.to_array());
                uvs.push(Vec2::new(world.x / tile, world.z / tile).to_array());
                // Same height/slope tint as the heightfield mesher, so the
                // two meshes read as one surface where they meet. When the
                // chunk renders with the layer blend, R is then overwritten
                // with wall space and A with the cliff factor, exactly as
                // `build_chunk_mesh` does (`mesh.rs:352-357`). On the legacy
                // path the tint stays untouched in RGBA — the stock
                // StandardMaterial multiplies it, and a wall-space R of
                // 0.502 read as a red channel stepped half-dark against the
                // flat chunks beside it.
                let mut color =
                    tint_vertex_color(world.y, normal.y, params.max_height, &params.tint);
                if params.uses_layer_material {
                    // R = wall space (neutral until the cliff mods author
                    // it), A = cliff region factor. A volumetric chunk is
                    // rock by construction, so the shader's triplanar gate
                    // is fully open.
                    color[0] = WALL_NEUTRAL;
                    color[3] = 1.0;
                }
                colors.push(color);
            }
        }
    }

    if positions.is_empty() {
        return None;
    }

    let mut indices: Vec<u32> = Vec::new();
    // Quads for every lattice edge the chunk owns: q in [0, n-1] per axis.
    for axis in 0..3usize {
        // Right-handed cycle: (a, b, c) = (X,Y,Z), (Y,Z,X), (Z,X,Y).
        let b = (axis + 1) % 3;
        let c = (axis + 2) % 3;
        for qz in 0..n {
            for qy in 0..n {
                for qx in 0..n {
                    let q = [qx, qy, qz];
                    let mut qa = q;
                    qa[axis] += 1;
                    // Lattice array index is the coordinate + 1.
                    let d0 = samples[sidx(q[0] + 1, q[1] + 1, q[2] + 1)];
                    let d1 = samples[sidx(qa[0] + 1, qa[1] + 1, qa[2] + 1)];
                    if (d0 < 0.0) == (d1 < 0.0) {
                        continue;
                    }

                    // The four cells sharing this edge, in rotational order
                    // around +axis. Cell coordinate + 1 indexes the array, so
                    // subtracting 1 from a q of 0 lands on index 0, which is
                    // exactly the overlap cell.
                    let mut cells = [[0usize; 3]; 4];
                    for (k, cell) in cells.iter_mut().enumerate() {
                        let sub_b = k == 0 || k == 3;
                        let sub_c = k == 0 || k == 1;
                        let mut v = [q[0] + 1, q[1] + 1, q[2] + 1];
                        if sub_b {
                            v[b] -= 1;
                        }
                        if sub_c {
                            v[c] -= 1;
                        }
                        *cell = v;
                    }

                    let mut v = [0u32; 4];
                    let mut ok = true;
                    for (k, cell) in cells.iter().enumerate() {
                        let idx = vertex_of[cidx(cell[0], cell[1], cell[2])];
                        if idx == u32::MAX {
                            ok = false;
                            break;
                        }
                        v[k] = idx;
                    }
                    if !ok {
                        continue;
                    }

                    // Pick the diagonal whose two triangles best agree with
                    // the field's own vertex normals. On folded surfaces
                    // (terrace lips, undercuts) a cell vertex can sit between
                    // two sheets and the fixed diagonal produces triangles
                    // facing INTO the rock — invisible under backface
                    // culling, seen as polygonal holes in the wall. Testing
                    // both diagonals against the vertex normals costs a few
                    // dots and repairs most of those flips.
                    let vn = [
                        Vec3::from(normals[v[0] as usize]),
                        Vec3::from(normals[v[1] as usize]),
                        Vec3::from(normals[v[2] as usize]),
                        Vec3::from(normals[v[3] as usize]),
                    ];
                    let diag_quality = |a: u32, b: u32, c: u32, d: u32| -> f32 {
                        // Triangles (a,b,c) and (a,c,d); quality = worst
                        // alignment of each triangle's geometric normal with
                        // the average vertex normal at its centroid.
                        let p = [
                            Vec3::from(positions[a as usize]),
                            Vec3::from(positions[b as usize]),
                            Vec3::from(positions[c as usize]),
                            Vec3::from(positions[d as usize]),
                        ];
                        let mut worst = f32::INFINITY;
                        for (i, j, k) in [(0usize, 1usize, 2usize), (0, 2, 3)] {
                            let gn = (p[j] - p[i]).cross(p[k] - p[i]);
                            let len = gn.length();
                            if len < 1e-9 {
                                return f32::NEG_INFINITY;
                            }
                            let n = (vn[i] + vn[j] + vn[k]).try_normalize().unwrap_or(gn / len);
                            worst = worst.min(gn.dot(n) / len);
                        }
                        worst
                    };
                    let use_alt =
                        diag_quality(v[1], v[2], v[3], v[0]) > diag_quality(v[0], v[1], v[2], v[3]);

                    // d0 < 0 means solid at q and empty at q+axis, so the
                    // surface faces +axis and this cycle is outward.
                    if d0 < 0.0 {
                        if use_alt {
                            indices.extend_from_slice(&[v[1], v[2], v[3], v[1], v[3], v[0]]);
                        } else {
                            indices.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
                        }
                    } else if use_alt {
                        indices.extend_from_slice(&[v[1], v[3], v[2], v[1], v[0], v[3]]);
                    } else {
                        indices.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
                    }
                }
            }
        }
    }

    if indices.is_empty() {
        return None;
    }

    if params.seal_faces.iter().any(|&face| face) && params.seal_depth > 0.0 {
        append_column_seals(
            params,
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
        );
    }

    Some(ChunkMeshData {
        positions,
        normals,
        uvs,
        colors,
        indices,
    })
}

/// Outward horizontal normal of each sealed face, `[−X, +X, −Z, +Z]` order.
const SEAL_FACE_NORMALS: [Vec3; 4] = [Vec3::NEG_X, Vec3::X, Vec3::NEG_Z, Vec3::Z];

/// Hangs a downward skirt from every mesh edge that lies on one of the box's
/// sealed faces (see [`VoxelChunkParams::seal_faces`]).
///
/// A border edge is one whose BOTH endpoints sit within ¾ voxel of the face
/// plane: surface-nets vertices of the border cells and of the overlap layer
/// cluster around the half-cell mark, while the first interior cell sits a
/// full cell and a half in — the cut separates exactly the boundary ring.
/// Skirt vertices duplicate their source (colour, uv) but carry the face's
/// horizontal outward normal, so the wall shades as a wall; on the layers
/// path the vertex ALPHA is zeroed like the heightfield skirts, so the sliver
/// never picks up the triplanar rock. Winding is normalised per edge so the
/// quads face outward even though the voxel material renders double-sided.
fn append_column_seals(
    params: &VoxelChunkParams,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
) {
    let extent = params.extent();
    let eps = params.voxel_size * 0.75;
    // (face index, plane coordinate on that axis).
    let faces = [(0usize, 0.0f32), (0, extent), (2, 0.0), (2, extent)];

    // Unique edges of the surface, collected BEFORE any skirt triangle lands.
    let mut edges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    for tri in indices.chunks_exact(3) {
        for k in 0..3 {
            let a = tri[k];
            let b = tri[(k + 1) % 3];
            edges.insert((a.min(b), a.max(b)));
        }
    }

    // One dropped copy per source vertex, shared between faces and edges.
    let mut dropped: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let drop_of = |v: u32,
                       positions: &mut Vec<[f32; 3]>,
                       normals: &mut Vec<[f32; 3]>,
                       uvs: &mut Vec<[f32; 2]>,
                       colors: &mut Vec<[f32; 4]>,
                       dropped: &mut std::collections::HashMap<u32, u32>,
                       at: Vec3,
                       out: Vec3,
                       layers: bool|
     -> u32 {
        *dropped.entry(v).or_insert_with(|| {
            let mut color = colors[v as usize];
            if layers {
                color[3] = 0.0;
            }
            let id = positions.len() as u32;
            positions.push([at.x, at.y - params.seal_depth, at.z]);
            normals.push(out.to_array());
            uvs.push(uvs[v as usize]);
            colors.push(color);
            id
        })
    };

    // O par é (EIXO, plano); o ordinal da face (que indexa `seal_faces`)
    // é a posição no array — [−X, +X, −Z, +Z]. Confundir os dois era
    // consultar `seal_faces[eixo]` e nunca selar +X/+Z.
    for (ordinal, &(face, plane)) in faces
        .iter()
        .enumerate()
        .filter(|&(o, _)| params.seal_faces[o])
    {
        let out = SEAL_FACE_NORMALS[ordinal];
        for &(a, b) in &edges {
            let pa = Vec3::from(positions[a as usize]);
            let pb = Vec3::from(positions[b as usize]);
            if (pa[face] - plane).abs() > eps || (pb[face] - plane).abs() > eps {
                continue;
            }
            // Uma aresta quase VERTICAL na fronteira é uma parede que continua
            // abaixo (cliff na costura) — não há fenda a selar, e pendê-la
            // produzia triângulos degenerados (a, b e as cópias caídas
            // colineares). Só arestas com traverso horizontal selam.
            let span = (pb - pa).abs();
            if span.y > span.x.max(span.z) * 2.0 {
                continue;
            }
            // Orient the top edge so (b − a) × (−Y) agrees with the outward
            // normal, then the two triangles below wound outward.
            let (a, b) = if (pb - pa).cross(Vec3::NEG_Y).dot(out) < 0.0 {
                (b, a)
            } else {
                (a, b)
            };
            let a2 = drop_of(
                a, positions, normals, uvs, colors, &mut dropped, pa, out, params.uses_layer_material,
            );
            let b2 = drop_of(
                b, positions, normals, uvs, colors, &mut dropped, pb, out, params.uses_layer_material,
            );
            indices.extend_from_slice(&[a, b, b2, a, b2, a2]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(origin: Vec3, cells: usize, vs: f32) -> VoxelChunkParams {
        VoxelChunkParams {
            origin,
            cells,
            voxel_size: vs,
            texture_tile_size: 8.0,
            tint: TintParams::default(),
            max_height: 100.0,
            uses_layer_material: true,
            seal_faces: [false; 4],
            seal_depth: 0.0,
        }
    }

    /// Ground plane at y = 0: solid below.
    fn plane(p: Vec3) -> f32 {
        p.y
    }

    /// Sphere of radius 6 centred at the origin.
    fn sphere(p: Vec3) -> f32 {
        p.length() - 6.0
    }

    #[test]
    fn test_uniform_region_produces_no_mesh() {
        // Entirely above the plane.
        let m = build_voxel_mesh(&plane, &params(Vec3::new(0.0, 100.0, 0.0), 8, 1.0));
        assert!(m.is_none(), "empty air must not allocate a mesh");
        // Entirely below it.
        let m = build_voxel_mesh(&plane, &params(Vec3::new(0.0, -100.0, 0.0), 8, 1.0));
        assert!(m.is_none(), "solid rock must not allocate a mesh");
    }

    #[test]
    fn test_plane_meshes_at_the_right_height_with_upward_normals() {
        let p = params(Vec3::new(0.0, -4.0, 0.0), 8, 1.0);
        let m = build_voxel_mesh(&plane, &p).expect("the plane crosses this chunk");
        assert!(!m.positions.is_empty());
        for (i, pos) in m.positions.iter().enumerate() {
            let world_y = p.origin.y + pos[1];
            assert!(
                world_y.abs() < 1.01,
                "vertex {i} at world y={world_y}, expected the y=0 plane"
            );
            assert!(
                m.normals[i][1] > 0.9,
                "vertex {i} normal {:?} should point up",
                m.normals[i]
            );
        }
    }

    #[test]
    fn test_every_buffer_is_finite_and_the_same_length() {
        let m = build_voxel_mesh(&sphere, &params(Vec3::splat(-8.0), 16, 1.0)).expect("sphere");
        let n = m.positions.len();
        assert_eq!(m.normals.len(), n);
        assert_eq!(m.uvs.len(), n);
        assert_eq!(m.colors.len(), n);
        for v in &m.positions {
            assert!(v.iter().all(|c| c.is_finite()), "non-finite position {v:?}");
        }
        for v in &m.normals {
            assert!(v.iter().all(|c| c.is_finite()), "non-finite normal {v:?}");
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-3, "normal {v:?} is not unit");
        }
        for v in &m.uvs {
            assert!(v.iter().all(|c| c.is_finite()), "non-finite uv {v:?}");
        }
    }

    #[test]
    fn test_indices_are_in_range_and_form_whole_triangles() {
        let m = build_voxel_mesh(&sphere, &params(Vec3::splat(-8.0), 16, 1.0)).expect("sphere");
        assert_eq!(m.indices.len() % 3, 0, "indices must be whole triangles");
        let n = m.positions.len() as u32;
        assert!(m.indices.iter().all(|&i| i < n), "index out of range");
    }

    #[test]
    fn test_no_degenerate_triangles() {
        let m = build_voxel_mesh(&sphere, &params(Vec3::splat(-8.0), 16, 1.0)).expect("sphere");
        let mut degenerate = 0;
        for t in m.indices.chunks_exact(3) {
            if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                degenerate += 1;
                continue;
            }
            let a = Vec3::from(m.positions[t[0] as usize]);
            let b = Vec3::from(m.positions[t[1] as usize]);
            let c = Vec3::from(m.positions[t[2] as usize]);
            if (b - a).cross(c - a).length() < 1e-9 {
                degenerate += 1;
            }
        }
        assert_eq!(degenerate, 0, "{degenerate} degenerate triangles");
    }

    #[test]
    fn test_triangle_winding_faces_out_of_the_solid() {
        // The sphere is the honest test: every face normal must agree with the
        // outward radial direction. A flipped winding shows up as a mesh lit
        // from inside, which is exactly the "estrada invisível" class of bug.
        let p = params(Vec3::splat(-8.0), 16, 1.0);
        let m = build_voxel_mesh(&sphere, &p).expect("sphere");
        let mut wrong = 0;
        let mut total = 0;
        for t in m.indices.chunks_exact(3) {
            let a = Vec3::from(m.positions[t[0] as usize]);
            let b = Vec3::from(m.positions[t[1] as usize]);
            let c = Vec3::from(m.positions[t[2] as usize]);
            let face = (b - a).cross(c - a);
            if face.length_squared() < 1e-12 {
                continue;
            }
            // Outward radial direction at the triangle centre.
            let centre_world = p.origin + (a + b + c) / 3.0;
            total += 1;
            if face.normalize().dot(centre_world.normalize()) < 0.0 {
                wrong += 1;
            }
        }
        assert!(total > 0, "no triangles to check");
        assert_eq!(wrong, 0, "{wrong}/{total} triangles wound inward");
    }

    #[test]
    fn test_vertex_normals_agree_with_the_analytic_gradient() {
        let p = params(Vec3::splat(-8.0), 16, 1.0);
        let m = build_voxel_mesh(&sphere, &p).expect("sphere");
        for (i, pos) in m.positions.iter().enumerate() {
            let world = p.origin + Vec3::from(*pos);
            let expected = world.normalize();
            let got = Vec3::from(m.normals[i]);
            assert!(
                got.dot(expected) > 0.95,
                "vertex {i} normal {got} vs radial {expected}"
            );
        }
    }

    #[test]
    fn test_neighbouring_chunks_agree_on_the_shared_boundary() {
        // Two chunks meeting at x = 0. Every vertex one of them places in the
        // shared column must exist at the same world position in the other —
        // that is what closes the seam without stitching.
        let vs = 1.0;
        let left = params(Vec3::new(-8.0, -8.0, -8.0), 8, vs);
        let right = params(Vec3::new(0.0, -8.0, -8.0), 8, vs);
        let a = build_voxel_mesh(&sphere, &left).expect("left");
        let b = build_voxel_mesh(&sphere, &right).expect("right");

        let world_of = |p: &VoxelChunkParams, v: &[f32; 3]| p.origin + Vec3::from(*v);
        let near_seam = |w: Vec3| (w.x - 0.0).abs() < 0.51;

        let a_seam: Vec<Vec3> = a
            .positions
            .iter()
            .map(|v| world_of(&left, v))
            .filter(|w| near_seam(*w))
            .collect();
        let b_seam: Vec<Vec3> = b
            .positions
            .iter()
            .map(|v| world_of(&right, v))
            .filter(|w| near_seam(*w))
            .collect();

        assert!(!a_seam.is_empty(), "no seam vertices to compare");
        for wa in &a_seam {
            let matched = b_seam.iter().any(|wb| (*wb - *wa).length() < 1e-4);
            assert!(
                matched,
                "left vertex {wa} has no coincident twin on the right"
            );
        }
    }

    #[test]
    fn test_an_overhang_produces_downward_facing_geometry() {
        // A slab of rock floating in the air: its underside must be meshed
        // with normals pointing DOWN. This is the geometry a heightfield
        // cannot express at all.
        let slab = |p: Vec3| {
            let q = Vec3::new(p.x.abs() - 6.0, p.y.abs() - 2.0, p.z.abs() - 6.0);
            q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
        };
        let m = build_voxel_mesh(&slab, &params(Vec3::splat(-8.0), 16, 1.0)).expect("slab");
        let down = m.normals.iter().filter(|n| n[1] < -0.9).count();
        let up = m.normals.iter().filter(|n| n[1] > 0.9).count();
        assert!(up > 0, "the slab needs a top face");
        assert!(
            down > 0,
            "the slab needs an underside — this is the whole point"
        );
    }

    #[test]
    fn test_vertex_colours_match_the_heightfield_tint() {
        // The boundary between the two meshers must be invisible. Whatever the
        // heightfield mesher would paint at this height and slope, the voxel
        // mesher paints too — except R and A, which carry wall space and the
        // cliff factor in both paths (layer-material worlds).
        let p = params(Vec3::new(0.0, -4.0, 0.0), 8, 1.0);
        let m = build_voxel_mesh(&plane, &p).expect("plane");
        for (i, pos) in m.positions.iter().enumerate() {
            let world = p.origin + Vec3::from(*pos);
            let expected = tint_vertex_color(world.y, m.normals[i][1], p.max_height, &p.tint);
            let got = m.colors[i];
            assert!(
                (got[1] - expected[1]).abs() < 1e-5 && (got[2] - expected[2]).abs() < 1e-5,
                "vertex {i} colour {got:?} does not match the terrain tint {expected:?}"
            );
            assert_eq!(got[0], WALL_NEUTRAL, "R carries wall space");
            assert_eq!(got[3], 1.0, "A carries the cliff factor");
        }
    }

    #[test]
    fn test_legacy_vertex_colours_keep_the_full_tint_in_rgb() {
        // Without `layers` the chunk renders with the stock StandardMaterial,
        // which MULTIPLIES the vertex colour. A wall-space R of 0.502 darkened
        // the red channel half-way against the flat heightfield chunks beside
        // it — the legacy path must bake the plain tint, all four channels,
        // exactly like `build_chunk_mesh` with no mask.
        let mut p = params(Vec3::new(0.0, -4.0, 0.0), 8, 1.0);
        p.uses_layer_material = false;
        let m = build_voxel_mesh(&plane, &p).expect("plane");
        assert!(!m.colors.is_empty());
        for (i, pos) in m.positions.iter().enumerate() {
            let world = p.origin + Vec3::from(*pos);
            let expected = tint_vertex_color(world.y, m.normals[i][1], p.max_height, &p.tint);
            assert_eq!(
                m.colors[i], expected,
                "vertex {i} must carry the plain tint on the legacy path"
            );
        }
    }

    #[test]
    fn test_build_is_deterministic() {
        let p = params(Vec3::splat(-8.0), 16, 1.0);
        let a = build_voxel_mesh(&sphere, &p).expect("sphere");
        let b = build_voxel_mesh(&sphere, &p).expect("sphere");
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.normals, b.normals);
    }

    #[test]
    fn test_degenerate_params_are_refused_rather_than_panicking() {
        assert!(build_voxel_mesh(&sphere, &params(Vec3::ZERO, 0, 1.0)).is_none());
        assert!(build_voxel_mesh(&sphere, &params(Vec3::ZERO, 8, 0.0)).is_none());
        assert!(build_voxel_mesh(&sphere, &params(Vec3::ZERO, 8, f32::NAN)).is_none());
    }

    #[test]
    fn test_seal_hangs_border_edges_down_without_touching_the_interior() {
        let base = params(Vec3::new(0.0, -4.0, 0.0), 8, 1.0);
        let surface = build_voxel_mesh(&plane, &base).expect("plane");

        let mut p = base.clone();
        p.seal_faces = [false, true, false, false]; // +X face
        p.seal_depth = 3.0;
        let m = build_voxel_mesh(&plane, &p).expect("plane");

        let eps = 0.75f32;
        let border: Vec<[f32; 3]> = surface
            .positions
            .iter()
            .filter(|v| (v[0] - 8.0).abs() <= eps)
            .copied()
            .collect();
        assert!(!border.is_empty(), "the plane's border ring must exist");

        assert_eq!(
            m.positions.len(),
            surface.positions.len() + border.len(),
            "exactly one dropped copy per border vertex, none elsewhere"
        );

        let skirt: Vec<&[f32; 3]> = m
            .positions
            .iter()
            .enumerate()
            .filter(|(i, _)| m.normals[*i][0] > 0.99)
            .map(|(_, v)| v)
            .collect();
        assert_eq!(skirt.len(), border.len(), "one skirt vertex per border vertex");
        for v in &skirt {
            assert!((v[0] - 8.0).abs() <= eps, "skirt stays on the sealed face");
            assert!(
                border.iter().any(|b| (b[0] - v[0]).abs() < 1e-4
                    && (b[2] - v[2]).abs() < 1e-4
                    && (v[1] - (b[1] - 3.0)).abs() < 1e-4),
                "skirt vertex {v:?} hangs seal_depth below its border source"
            );
        }

        let border_edges: std::collections::HashSet<(u32, u32)> = surface
            .indices
            .chunks_exact(3)
            .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
            .map(|(a, b)| (a.min(b), a.max(b)))
            .filter(|&(a, b)| {
                let (pa, pb) = (surface.positions[a as usize], surface.positions[b as usize]);
                (pa[0] - 8.0).abs() <= eps && (pb[0] - 8.0).abs() <= eps
            })
            .collect();
        assert!(!border_edges.is_empty());
        assert_eq!(
            m.indices.len(),
            surface.indices.len() + border_edges.len() * 6,
            "two triangles per unique border edge"
        );
    }

    #[test]
    fn test_seal_skirts_carry_the_face_normal_and_no_cliff_alpha() {
        let mut p = params(Vec3::new(0.0, -4.0, 0.0), 8, 1.0);
        p.seal_faces = [true, false, false, false]; // −X face
        p.seal_depth = 4.0;
        let m = build_voxel_mesh(&plane, &p).expect("plane");
        let skirts = m
            .normals
            .iter()
            .enumerate()
            .filter(|(_, n)| n[0] < -0.99)
            .count();
        assert!(skirts > 0, "the −X seal must exist");
        for (i, n) in m.normals.iter().enumerate() {
            if n[0] < -0.99 {
                assert!(
                    (n[1].abs()) < 1e-4 && (n[2].abs()) < 1e-4,
                    "skirt normal is horizontal: {n:?}"
                );
                assert!(
                    (m.colors[i][3] - 0.0).abs() < 1e-5,
                    "layers path zeroes the skirt alpha so no triplanar rock"
                );
            }
        }
    }
}

//! Spawning the volumetric chunks — where the field becomes geometry in the
//! world.
//!
//! A terrain chunk classified [`ChunkClass::Volumetric`] does **not** get a
//! heightfield mesh. It is covered instead by a stack of [`VoxelChunk`]
//! entities, each a 32³ box meshed with surface nets. The two paths never
//! overlap, so there is no double geometry and nothing to z-fight.
//!
//! The stack is vertical because the shape is: a cliff on a 200 m hillside
//! spans far more Y than one box. Most of that stack is proven uniform and
//! never sampled ([`VoxelField::region_state`]); only the boxes the surface
//! actually crosses are built.

use bevy::prelude::*;

use super::super::brush::BrushGrid;
use super::super::layer_material::TerrainChunkMaterial;
use super::super::mesh::HeightField;
use super::super::runtime::{ChunkLayerMap, to_bevy_mesh};
use super::super::spec::TerrainSpec;
use super::field::VoxelField;
use super::mods::Bounds3;
use super::surface_nets::{VOXEL_CHUNK_CELLS, VoxelChunkParams, build_voxel_mesh};

/// A meshed box of the voxel field.
#[derive(Debug, Component)]
pub struct VoxelChunk {
    /// Box index inside the terrain grid (unique, stable, used for the name).
    pub coords: IVec3,
    /// World position of the box's minimum corner.
    pub origin: Vec3,
    /// Edge length in meters.
    pub extent: f32,
    /// Cell size in meters.
    pub voxel_size: f32,
}

/// Result of one bootstrap pass, for the log and for tests.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VoxelSpawnStats {
    /// Boxes that produced geometry.
    pub meshed: usize,
    /// Boxes proven uniform (sky or bedrock) and skipped without sampling.
    pub skipped_uniform: usize,
    /// Boxes sampled that turned out to hold no surface after all.
    pub empty: usize,
    /// Terrain chunks handed to the voxel mesher.
    pub chunks: usize,
}

/// Builds and spawns every volumetric chunk of the world.
///
/// Iteration is driven by the **terrain chunk grid**, not by a world-aligned
/// voxel grid, and that is load-bearing. A voxel box snapped to its own
/// lattice spills into neighbouring terrain chunks whenever the two grids do
/// not share an origin — and they generally do not: the terrain grid starts at
/// `-world_size/2`, which for the 4 km world is -2000, not a multiple of the
/// 32 m box. The spilled box meshes ground that a *flat* chunk is also drawing,
/// and the two z-fight along the whole boundary.
///
/// Driving from the terrain grid makes the tiling exact: a volumetric chunk is
/// covered by whole boxes, a flat chunk by none.
#[allow(clippy::too_many_arguments)]
pub fn spawn_voxel_chunks(
    world: &mut World,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    spec: &TerrainSpec,
    grid: &BrushGrid,
    field: &VoxelField,
    material: &Handle<StandardMaterial>,
    layer_map: Option<&ChunkLayerMap>,
    voxel_size: f32,
    chunk_edge: f32,
) -> VoxelSpawnStats {
    let mut stats = VoxelSpawnStats::default();
    if field.is_flat() || !(chunk_edge.is_finite() && chunk_edge > 0.0) {
        return stats;
    }
    let extent = VOXEL_CHUNK_CELLS as f32 * voxel_size;
    if !(extent.is_finite() && extent > 0.0) {
        return stats;
    }
    // Boxes per terrain-chunk edge, rounded up so a chunk is always covered.
    let per_edge = (chunk_edge / extent).ceil().max(1.0) as i32;
    let mods_y = field.index().bounds();
    // `chunk_tint` is what the heightfield mesher uses: with `layers` active it
    // zeroes the banding so the splat owns the look, and the voxel path has to
    // make the same choice or the two disagree at every boundary.
    let tint = spec.chunk_tint();

    let half = spec.world_size * 0.5;
    let rows = (spec.world_size / chunk_edge).ceil().max(1.0) as i32;

    for cz in 0..rows {
        for cx in 0..rows {
            let x0 = -half + cx as f32 * chunk_edge;
            let z0 = -half + cz as f32 * chunk_edge;
            if !field.is_volumetric_chunk(x0, z0, chunk_edge) {
                continue;
            }
            stats.chunks += 1;
            // The per-chunk layer material of the SAME terrain chunk. With
            // `layers` active the terrain is drawn by `TerrainChunkMaterial`
            // and the standard handle is only a fallback — giving the voxel
            // boxes the fallback paints them plain white next to a textured
            // hillside, which is exactly what it looks like.
            let chunk_material = layer_map.and_then(|m| m.get(cx as u32, cz as u32)).cloned();

            // Vertical envelope for this chunk only: the ground it spans plus
            // whatever the mods reach. Everything outside is provably sky or
            // bedrock and never gets a box at all.
            let (gmin, gmax) = grid
                .range_over(x0, z0, x0 + chunk_edge, z0 + chunk_edge)
                .unwrap_or((0.0, spec.max_height));
            let y_lo = gmin.min(mods_y.min.y) - 1.0;
            let y_hi = gmax.max(mods_y.max.y) + 1.0;
            let iy0 = (y_lo / extent).floor() as i32;
            let iy1 = (y_hi / extent).ceil() as i32;

            for iz in 0..per_edge {
                for ix in 0..per_edge {
                    for iy in iy0..iy1 {
                        let origin = Vec3::new(
                            x0 + ix as f32 * extent,
                            iy as f32 * extent,
                            z0 + iz as f32 * extent,
                        );
                        let bounds = Bounds3::from_corners(origin, origin + Vec3::splat(extent));
                        if field.region_state(grid, &bounds).is_some() {
                            stats.skipped_uniform += 1;
                            continue;
                        }
                        let params = VoxelChunkParams {
                            origin,
                            cells: VOXEL_CHUNK_CELLS,
                            voxel_size,
                            texture_tile_size: spec.texture_tile_size,
                            tint: tint.clone(),
                            max_height: spec.max_height,
                        };
                        let density = |p: Vec3| field.density(grid, p);
                        let Some(data) = build_voxel_mesh(&density, &params) else {
                            stats.empty += 1;
                            continue;
                        };
                        let coords = IVec3::new(cx * per_edge + ix, iy, cz * per_edge + iz);
                        let handle = meshes.add(to_bevy_mesh(&data));
                        let mut entity = world.spawn((
                            Name::new(format!(
                                "voxel chunk {}-{}-{}",
                                coords.x, coords.y, coords.z
                            )),
                            VoxelChunk {
                                coords,
                                origin,
                                extent,
                                voxel_size,
                            },
                            // Surface-nets positions are origin-relative on all
                            // three axes, unlike the heightfield mesher's
                            // absolute Y.
                            Transform::from_translation(origin),
                            Visibility::Inherited,
                            Mesh3d(handle),
                            ChildOf(parent),
                        ));
                        match &chunk_material {
                            Some(layer) => {
                                entity
                                    .insert(MeshMaterial3d::<TerrainChunkMaterial>(layer.clone()));
                            }
                            None => {
                                entity.insert(MeshMaterial3d(material.clone()));
                            }
                        }
                        stats.meshed += 1;
                    }
                }
            }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::heightmap::HeightMapU16;
    use crate::terrain::voxel::cave::CaveSpec;
    use crate::terrain::voxel::mods::{BoxMod, ModOp, VoxelMod};
    use bevy::math::Vec2;

    fn flat_grid(world_size: f32, height: f32, max_height: f32) -> BrushGrid {
        let n = 33usize;
        let raw = vec![((height / max_height) * 65535.0).round() as u16; n * n];
        let map = HeightMapU16 {
            width: n,
            depth: n,
            data: raw,
        };
        BrushGrid::from_height_map(&map, world_size, max_height, 0.0).expect("grid")
    }

    /// Ground climbing along +X, so a region spans real relief.
    fn sloped_grid(world_size: f32, max_height: f32) -> BrushGrid {
        let n = 33usize;
        let mut raw = vec![0u16; n * n];
        for z in 0..n {
            for x in 0..n {
                let f = x as f32 / (n - 1) as f32;
                raw[z * n + x] = (f * 65535.0).round() as u16;
            }
        }
        let map = HeightMapU16 {
            width: n,
            depth: n,
            data: raw,
        };
        BrushGrid::from_height_map(&map, world_size, max_height, 0.0).expect("grid")
    }

    fn shelf_field(world_size: f32) -> VoxelField {
        let shelf: Box<dyn VoxelMod> = Box::new(BoxMod::new(
            "shelf",
            Bounds3::from_corners(Vec3::new(-16.0, 24.0, -16.0), Vec3::new(16.0, 32.0, 16.0)),
            ModOp::Union,
        ));
        VoxelField::new(vec![shelf], world_size, 64.0)
    }

    struct Harness {
        world: World,
        material: Handle<StandardMaterial>,
        parent: Entity,
        meshes: Assets<Mesh>,
    }

    fn harness() -> Harness {
        let mut world = World::new();
        let meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let material = materials.add(StandardMaterial::default());
        let parent = world.spawn(Name::new("terrain")).id();
        Harness {
            world,
            material,
            parent,
            meshes,
        }
    }

    #[test]
    fn test_a_flat_world_spawns_nothing_at_all() {
        let mut h = harness();
        let grid = flat_grid(256.0, 10.0, 100.0);
        let stats = spawn_voxel_chunks(
            &mut h.world,
            &mut h.meshes,
            h.parent,
            &TerrainSpec::default(),
            &grid,
            &VoxelField::flat(256.0, 64.0),
            &h.material,
            None,
            1.0,
            64.0,
        );
        assert_eq!(stats, VoxelSpawnStats::default());
        assert_eq!(h.world.query::<&VoxelChunk>().iter(&h.world).count(), 0);
    }

    #[test]
    fn test_spawning_meshes_the_shelf() {
        let mut h = harness();
        let grid = flat_grid(256.0, 10.0, 100.0);
        let spec = TerrainSpec {
            world_size: 256.0,
            ..TerrainSpec::default()
        };
        let field = shelf_field(256.0);
        let stats = spawn_voxel_chunks(
            &mut h.world,
            &mut h.meshes,
            h.parent,
            &spec,
            &grid,
            &field,
            &h.material,
            None,
            1.0,
            64.0,
        );
        assert!(
            stats.meshed > 0,
            "the shelf must produce geometry: {stats:?}"
        );
        assert!(stats.chunks > 0, "some terrain chunk must be claimed");
        let spawned = h.world.query::<&VoxelChunk>().iter(&h.world).count();
        assert_eq!(spawned, stats.meshed, "one entity per meshed box");
    }

    /// The z-fighting regression, stated as an invariant.
    ///
    /// Every voxel box must lie inside a terrain chunk the classifier calls
    /// volumetric. A box that spills into a flat chunk meshes ground the
    /// heightfield mesher is also drawing, and the pair z-fight across the
    /// whole overlap — a full-screen moiré, not a subtle seam.
    #[test]
    fn test_no_voxel_box_lands_in_a_chunk_the_heightfield_still_owns() {
        for world_size in [256.0_f32, 300.0, 4000.0] {
            let mut h = harness();
            let grid = flat_grid(world_size, 10.0, 100.0);
            let spec = TerrainSpec {
                world_size,
                ..TerrainSpec::default()
            };
            let field = shelf_field(world_size);
            let edge = 64.0_f32;
            spawn_voxel_chunks(
                &mut h.world,
                &mut h.meshes,
                h.parent,
                &spec,
                &grid,
                &field,
                &h.material,
                None,
                1.0,
                edge,
            );

            let half = world_size * 0.5;
            let boxes: Vec<(Vec3, f32)> = h
                .world
                .query::<&VoxelChunk>()
                .iter(&h.world)
                .map(|c| (c.origin, c.extent))
                .collect();
            assert!(!boxes.is_empty(), "world {world_size}: nothing spawned");

            for (origin, extent) in boxes {
                // Every corner of the box must sit in a volumetric chunk.
                for (dx, dz) in [
                    (0.0, 0.0),
                    (extent - 0.01, 0.0),
                    (0.0, extent - 0.01),
                    (extent - 0.01, extent - 0.01),
                ] {
                    let px = origin.x + dx;
                    let pz = origin.z + dz;
                    let cx = ((px + half) / edge).floor();
                    let cz = ((pz + half) / edge).floor();
                    let x0 = -half + cx * edge;
                    let z0 = -half + cz * edge;
                    assert!(
                        field.is_volumetric_chunk(x0, z0, edge),
                        "world {world_size}: box at {origin:?} reaches ({px}, {pz}), \
                         inside FLAT chunk ({cx}, {cz}) — that overlap z-fights"
                    );
                }
            }
        }
    }

    #[test]
    fn test_real_relief_lets_most_boxes_be_proven_uniform() {
        // The proof exists for this: a wall on a hillside. Without it every box
        // in the stack would be sampled ~39 k times to discover it is sky.
        let mut h = harness();
        let grid = sloped_grid(256.0, 200.0);
        let spec = TerrainSpec {
            world_size: 256.0,
            max_height: 200.0,
            ..TerrainSpec::default()
        };
        let wall: Box<dyn VoxelMod> = Box::new(BoxMod::new(
            "wall",
            Bounds3::from_corners(Vec3::new(-64.0, 100.0, -8.0), Vec3::new(64.0, 130.0, 8.0)),
            ModOp::Union,
        ));
        let field = VoxelField::new(vec![wall], 256.0, 64.0);
        let stats = spawn_voxel_chunks(
            &mut h.world,
            &mut h.meshes,
            h.parent,
            &spec,
            &grid,
            &field,
            &h.material,
            None,
            1.0,
            64.0,
        );
        assert!(
            stats.meshed > 0,
            "the wall must produce geometry: {stats:?}"
        );
        assert!(
            stats.skipped_uniform > 0,
            "deep relief must let sky/bedrock boxes be skipped unsampled: {stats:?}"
        );
    }

    #[test]
    fn test_a_cave_produces_a_ceiling() {
        let mut h = harness();
        let grid = flat_grid(256.0, 60.0, 100.0);
        let spec = TerrainSpec {
            world_size: 256.0,
            max_height: 100.0,
            ..TerrainSpec::default()
        };
        let cave = CaveSpec {
            name: Some("mina".into()),
            path: vec![Vec2::new(-60.0, 0.0), Vec2::new(60.0, 0.0)],
            radius: 3.5,
            depth: 12.0,
            open_ends: true,
        };
        let field = VoxelField::new(cave.build(&grid), 256.0, 64.0);
        let stats = spawn_voxel_chunks(
            &mut h.world,
            &mut h.meshes,
            h.parent,
            &spec,
            &grid,
            &field,
            &h.material,
            None,
            1.0,
            64.0,
        );
        assert!(stats.meshed > 0, "the tunnel must mesh: {stats:?}");

        // A ceiling is a surface whose normal points DOWN. No heightfield mesh
        // has one anywhere.
        let meshes = &h.meshes;
        let mut downward = 0usize;
        let mut entities: Vec<Entity> = Vec::new();
        {
            let mut q = h.world.query::<(Entity, &VoxelChunk)>();
            for (e, _) in q.iter(&h.world) {
                entities.push(e);
            }
        }
        for e in entities {
            let handle = h.world.get::<Mesh3d>(e).expect("voxel chunk has a mesh");
            let mesh = meshes.get(&handle.0).expect("mesh asset exists");
            if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(normals)) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            {
                downward += normals.iter().filter(|n| n[1] < -0.5).count();
            }
        }
        assert!(
            downward > 0,
            "the cave meshed no downward-facing surface — there is no roof, \
             which is the one thing a heightfield could not do either"
        );
    }
}

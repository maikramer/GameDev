//! Spawning the volumetric columns — where the field becomes geometry.
//!
//! No caminho 100% volumétrico CADA célula da grelha de chunks é uma COLUNA
//! voxel: a entidade `TerrainChunk` (nome `chunk cz-cx`) é o pai de uma
//! pilha de caixas [`VoxelChunk`] meshadas com surface nets. O ladder de
//! LOD vive no `plugin.rs` (select + histerese + budget + cull); o que este
//! módulo fornece é a geometria pura — [`lod_shape`] (tiled de caixas por
//! LOD), [`column_boxes`] (quais caixas existem) e [`build_box_mesh`] (uma
//! caixa → buffers).

use bevy::prelude::*;

use super::super::brush::BrushGrid;
use super::super::mesh::HeightField;
use super::super::runtime::{ChunkLayerMap, ChunkMaterialHandle, to_bevy_mesh};
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

// ── Colunas voxel com ladder de LOD ─────────────────────────────────────────
//
// O caminho 100% volumétrico trata CADA célula da grelha de chunks como uma
// COLUNA voxel: o `TerrainChunk` deixa de ter mesh próprio e passa a ser o
// pai de uma pilha de caixas surface-nets. O ladder reutiliza a semântica do
// heightfield (select + histerese + budget + cull no `plugin.rs`); o que
// muda é o que um LOD constrói:

/// Geometria de UM LOD de uma coluna: `per_edge` caixas por borda de chunk,
/// cada uma com `cells` células por eixo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelLodShape {
    pub cells: usize,
    pub per_edge: i32,
}

/// Escolhe o tiling de caixas de uma coluna ao `lod`.
///
/// O ladder duplica a célula a cada nível — LOD 0 renderiza ao passo LOD-0
/// (`resolution`), comprando a mesma redução 4×/16× de triângulos do ladder
/// heightfield. `cells × célula × per_edge` tem de ladrilhar a borda do
/// chunk: procura-se um tiling exato a 32/16/8 células e, não havendo, UMA
/// caixa cobrindo a borda inteira (o mesher aceita qualquer tamanho de
/// célula; os LODs grossos ficam baratos pelo `cells`, não por caixas
/// parciais). Determinístico e global: dois vizinhos ao mesmo LOD derivam
/// SEMPRE o mesmo shape — é isso que mantém as costuras deles fechadas por
/// coincidência de vértices.
pub fn lod_shape(lod0_cell: f32, chunk_edge: f32, lod: u8) -> VoxelLodShape {
    if !(lod0_cell.is_finite() && lod0_cell > 0.0) || !(chunk_edge.is_finite() && chunk_edge > 0.0)
    {
        return VoxelLodShape {
            cells: VOXEL_CHUNK_CELLS,
            per_edge: 1,
        };
    }
    let target = lod0_cell * (1u32 << lod) as f32;
    for cells in [32usize, 16, 8] {
        let wanted = cells as f32 * target;
        let per_edge = (chunk_edge / wanted).ceil().max(1.0) as i32;
        let extent = chunk_edge / per_edge as f32;
        let actual = extent / cells as f32;
        if ((actual - target) / target).abs() <= 0.05 {
            return VoxelLodShape { cells, per_edge };
        }
    }
    let cells = ((chunk_edge / target).round() as usize).clamp(4, VOXEL_CHUNK_CELLS);
    VoxelLodShape { cells, per_edge: 1 }
}

/// Uma caixa surface-nets de uma coluna, pronta para meshar.
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelBoxSpec {
    /// Coordenada da caixa na grelha lógica da coluna (nome/identidade).
    pub pos: IVec3,
    /// Posição mundial do canto mínimo.
    pub origin: Vec3,
    /// Aresta em metros (cúbica; as caixas ladrilham a coluna exatamente).
    pub extent: f32,
    /// Células por eixo.
    pub cells: usize,
    /// Aresta de uma célula em metros.
    pub voxel_size: f32,
    /// Faces na fronteira da coluna a selar, `[-X, +X, -Z, +Z]`.
    pub seal: [bool; 4],
}

/// Profundidade do selo de fronteira de coluna: a mesma conta do
/// `runtime::volumetric_seal` — quatro células cobrem o pior desacordo do
/// surface nets com o vizinho, piso de 2 m.
pub fn seal_depth(voxel_size: f32) -> f32 {
    (voxel_size.max(0.0) * 4.0).max(2.0)
}

/// As caixas que renderizam UMA coluna ao `lod` dado.
///
/// A pilha vertical é o envelope do chunk: o intervalo de alturas da grelha
/// (`range_over`, O(1)) mais o alcance dos mods — o resto é céu ou bedrock
/// provados (`region_state`) e nunca chega a ser amostrado.
pub fn column_boxes(
    spec: &TerrainSpec,
    grid: &BrushGrid,
    field: &VoxelField,
    chunk_edge: f32,
    lod0_cell: f32,
    lod: u8,
    coords: UVec2,
) -> Vec<VoxelBoxSpec> {
    let shape = lod_shape(lod0_cell, chunk_edge, lod);
    let extent = chunk_edge / shape.per_edge as f32;
    let voxel_size = extent / shape.cells as f32;
    let half = spec.world_size * 0.5;
    let x0 = -half + coords.x as f32 * chunk_edge;
    let z0 = -half + coords.y as f32 * chunk_edge;
    let mods_y = field.index().bounds();
    let (gmin, gmax) = grid
        .range_over(x0, z0, x0 + chunk_edge, z0 + chunk_edge)
        .unwrap_or((0.0, spec.max_height));
    let y_lo = gmin.min(mods_y.min.y) - 1.0;
    let y_hi = gmax.max(mods_y.max.y) + 1.0;
    let iy0 = (y_lo / extent).floor() as i32;
    let iy1 = (y_hi / extent).ceil() as i32;

    let mut boxes = Vec::new();
    for iz in 0..shape.per_edge {
        for ix in 0..shape.per_edge {
            for iy in iy0..iy1 {
                let origin = Vec3::new(
                    x0 + ix as f32 * extent,
                    iy as f32 * extent,
                    z0 + iz as f32 * extent,
                );
                let bounds = Bounds3::from_corners(origin, origin + Vec3::splat(extent));
                if field.region_state(grid, &bounds).is_some() {
                    continue;
                }
                boxes.push(VoxelBoxSpec {
                    pos: IVec3::new(coords.x as i32 * shape.per_edge + ix, iy, coords.y as i32 * shape.per_edge + iz),
                    origin,
                    extent,
                    cells: shape.cells,
                    voxel_size,
                    seal: [
                        ix == 0,
                        ix == shape.per_edge - 1,
                        iz == 0,
                        iz == shape.per_edge - 1,
                    ],
                });
            }
        }
    }
    boxes
}

/// Mesa UMA caixa da coluna (SDF → surface nets → buffers).
pub fn build_box_mesh(
    spec: &TerrainSpec,
    grid: &BrushGrid,
    field: &VoxelField,
    b: &VoxelBoxSpec,
) -> Option<super::super::mesh::ChunkMeshData> {
    let params = VoxelChunkParams {
        origin: b.origin,
        cells: b.cells,
        voxel_size: b.voxel_size,
        texture_tile_size: spec.texture_tile_size,
        tint: spec.chunk_tint(),
        max_height: spec.max_height,
        uses_layer_material: !spec.layers.is_empty(),
        seal_faces: b.seal,
        seal_depth: seal_depth(b.voxel_size),
    };
    let density = |p: Vec3| field.density(grid, p);
    build_voxel_mesh(&density, &params)
}

/// Spawna a entidade de UMA caixa voxel (filha da coluna). `visible = false`
/// para caixas em construção (staged) — entram visíveis só no swap da
/// coluna completa, para nunca desenhar metade de um LOD ao lado do outro.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_box_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    b: &VoxelBoxSpec,
    data: super::super::mesh::ChunkMeshData,
    material: &ChunkMaterialHandle,
    visible: bool,
) -> Entity {
    let handle = meshes.add(to_bevy_mesh(&data));
    let mut entity = commands.spawn((
        Name::new(format!(
            "voxel chunk {}-{}-{}",
            b.pos.x, b.pos.y, b.pos.z
        )),
        VoxelChunk {
            coords: b.pos,
            origin: b.origin,
            extent: b.extent,
            voxel_size: b.voxel_size,
        },
        // Surface-nets positions are origin-relative on all three axes.
        Transform::from_translation(b.origin),
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
        Mesh3d(handle),
        ChildOf(parent),
    ));
    match material {
        ChunkMaterialHandle::Layer(material) => {
            entity.insert(MeshMaterial3d(material.clone()));
        }
        ChunkMaterialHandle::Standard(material) => {
            entity.insert(MeshMaterial3d(material.clone()));
        }
    }
    entity.id()
}

/// Spawna uma COLUNA inteira: a entidade `TerrainChunk` (o nome `chunk
/// cz-cx` é o contrato de adoção/profiler) mais as caixas de `boxes` já
/// visíveis. Devolve `(entidade, caixas meshadas)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_column(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coords: UVec2,
    lod: u8,
    material: &ChunkMaterialHandle,
    boxes: &[VoxelBoxSpec],
    grid: &BrushGrid,
    field: &VoxelField,
    spec: &TerrainSpec,
) -> (Entity, usize) {
    let column = commands
        .spawn((
            Name::new(super::super::plugin::chunk_name(coords)),
            Transform::default(),
            Visibility::Inherited,
            ChildOf(parent),
            super::super::plugin::TerrainChunk {
                coords,
                lod,
                built_lod: lod,
            },
        ))
        .id();
    let mut meshed = 0;
    for b in boxes {
        if let Some(data) = build_box_mesh(spec, grid, field, b) {
            spawn_box_entity(commands, meshes, column, b, data, material, true);
            meshed += 1;
        }
    }
    (column, meshed)
}

/// Bootstrap do caminho 100% voxel: spawna as colunas dentro do raio de
/// render ao LOD que a distância da câmara pede (o plugin cuida do resto —
/// respawn à aproximação e refinamento com histerese). Sem câmara (testes
/// headless) tudo nasce em LOD 0, como no heightfield.
#[allow(clippy::too_many_arguments)]
pub fn spawn_voxel_columns(
    world: &mut World,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    spec: &TerrainSpec,
    grid: &BrushGrid,
    field: &VoxelField,
    camera_xz: Option<Vec2>,
    standard: Handle<StandardMaterial>,
    layer_map: Option<&ChunkLayerMap>,
) -> VoxelSpawnStats {
    let mut stats = VoxelSpawnStats::default();
    let edge = super::super::plugin::chunk_edge(spec);
    if !(edge.is_finite() && edge > 0.0) {
        return stats;
    }
    let lod0_cell = super::super::plugin::lod0_step(spec) as f32;
    let max_lod = super::super::plugin::max_lod_for(spec, edge);
    let rows = (spec.world_size / edge).ceil().max(1.0) as u32;
    let half = spec.world_size * 0.5;

    let mut commands = world.commands();
    for cz in 0..rows {
        for cx in 0..rows {
            let center = Vec2::new(
                -half + cx as f32 * edge + edge * 0.5,
                -half + cz as f32 * edge + edge * 0.5,
            );
            let distance = camera_xz.map(|cam| cam.distance(center));
            if let Some(distance) = distance
                && distance > spec.effective_render_distance()
            {
                continue;
            }
            let lod = distance
                .map(|d| super::super::plugin::raw_lod(d, spec.lod_distance(), max_lod))
                .unwrap_or(0);
            let coords = UVec2::new(cx, cz);
            let boxes = column_boxes(spec, grid, field, edge, lod0_cell, lod, coords);
            stats.chunks += 1;
            let material = layer_map
                .and_then(|m| m.get(cx, cz).cloned())
                .map(ChunkMaterialHandle::Layer)
                .unwrap_or_else(|| ChunkMaterialHandle::Standard(standard.clone()));
            let (_, built) = spawn_column(
                &mut commands,
                meshes,
                parent,
                coords,
                lod,
                &material,
                &boxes,
                grid,
                field,
                spec,
            );
            stats.meshed += built;
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

    #[test]
    fn test_lod_shape_ladder_doubles_the_cell() {
        // Chunk 64 m, célula LOD-0 de 1 m: LOD0 = 2×2 caixas de 32³ @1 m,
        // LOD1 = 1 caixa @2 m, LOD2 = 16³ @4 m (32 células a 4 m não
        // ladrilham 64 m — a 16 sim).
        assert_eq!(lod_shape(1.0, 64.0, 0), VoxelLodShape { cells: 32, per_edge: 2 });
        assert_eq!(lod_shape(1.0, 64.0, 1), VoxelLodShape { cells: 32, per_edge: 1 });
        assert_eq!(lod_shape(1.0, 64.0, 2), VoxelLodShape { cells: 16, per_edge: 1 });
        // Edge 16 (spec pequena dos testes): tudo numa caixa por coluna.
        assert_eq!(lod_shape(1.0, 16.0, 0), VoxelLodShape { cells: 16, per_edge: 1 });
        assert_eq!(lod_shape(1.0, 16.0, 2), VoxelLodShape { cells: 4, per_edge: 1 });
    }

    /// O invariante que evitava o moiré: as caixas ladrilham a COLUNA
    /// exatamente — dentro da célula da grelha e sobre o lattice derivado da
    /// borda. Com tudo voxel, o mesmo contrato garante que colunas vizinhas
    /// ao mesmo LOD partilham vértices por coincidência.
    #[test]
    fn test_boxes_tile_the_column_exactly() {
        for world_size in [256.0_f32, 300.0, 4000.0] {
            let grid = flat_grid(world_size, 10.0, 100.0);
            let spec = TerrainSpec {
                world_size,
                ..TerrainSpec::default()
            };
            let field = shelf_field(world_size);
            let edge = 64.0_f32;
            let coords = UVec2::new(2, 1);
            let boxes = column_boxes(&spec, &grid, &field, edge, 1.0, 0, coords);
            assert!(!boxes.is_empty(), "world {world_size}: nothing planned");

            let half = world_size * 0.5;
            let x0 = -half + coords.x as f32 * edge;
            let z0 = -half + coords.y as f32 * edge;
            for b in &boxes {
                assert!(
                    b.origin.x >= x0 - 0.01
                        && b.origin.x + b.extent <= x0 + edge + 0.01
                        && b.origin.z >= z0 - 0.01
                        && b.origin.z + b.extent <= z0 + edge + 0.01,
                    "box {b:?} overruns its column"
                );
                assert!(
                    ((b.origin.x - x0) / b.extent).fract().abs() < 1e-4,
                    "box not on the column lattice: {b:?}"
                );
            }
        }
    }

    /// Mundo sem mods: as caixas de coluna continuam a cobrir a superfície
    /// (o heightfield é só o termo-base agora) e o mesh sai ao nível certo.
    #[test]
    fn test_a_flat_world_boxes_cover_the_surface() {
        let grid = flat_grid(256.0, 10.0, 100.0);
        let spec = TerrainSpec::default();
        let field = VoxelField::flat(256.0, 64.0);
        let boxes = column_boxes(&spec, &grid, &field, 64.0, 1.0, 0, UVec2::new(1, 1));
        assert!(!boxes.is_empty(), "flat ground must still be meshed");
        let mut meshed = 0;
        for b in &boxes {
            if let Some(data) = build_box_mesh(&spec, &grid, &field, b) {
                meshed += 1;
                for (i, pos) in data.positions.iter().enumerate() {
                    // Os vértices de selo pendem seal_depth abaixo da
                    // superfície — só a pele para cima tem de acertar o nível.
                    if data.normals[i][1] < 0.5 {
                        continue;
                    }
                    assert!(
                        (10.0 - (b.origin.y + pos[1])).abs() < 1.01,
                        "vertex off the flat surface: {pos:?}"
                    );
                }
            }
        }
        assert!(meshed > 0, "at least one box meshes the surface");
    }

    /// A prova `region_state` continua a pagar o aluguer: relevo profundo
    /// deixa a maioria das caixas do envelope por amostrar.
    #[test]
    fn test_real_relief_lets_most_boxes_be_proven_uniform() {
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
        let boxes = column_boxes(&spec, &grid, &field, 64.0, 1.0, 0, UVec2::new(1, 1));
        assert!(!boxes.is_empty(), "the wall column must plan boxes");
        // Sem a prova seriam 2×2×(200/32) ≈ 26 caixas por coluna.
        assert!(
            boxes.len() < 12,
            "{} boxes planned — region_state is not pruning",
            boxes.len()
        );
    }

    #[test]
    fn test_a_cave_produces_a_ceiling() {
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
        let boxes = column_boxes(&spec, &grid, &field, 64.0, 1.0, 0, UVec2::new(1, 1));
        let mut downward = 0usize;
        for b in &boxes {
            if let Some(data) = build_box_mesh(&spec, &grid, &field, b) {
                downward += data
                    .normals
                    .iter()
                    .filter(|n| n[1] < -0.5)
                    .count();
            }
        }
        assert!(
            downward > 0,
            "the cave meshed no downward-facing surface — there is no roof"
        );
    }

    /// As caixas na fronteira da coluna levam o selo da face externa; as
    /// interiores não (as costuras internas fecham por coincidência).
    #[test]
    fn test_seal_faces_mark_only_column_boundary_boxes() {
        let grid = flat_grid(256.0, 10.0, 100.0);
        let spec = TerrainSpec::default();
        let field = shelf_field(256.0);
        let boxes = column_boxes(&spec, &grid, &field, 64.0, 1.0, 0, UVec2::new(1, 1));
        assert!(!boxes.is_empty());
        let half = 128.0_f32;
        let x0 = -half + 64.0; // coluna (1,1)
        let z0 = -half + 64.0;
        let per_edge = 2; // LOD0: 2×2 caixas de 32 m
        for b in &boxes {
            let lx = ((b.origin.x - x0) / b.extent).round() as i32;
            let lz = ((b.origin.z - z0) / b.extent).round() as i32;
            assert_eq!(b.seal[0], lx == 0, "−X seal só na fronteira: {b:?}");
            assert_eq!(b.seal[1], lx == per_edge - 1, "+X seal só na fronteira: {b:?}");
            assert_eq!(b.seal[2], lz == 0, "−Z seal só na fronteira: {b:?}");
            assert_eq!(b.seal[3], lz == per_edge - 1, "+Z seal só na fronteira: {b:?}");
        }
        // O canto mínimo leva −X e −Z; o oposto leva +X e +Z.
        assert!(boxes.iter().any(|b| b.seal[0] && b.seal[2]));
        assert!(boxes.iter().any(|b| b.seal[1] && b.seal[3]));
    }
}

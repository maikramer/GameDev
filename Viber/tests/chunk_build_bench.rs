//! Micro-benchmark: quanto custa meshar UMA caixa voxel por LOD?
//!
//! O ladder de colunas voxel reconstrói caixas inline no main thread com um
//! orçamento de `VOXEL_MAX_MESH_BUILDS_PER_FRAME` caixas/frame. Uma caixa 32³
//! ao passo de 1 m amostra ~39 k vezes o campo; este bench põe um número no
//! custo por LOD com a geometria exata que o simple-rpg usa, para o orçamento
//! ser argumentado com dados e não ao toque.
//!
//! Run with: `cargo test --release --test chunk_build_bench -- --nocapture`

use std::time::Instant;

use viber::terrain::brush::BrushGrid;
use viber::terrain::heightmap::HeightMapU16;
use viber::terrain::spec::TerrainSpec;
use viber::terrain::voxel::{VoxelField, build_box_mesh, column_boxes, lod_shape};

/// Grid de terreno com relevo suave (o custo no sampler é o mesmo do
/// runtime: Catmull-Rom monotone).
fn rolling_grid() -> (TerrainSpec, BrushGrid) {
    let spec = TerrainSpec {
        world_size: 256.0,
        max_height: 50.0,
        chunk_size: 64.0,
        resolution: 64,
        seed: 11,
        ..TerrainSpec::default()
    };
    let map = HeightMapU16::procedural(&spec, spec.resolution.max(1) as usize);
    let grid = BrushGrid::from_height_map(&map, spec.world_size, spec.max_height, 1.0)
        .expect("grid builds");
    (spec, grid)
}

#[test]
fn bench_voxel_box_build_per_lod() {
    let (spec, grid) = rolling_grid();
    let field = VoxelField::flat(spec.world_size, spec.chunk_size);
    let edge = spec.chunk_size;

    for lod in 0..3u8 {
        let shape = lod_shape(1.0, edge, lod);
        let coords = bevy::math::UVec2::new(2, 2);
        let boxes = column_boxes(&spec, &grid, &field, edge, 1.0, lod, coords);
        assert!(!boxes.is_empty(), "lod {lod}: no boxes planned");

        // Warm-up (allocator, caches).
        for b in &boxes {
            let _ = build_box_mesh(&spec, &grid, &field, b);
        }
        let start = Instant::now();
        let mut built = 0usize;
        for b in &boxes {
            if build_box_mesh(&spec, &grid, &field, b).is_some() {
                built += 1;
            }
        }
        let elapsed = start.elapsed();
        let per_box = elapsed / built.max(1) as u32;
        let step = (edge / shape.per_edge as f32) / shape.cells as f32;
        println!(
            "LOD {lod} ({} células × {} por coluna, passo {step:.1} m): {built} caixas em {elapsed:?} → {per_box:?}/caixa",
            shape.cells, shape.per_edge,
        );
        assert!(built > 0, "lod {lod}: nothing built");
    }
}

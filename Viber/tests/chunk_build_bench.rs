//! Micro-benchmark: how long does one terrain chunk mesh actually take?
//!
//! `update_chunk_lods` rebuilds chunks **inline on the main thread** with a
//! budget of `DEFAULT_MAX_MESH_BUILDS_PER_FRAME`. When the camera crosses the
//! reselect gate a burst of rebuilds lands on one frame, which is the shape of
//! the stutter reported while walking. This puts a number on the per-chunk
//! cost with the exact geometry `simple-rpg` authors, so the budget can be
//! argued from data instead of guessed.
//!
//! Run with: `cargo test --release --test chunk_build_bench -- --nocapture`

use std::time::Instant;

use bevy::math::Vec3;
use viber::terrain::mesh::{ChunkMeshParams, HeightField, TintParams, build_chunk_mesh};

/// Analytic terrain — cheap enough that the timing reflects the mesh builder,
/// not the sampler.
struct Rolling {
    max_height: f32,
}

impl HeightField for Rolling {
    fn sample(&self, x: f32, z: f32) -> f32 {
        let a = (x * 0.013).sin() * (z * 0.011).cos();
        let b = (x * 0.041).sin() * 0.35;
        (a + b) * 0.5 * self.max_height + self.max_height * 0.5
    }

    fn sample_normal(&self, x: f32, z: f32, epsilon: f32) -> Vec3 {
        let dx = self.sample(x + epsilon, z) - self.sample(x - epsilon, z);
        let dz = self.sample(x, z + epsilon) - self.sample(x, z - epsilon);
        Vec3::new(-dx, 2.0 * epsilon, -dz).normalize()
    }

    fn max_height(&self) -> f32 {
        self.max_height
    }
}

fn tint() -> TintParams {
    TintParams {
        base_color: [0.79, 0.77, 0.73, 1.0],
        color_low: [0.21, 0.31, 0.16, 1.0],
        color_mid: [0.46, 0.63, 0.31, 1.0],
        color_high: [0.91, 0.94, 1.0, 1.0],
        color_rock: [0.45, 0.42, 0.38, 1.0],
        snow_height: 0.92,
        slope_threshold: 0.93,
        slope_softness: 0.05,
        height_blend_strength: 0.22,
    }
}

/// `simple-rpg`: world-size 4000, max-height 200, chunk 64 m, LOD-0 step 1 m.
fn params(lod_step: usize) -> ChunkMeshParams {
    ChunkMeshParams {
        origin: Vec3::new(128.0, 0.0, -256.0),
        size: 64.0,
        lod_step,
        skirt_depth: 200.0 * 0.015625,
        normal_epsilon: 4000.0 / 1024.0,
        texture_tile_size: 5.0,
        levels: 5,
        world_size: 4000.0,
        tint: tint(),
        cliff_angle: 50.0,
    }
}

#[test]
fn bench_chunk_build_cost_per_lod() {
    let field = Rolling { max_height: 200.0 };
    println!("\n--- custo de build_chunk_mesh (chunk 64 m, simple-rpg) ---");
    for lod in 0u32..4 {
        let step = 1usize << lod;
        let p = params(step);
        // aquecer
        let warm = build_chunk_mesh(&field, &p, None).expect("build").expect("mesh");
        let runs = 20;
        let start = Instant::now();
        for _ in 0..runs {
            let _ = build_chunk_mesh(&field, &p, None).expect("build");
        }
        let each = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
        println!(
            "  LOD {lod} (passo {step} m): {:6.2} ms/chunk   {} vértices",
            each,
            warm.positions.len()
        );
    }
    println!(
        "\n  orçamento actual = {} chunks/frame → multiplique a linha LOD 0.",
        viber::terrain::spec::DEFAULT_MAX_MESH_BUILDS_PER_FRAME
    );
}

//! End-to-end checks for the voxel terrain layer, driven by the QA world.
//!
//! The claim under test is the one the heightfield cannot make: after parsing
//! `worlds/qa-voxel.xml` and carving it exactly as the engine does, there is a
//! place in the world where you can stand with **rock above your head**.

use bevy::math::Vec3;

use viber::recipes;
use viber::terrain::brush::BrushGrid;
use viber::terrain::features::apply_features;
use viber::terrain::heightmap::HeightMapU16;
use viber::terrain::voxel::{
    Bounds3, ChunkClass, VOXEL_CHUNK_CELLS, VoxelChunkParams, VoxelField, VoxelMod,
    build_voxel_mesh,
};
use viber::terrain::voxel::transvoxel_mesh::sliver_area2_floor;
use viber::xml;

use std::path::Path;

/// Parses and carves a world exactly like `terrain::runtime::bootstrap` does,
/// then builds the voxel field from its caves.
fn carve(world_file: &str) -> (BrushGrid, VoxelField, viber::terrain::spec::TerrainSpec) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join(world_file);
    let loaded = xml::include::load_world(&path).expect("world loads");
    let world = recipes::parse_world(&loaded.root_attrs, &loaded.nodes).expect("world parses");
    let mut pending = viber::recipes::spawn::PendingTerrain::default();
    viber::recipes::spawn::collect_terrain(&world.entities, &mut pending);
    let spec = pending.terrain.clone().expect("world has a <Terrain>");

    let map = HeightMapU16::procedural(&spec, spec.resolution.max(1) as usize);
    let mut grid = BrushGrid::from_height_map(
        &map,
        spec.world_size,
        spec.max_height,
        spec.height_smoothing,
    )
    .expect("grid builds");
    apply_features(&mut grid, &pending.features);

    // Same order as the bootstrap: cliff bands resolve against the CARVED
    // grid, then caves, then arches.
    let texel = grid.texel();
    let cliff_bands: Vec<viber::terrain::voxel::CliffBand> = pending
        .features
        .cliffs
        .iter()
        .filter_map(|cliff| viber::terrain::voxel::CliffBand::build(cliff, &grid, texel))
        .collect();
    let mut mods: Vec<Box<dyn VoxelMod>> = Vec::new();
    for (i, band) in cliff_bands.iter().enumerate() {
        mods.extend(band.clone().into_mods(&format!("cliff:{i}")));
    }
    for cave in &pending.features.caves {
        mods.extend(cave.build(&grid));
    }
    for arch in &pending.features.arches {
        mods.extend(arch.build(&grid));
    }
    let field = VoxelField::new(mods, spec.world_size, spec.chunk_size);
    (grid, field, spec)
}

#[test]
fn test_qa_world_parses_cliffs_caves_and_arches_into_voxel_mods() {
    let (_grid, field, _spec) = carve("qa-voxel.xml");
    assert!(!field.is_flat(), "the QA world authors 3D features");
    assert!(
        field.mods().len() > 20,
        "two tunnels of ~180 m at 4 m spacing plus cliff bands and an arch, got {}",
        field.mods().len()
    );
    // The `arch` cliff profile bores its window: exactly one label per
    // authored arch-band, deterministic by construction.
    let windows = field
        .mods()
        .iter()
        .filter(|m| m.label().ends_with(":window"))
        .count();
    assert_eq!(windows, 1, "the janela cliff must bore exactly one window");
}

#[test]
fn test_somewhere_in_the_tunnel_there_is_rock_overhead() {
    // The whole point of the exercise. A heightfield column has exactly one
    // surface; find a column with two, and a gap between them tall enough to
    // stand in.
    let (grid, field, _spec) = carve("qa-voxel.xml");

    let mut best: Option<(f32, f32, f32)> = None; // (headroom, x, z)
    for i in 0..=180 {
        let x = -90.0 + i as f32;
        let z = 6.0 - (i as f32 / 180.0) * 6.0 + (i as f32 / 180.0) * 2.0;
        let spans = field.column(&grid, x, z);
        if spans.len() < 2 {
            continue;
        }
        // Air between the underside of the roof and the floor below it.
        let headroom = spans[0].bottom - spans[1].top;
        if best.is_none_or(|(h, _, _)| headroom > h) {
            best = Some((headroom, x, z));
        }
    }

    let (headroom, x, z) = best.expect(
        "no column in the tunnel had two solid spans — the cave has no roof, \
         which is exactly the thing a heightfield could not do either",
    );
    assert!(
        headroom > 2.0,
        "the cave at ({x}, {z}) is only {headroom:.2} m tall — not walkable"
    );

    // And the top surface is still the hill, so every existing gameplay query
    // that asks `sample()` keeps getting the answer it always got.
    let top = field.surface_top(&grid, x, z);
    let spans = field.column(&grid, x, z);
    assert!(
        (top - spans[0].top).abs() < 0.01,
        "surface_top must report the hill above the cave"
    );
    // Standing inside the tunnel, the floor is the cave floor, not the hill.
    let inside = spans[0].bottom - 0.5;
    let floor = field
        .surface_below(&grid, x, z, inside)
        .expect("a cave has a floor");
    assert!(
        floor < top - 2.0,
        "the cave floor {floor:.2} must be well under the hilltop {top:.2}"
    );
}

#[test]
fn test_a_world_without_3d_features_stays_bit_for_bit_the_heightfield() {
    // The non-regression claim for every world authored before this existed.
    // qa-agua carves pads/lakes/rivers/roads — all heightfield work — and
    // authors no cliff, cave or arch, so the field must come out empty.
    let (grid, field, _spec) = carve("qa-agua.xml");
    assert!(
        field.is_flat(),
        "qa-agua authors no 3D feature but the field grew {} mods",
        field.mods().len()
    );
    for i in 0..64 {
        let x = -240.0 + i as f32 * 7.5;
        let z = 180.0 - i as f32 * 5.5;
        assert_eq!(
            field.surface_top(&grid, x, z),
            grid.sample(x, z),
            "flat world diverged from the heightfield at {x},{z}"
        );
    }
}

#[test]
fn test_only_the_chunks_the_caves_touch_are_volumetric() {
    // The cost argument: a world with two tunnels must not turn its whole
    // terrain into voxels.
    let (_grid, field, spec) = carve("qa-voxel.xml");
    let edge = spec.chunk_size;
    let half = spec.world_size * 0.5;
    let rows = (spec.world_size / edge).ceil() as i32;

    let mut volumetric = 0;
    let mut total = 0;
    for cz in 0..rows {
        for cx in 0..rows {
            let min = Vec3::new(-half + cx as f32 * edge, 0.0, -half + cz as f32 * edge);
            let bounds = Bounds3::from_corners(min, min + Vec3::new(edge, spec.max_height, edge));
            total += 1;
            if field.classify(&bounds) == ChunkClass::Volumetric {
                volumetric += 1;
            }
        }
    }
    assert!(volumetric > 0, "the caves must claim some chunks");
    assert!(
        volumetric < total,
        "{volumetric}/{total} chunks volumetric — the flat path must still carry most of the world"
    );
}

#[test]
fn test_the_tunnel_actually_meshes_into_geometry() {
    let (grid, field, spec) = carve("qa-voxel.xml");
    let voxel_size = 1.0_f32;
    let extent = VOXEL_CHUNK_CELLS as f32 * voxel_size;

    // Walk the boxes around the middle of the first tunnel until one meshes.
    let mut meshed = 0;
    let mut downward_normals = 0;
    for cy in -1..3 {
        for cx in -3..3 {
            let origin = Vec3::new(cx as f32 * extent, cy as f32 * extent, -extent);
            let params = VoxelChunkParams {
                origin,
                cells: VOXEL_CHUNK_CELLS,
                voxel_size,
                texture_tile_size: spec.texture_tile_size,
                tint: spec.chunk_tint(),
                max_height: spec.max_height,
                uses_layer_material: true,
                transitions: [false; 4],
            };
            let density = |p: Vec3| field.density(&grid, p);
            if let Some(data) = build_voxel_mesh(&density, &params) {
                meshed += 1;
                downward_normals += data.normals.iter().filter(|n| n[1] < -0.5).count();
                assert!(
                    data.positions
                        .iter()
                        .all(|p| p.iter().all(|c| c.is_finite())),
                    "non-finite vertex in a cave chunk"
                );
                assert_eq!(data.indices.len() % 3, 0);
            }
        }
    }
    assert!(meshed > 0, "no voxel chunk meshed around the tunnel");
    assert!(
        downward_normals > 0,
        "the tunnel produced no downward-facing surface — there is no ceiling"
    );
}

#[test]
fn test_two_runs_of_the_same_world_are_byte_identical() {
    // "Mesma seed, mesmo mundo" — the contract the pre-stroke probes used to
    // guarantee by hand. Compared as raw f32/u32 bits, not floats: NaN layout
    // drift and last-bit jitter both count as failures.
    let (grid_a, field_a, spec) = carve("qa-voxel.xml");
    let (grid_b, field_b, _spec) = carve("qa-voxel.xml");

    // The columns answer identically, bit for bit, over the tunnels, the
    // cliffs and the arch.
    for i in 0..200 {
        let x = -95.0 + i as f32 * 1.7;
        let z = -80.0 + (i * 37 % 160) as f32;
        for y in [0.0, 8.0, 20.0, 40.0] {
            assert_eq!(
                field_a.density(&grid_a, Vec3::new(x, y, z)).to_bits(),
                field_b.density(&grid_b, Vec3::new(x, y, z)).to_bits(),
                "density bits diverged at ({x}, {y}, {z})"
            );
        }
        let cols_a = field_a.column(&grid_a, x, z);
        let cols_b = field_b.column(&grid_b, x, z);
        assert_eq!(cols_a.len(), cols_b.len(), "span count diverged at {x},{z}");
        for (sa, sb) in cols_a.iter().zip(cols_b.iter()) {
            assert_eq!(sa.top.to_bits(), sb.top.to_bits());
            assert_eq!(sa.bottom.to_bits(), sb.bottom.to_bits());
        }
    }

    // And so does the mesh of the tunnel chunk.
    let voxel_size = 1.0_f32;
    let extent = VOXEL_CHUNK_CELLS as f32 * voxel_size;
    let params = VoxelChunkParams {
        origin: Vec3::new(-extent, 0.0, -extent),
        cells: VOXEL_CHUNK_CELLS,
        voxel_size,
        texture_tile_size: spec.texture_tile_size,
        tint: spec.chunk_tint(),
        max_height: spec.max_height,
        uses_layer_material: true,
        transitions: [false; 4],
    };
    let mesh_a =
        build_voxel_mesh(&|p| field_a.density(&grid_a, p), &params).expect("chunk meshes (a)");
    let mesh_b =
        build_voxel_mesh(&|p| field_b.density(&grid_b, p), &params).expect("chunk meshes (b)");
    let bits_a: Vec<u32> = mesh_a
        .positions
        .iter()
        .flat_map(|p| p.iter().map(|c| c.to_bits()))
        .collect();
    let bits_b: Vec<u32> = mesh_b
        .positions
        .iter()
        .flat_map(|p| p.iter().map(|c| c.to_bits()))
        .collect();
    assert_eq!(bits_a, bits_b, "vertex bits diverged between runs");
    assert_eq!(
        mesh_a.indices, mesh_b.indices,
        "indices diverged between runs"
    );
}

#[test]
fn test_the_tunnel_mesh_has_no_degenerate_triangles() {
    let (grid, field, spec) = carve("qa-voxel.xml");
    let voxel_size = 1.0_f32;
    let extent = VOXEL_CHUNK_CELLS as f32 * voxel_size;
    let params = VoxelChunkParams {
        origin: Vec3::new(-extent, 0.0, -extent),
        cells: VOXEL_CHUNK_CELLS,
        voxel_size,
        texture_tile_size: spec.texture_tile_size,
        tint: spec.chunk_tint(),
        max_height: spec.max_height,
        uses_layer_material: true,
        transitions: [false; 4],
    };
    let data = build_voxel_mesh(&|p| field.density(&grid, p), &params).expect("chunk meshes");
    assert!(data.indices.len() >= 3);
    // Marching cubes emite SLIVERS onde a isosuperfície passa rente a um canto
    // do lattice — dois vértices interpolados quase coincidentes. Não são
    // removíveis sem furar a malha (partilham arestas com triângulos bons), e
    // não se veem. O que tem de continuar verdade é que são uma MINORIA: um
    // mesher partido produz slivers às centenas de por cento, não a 1 %.
    let floor = sliver_area2_floor(voxel_size);
    let total = data.indices.len() / 3;
    let mut slivers = 0usize;
    for tri in data.indices.chunks_exact(3) {
        let (a, b, c) = (
            Vec3::from(data.positions[tri[0] as usize]),
            Vec3::from(data.positions[tri[1] as usize]),
            Vec3::from(data.positions[tri[2] as usize]),
        );
        if (b - a).cross(c - a).length_squared() <= floor {
            slivers += 1;
        }
    }
    let fraction = slivers as f32 / total as f32;
    assert!(
        fraction < 0.05,
        "{slivers}/{total} triângulos são slivers ({:.1} %) — o mesher degradou",
        fraction * 100.0
    );
}

#[test]
fn test_a_fully_interior_tunnel_chunk_is_watertight() {
    // A cave surface that never touches the chunk border must close: every
    // undirected edge shared by exactly two triangles. The neighbouring
    // chunk owns the faces at the outermost lattice plane, so any edge
    // touching it is exempt (the seam-coincidence has its own test in
    // `surface_nets.rs`).
    let (grid, field, spec) = carve("qa-voxel.xml");
    let voxel_size = 1.0_f32;
    let extent = VOXEL_CHUNK_CELLS as f32 * voxel_size;

    // Positions are chunk-relative on ALL axes: the lattice spans
    // [-1, extent + 1]. Vertices are smoothed inside their cell, so the
    // seam band is one voxel DEEP, not a plane: any vertex within a voxel
    // of either extreme may complete a face the neighbouring chunk owns.
    let on_border = |p: &[f32; 3]| {
        p[0] <= voxel_size
            || p[0] >= extent - voxel_size
            || p[1] <= voxel_size
            || p[1] >= extent - voxel_size
            || p[2] <= voxel_size
            || p[2] >= extent - voxel_size
    };
    let mut found = false;
    for z0 in [-extent, 0.0] {
        for cy in 0..2 {
            for cx in -3..3 {
                let origin = Vec3::new(cx as f32 * extent, cy as f32 * extent, z0);
                let params = VoxelChunkParams {
                    origin,
                    cells: VOXEL_CHUNK_CELLS,
                    voxel_size,
                    texture_tile_size: spec.texture_tile_size,
                    tint: spec.chunk_tint(),
                    max_height: spec.max_height,
                    uses_layer_material: true,
                    transitions: [false; 4],
                };
                let Some(data) = build_voxel_mesh(&|p| field.density(&grid, p), &params) else {
                    continue;
                };
                // Tunnel chunks only: the bare terrain sheet has no roof,
                // and its seam rim would pollute the claim under test.
                if !data.normals.iter().any(|n| n[1] < -0.5) {
                    continue;
                }
                // Undirected edge census.
                let mut edges: std::collections::HashMap<(u32, u32), u32> =
                    std::collections::HashMap::new();
                for tri in data.indices.chunks_exact(3) {
                    for k in 0..3 {
                        let (a, b) = (tri[k], tri[(k + 1) % 3]);
                        *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                    }
                }
                let open: Vec<_> = edges
                    .iter()
                    .filter(|item| *item.1 == 1)
                    .filter(|item| {
                        let (a, b) = item.0;
                        !on_border(&data.positions[*a as usize])
                            && !on_border(&data.positions[*b as usize])
                    })
                    .collect();
                assert!(
                    open.is_empty(),
                    "{} interior edges have a single face — the chunk leaks",
                    open.len()
                );
                found = true;
            }
        }
    }
    assert!(found, "no tunnel chunk meshed to check");
}

#[test]
fn test_the_free_standing_arch_gives_a_walker_two_spans() {
    // The acceptance query for `<Arch>`: under the crown the column holds
    // ground + air + the arch band, so `column()` answers 2 spans and
    // `surface_below` puts the walker on the GROUND, not the band.
    let (grid, field, _spec) = carve("qa-voxel.xml");
    let mut best: Option<(f32, f32, f32, usize)> = None; // (band thickness, x, z, spans)
    for dz in -4..=4 {
        for dx in -4..=4 {
            let x = 72.0 + dx as f32;
            let z = -18.0 + dz as f32;
            let spans = field.column(&grid, x, z);
            if spans.len() >= 2 {
                let t = spans[0].thickness();
                if best.is_none_or(|(bt, _, _, _)| t > bt) {
                    best = Some((t, x, z, spans.len()));
                }
            }
        }
    }
    let (thickness, x, z, _spans) =
        best.expect("no two-span column near the arch — the portal has no opening");
    let spans = field.column(&grid, x, z);
    assert!(
        spans.len() == 2,
        "expected exactly 2 spans under the arch at ({x}, {z}), got {}",
        spans.len()
    );
    assert!(
        thickness < 12.0,
        "the top span at ({x}, {z}) is {thickness:.1} m thick — that is not a walkable band"
    );
    // Standing in the opening (just under the band underside) and asking for
    // the ground BELOW gets the plaza, not the band top.
    let from = spans[0].bottom - 0.5;
    let ground = field
        .surface_below(&grid, x, z, from)
        .expect("solid ground under the arch");
    assert!(
        ground <= spans[1].top + 0.5,
        "the floor under the arch at ({x}, {z}) is {ground:.2} vs span top {:.2}",
        spans[1].top
    );
}

#[test]
fn test_folded_cliff_walls_have_no_holes_and_bounded_flips() {
    // The visual gate that caught the wall-speckling: a cliff wall meshed
    // from folded profiles (terraces, undercuts, gullies) must have no
    // topological holes anywhere, and the backfacing-triangle count — the
    // GPU-culled "polygonal holes" — must stay bounded. Known residual:
    // sub-voxel thin sheets (a carved void passing under the natural
    // terrain) put both crossings in one cell and yield a handful of
    // flipped triangles along the sheet line; killing those needs voxel
    // refinement near features and is intentionally NOT attempted here.
    let (grid, field, spec) = carve("qa-cliffs.xml");
    let voxel_size = 1.0_f32;
    let extent = VOXEL_CHUNK_CELLS as f32 * voxel_size;

    // Positions are chunk-relative on ALL axes; the seam band is one voxel
    // deep on each face of the chunk.
    let on_border = |p: &[f32; 3]| {
        p[0] <= voxel_size
            || p[0] >= extent - voxel_size
            || p[1] <= voxel_size
            || p[1] >= extent - voxel_size
            || p[2] <= voxel_size
            || p[2] >= extent - voxel_size
    };

    let mut total_flipped = 0usize;
    let xs: Vec<f32> = (-6i32..7).map(|i| i as f32 * extent).collect();
    let zs: Vec<f32> = (-4i32..=6).map(|i| i as f32 * extent).collect();
    for z0 in zs {
        for cy in 0..3 {
            for x0 in &xs {
                let origin = Vec3::new(*x0, cy as f32 * extent, z0);
                let params = VoxelChunkParams {
                    origin,
                    cells: VOXEL_CHUNK_CELLS,
                    voxel_size,
                    texture_tile_size: spec.texture_tile_size,
                    tint: spec.chunk_tint(),
                    max_height: spec.max_height,
                    uses_layer_material: true,
                    transitions: [false; 4],
                };
                let Some(data) = build_voxel_mesh(&|p| field.density(&grid, p), &params) else {
                    continue;
                };
                // No topological holes: every interior edge has exactly two
                // triangles.
                let mut edges: std::collections::HashMap<(u32, u32), u32> =
                    std::collections::HashMap::new();
                for tri in data.indices.chunks_exact(3) {
                    for k in 0..3 {
                        let (a, b) = (tri[k], tri[(k + 1) % 3]);
                        *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                    }
                }
                let holes = edges
                    .iter()
                    .filter(|item| *item.1 == 1)
                    .filter(|item| {
                        let (a, b) = item.0;
                        !on_border(&data.positions[*a as usize])
                            && !on_border(&data.positions[*b as usize])
                    })
                    .count();
                assert_eq!(holes, 0, "interior holes in chunk origin {origin}");
                // Winding: geometric normal must agree with the field
                // gradient (outward = +gradient); seam-band triangles are
                // exempt (their other half lives in the neighbour).
                for tri in data.indices.chunks_exact(3) {
                    if on_border(&data.positions[tri[0] as usize])
                        || on_border(&data.positions[tri[1] as usize])
                        || on_border(&data.positions[tri[2] as usize])
                    {
                        continue;
                    }
                    let (a, b, c) = (
                        Vec3::from(data.positions[tri[0] as usize]),
                        Vec3::from(data.positions[tri[1] as usize]),
                        Vec3::from(data.positions[tri[2] as usize]),
                    );
                    let gn = (b - a).cross(c - a);
                    if gn.length_squared() < 1e-12 {
                        continue;
                    }
                    let centroid = origin + (a + b + c) / 3.0;
                    let e = 0.5;
                    let g = Vec3::new(
                        field.density(&grid, centroid + Vec3::X * e)
                            - field.density(&grid, centroid - Vec3::X * e),
                        field.density(&grid, centroid + Vec3::Y * e)
                            - field.density(&grid, centroid - Vec3::Y * e),
                        field.density(&grid, centroid + Vec3::Z * e)
                            - field.density(&grid, centroid - Vec3::Z * e),
                    );
                    if g.dot(gn) < 0.0 {
                        total_flipped += 1;
                    }
                }
            }
        }
    }
    assert!(
        total_flipped <= 200,
        "{total_flipped} backfacing triangles across the cliff walls — the \
         thin-sheet speckling regressed past its documented bound"
    );
}

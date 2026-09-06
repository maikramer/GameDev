//! Terrain runtime — turns the declarative specs into a carved world.
//!
//! Owns the one-shot startup pipeline (exclusive systems, so no archetype
//! churn in the frame loop):
//!
//! 1. **Grid** — heightmap file (PNG 8/16-bit grayscale, or the VibeGame
//!    packed `.ahgt` format, decoded natively by `HeightMapU16::from_ahgt`;
//!    blocking read is fine at startup) or the deterministic procedural field.
//!    A heightmap that fails to load falls back to procedural with a warning.
//! 2. **Features** — [`apply_features`] runs pads → water → roads on the
//!    [`BrushGrid`] (the VibeGame order), producing the query registries.
//! 3. **Entities** — chunk meshes (LOD 0, integer grid step; dynamic LOD
//!    selection arrives with the core plugin), water mirrors/ribbons and road
//!    ribbons; registries land as resources for gameplay queries
//!    (`avoid-water`, `isPointOnRoad`, ground height sampling).
//!
//! Headless `analyze` never runs this — it only parses and validates.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use bevy::render::mesh::{Indices, PrimitiveTopology};

use super::brush::BrushGrid;
use super::features::{FeatureResult, apply_features};
use super::heightmap::HeightMapU16;

use super::layer_material::{TerrainChunkMaterial, TerrainChunkParams};
use super::mesh::{ChunkMeshParams, build_chunk_mesh};
use super::roads::RoadPath;
use super::sampler::ResolvedPad;
use super::spec::TerrainSpec;
use super::splat::{
    SLOT_GRAVEL, SLOT_RIVERBED, SplatParams, chunk_splat_image, generate_chunk_splats, pool_albedo,
    solid_white_image,
};
use super::voxel::{Span, VoxelField};
use super::water::{WaterBody, lake_water_mesh, river_water_mesh};
use super::water_material::{WaterExtension, WaterMaterial};
use crate::recipes::spawn::PendingTerrain;
use crate::textures::WorldTiledTextures;

/// One texture still loading, watched by [`drop_failed_terrain_textures`].
#[derive(Debug, Clone)]
pub enum WatchedTexture {
    /// Legacy single-texture material (terrain fallback path, road ribbons,
    /// junction discs, ground decals).
    Standard {
        material: Handle<StandardMaterial>,
        texture: Handle<Image>,
    },
    /// One layer slot (0..3) of a chunk layer material. `repoint` is the
    /// texture the slot falls back to when this one never arrives.
    Layer {
        material: Handle<TerrainChunkMaterial>,
        slot: usize,
        texture: Handle<Image>,
        repoint: Handle<Image>,
    },
}

/// Textures whose materials are still waiting on their images, so a texture
/// that never arrives can be dropped instead of blanking what it was painting.
///
/// Bevy will not prepare a material until every texture it references is
/// resident, and a mesh with an unprepared material is simply not drawn. A
/// single missing texture therefore makes the whole terrain disappear —
/// which is exactly what `simple-rpg` hit when it asked for `vale_grass.png`
/// before the texture pool existed.
#[derive(Resource, Default)]
pub struct PendingTerrainTextures {
    /// Entries still waiting on their image.
    pub watched: Vec<WatchedTexture>,
}

/// Drops textures that failed to load from the materials referencing them.
///
/// The legacy material falls back to its vertex-color tint (terrain) or flat
/// base color (roads) — a texture-less surface instead of no surface. Layer
/// slots degrade one step softer: a failed slot is repointed to the first
/// OTHER texture of the chunk (leito → gravel quando o chunk o carrega),
/// nunca a ela própria — o repoint antigo para a dominante era o próprio
/// slot falhado na maioria dos chunks e caía no branco sólido.
///
/// Sampler ownership does NOT live here: world-tiled textures are registered
/// with [`crate::textures::WorldTiledTextures`] at `load` time and the
/// sampler is settled once by the single-writer texture pass. Writing it
/// here too re-opened the clamp/REPEAT race that stretched ground textures.
pub fn drop_failed_terrain_textures(
    server: Res<AssetServer>,
    mut pending: ResMut<PendingTerrainTextures>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_materials: Option<ResMut<Assets<TerrainChunkMaterial>>>,
    mut images: ResMut<Assets<Image>>,
) {
    if pending.watched.is_empty() {
        return;
    }
    pending.watched.retain(|watched| match watched {
        WatchedTexture::Standard { material, texture } => {
            match server.get_load_state(texture) {
                Some(bevy::asset::LoadState::Failed(error)) => {
                    warn!(
                        "terrain texture failed to load ({error}); rendering untextured \
                         instead of hiding the surface"
                    );
                    if let Some(mut material) = materials.get_mut(material) {
                        material.base_color_texture = None;
                    }
                    false
                }
                // Loaded: nada a fazer — sampler/mips são responsabilidade do
                // `crate::textures` (escritor único).
                Some(bevy::asset::LoadState::Loaded) => false,
                _ => true,
            }
        }
        WatchedTexture::Layer {
            material,
            slot,
            texture,
            repoint,
        } => match server.get_load_state(texture) {
            Some(bevy::asset::LoadState::Failed(error)) => {
                // O repoint foi escolhido no spawn (leito → gravel quando o
                // chunk o carrega, resto → layer dominante); se ele também
                // caiu, branco sólido — o chão fica de pé.
                warn!(
                    "terrain chunk layer {slot} failed to load ({error}); repointing \
                     the slot to its fallback texture"
                );
                if let Some(chunk_materials) = chunk_materials.as_mut() {
                    if let Some(mut layer) = chunk_materials.get_mut(material) {
                        let fallback = images
                            .contains(repoint)
                            .then(|| repoint.clone())
                            .unwrap_or_else(|| images.add(solid_white_image()));
                        *layer.texture_mut(*slot) = fallback;
                    }
                }
                false
            }
            Some(bevy::asset::LoadState::Loaded) => false,
            _ => true,
        },
    });
}

/// Loads a world-tiled texture via [`crate::textures::load_tiled_image`],
/// tolerating apps that forgot to init the registry (headless test apps):
/// the texture still loads, only the repeat sampler is lost, and the warn
/// says so.
pub(crate) fn load_world_texture(
    server: &AssetServer,
    world: &mut World,
    texture: &str,
) -> Handle<Image> {
    match world.get_resource_mut::<WorldTiledTextures>() {
        Some(mut tiled) => crate::textures::load_tiled_image(server, &mut tiled, texture),
        None => {
            warn!(
                "texture `{texture}` is world-tiled but `WorldTiledTextures` is not \
                 initialized — the repeat sampler will not be applied"
            );
            // strip leading '/' — bevy treats root-absolute asset paths as unapproved
            server.load(texture.trim_start_matches('/').to_string())
        }
    }
}

/// Per-chunk layer materials, published by the bootstrap: one material per
/// terrain chunk cell (the four pool textures with the highest aggregate
/// weight in that chunk + its own splat plane), keyed by chunk `(cx, cz)`.
#[derive(Debug, Clone)]
pub struct ChunkLayerMap {
    /// Chunks per world side (`world_size / edge`).
    pub rows: u32,
    /// Chunk edge in meters (matches the mesh grid of `spawn_chunks`).
    pub edge: f32,
    pub materials: std::collections::HashMap<(u32, u32), Handle<TerrainChunkMaterial>>,
}

impl ChunkLayerMap {
    pub fn get(&self, cx: u32, cz: u32) -> Option<&Handle<TerrainChunkMaterial>> {
        self.materials.get(&(cx, cz))
    }
}

/// Materials the terrain chunks render with, published by the bootstrap for
/// the LOD plugin (respawned chunks reuse them). Exactly one arm is `Some`
/// after a successful bootstrap: the per-chunk layer materials when the
/// world opted in (`layers`) and the render plugins are live, the legacy
/// single-texture material otherwise.
#[derive(Resource, Debug, Default, Clone)]
pub struct TerrainChunkMaterials {
    pub layer: Option<ChunkLayerMap>,
    pub standard: Option<Handle<StandardMaterial>>,
}

impl TerrainChunkMaterials {
    /// The layer handle ONE chunk should (re)spawn with.
    pub fn chunk_layer(&self, cx: u32, cz: u32) -> Option<Handle<TerrainChunkMaterial>> {
        self.layer.as_ref()?.get(cx, cz).cloned()
    }
}

/// Typed handle of whichever material a chunk carries.
#[derive(Debug, Clone)]
pub enum ChunkMaterialHandle {
    Layer(Handle<TerrainChunkMaterial>),
    Standard(Handle<StandardMaterial>),
}

/// Carved world state, published after the bootstrap for gameplay queries.
///
/// `grid` and `voxel` live behind `Arc`s so [`Self::reader`] can hand them to
/// readers outside the ECS (the Luau context) without copying — the terrain is
/// immutable after the bootstrap (destructible terrain is not in this round),
/// so sharing IS the single source of height, not a copy of it.
#[derive(Resource)]
pub struct TerrainRuntime {
    pub spec: TerrainSpec,
    pub grid: std::sync::Arc<BrushGrid>,
    pub water: Vec<WaterBody>,
    pub roads: Vec<RoadPath>,
    pub pads: Vec<ResolvedPad>,
    /// The 3D shape layer over `grid`: cliff bodies, overhangs, caves.
    ///
    /// Empty in a world that authors no 3D feature, and empty is free — every
    /// query below then costs exactly what it cost before this field existed.
    pub voxel: std::sync::Arc<VoxelField>,
}

/// Shared read handle over the carved world, for readers outside the ECS
/// (the Luau script context). Cheap to clone; the data behind it never
/// changes after the bootstrap.
#[derive(Debug, Clone)]
pub struct TerrainReader {
    pub grid: std::sync::Arc<BrushGrid>,
    pub voxel: std::sync::Arc<VoxelField>,
}

/// A standing surface thinner than this with hollow ground below is a slab
/// (arch band, tight overhang brow) — no prop should root on it.
const MIN_STAND_THICKNESS: f32 = 4.0;

impl TerrainRuntime {
    /// Shared read handle over this runtime (two `Arc` clones).
    pub fn reader(&self) -> TerrainReader {
        TerrainReader {
            grid: self.grid.clone(),
            voxel: self.voxel.clone(),
        }
    }

    /// True when the column's standing surface is a THIN slab floating over
    /// hollow ground — the band of an `<Arch>`, the brow of a tight
    /// overhang. A cave roof under a hill does NOT count: the surface there
    /// is the hill, thick and honest. Flat world is always `false`.
    pub fn has_thin_roof(&self, x: f32, z: f32) -> bool {
        if self.voxel.is_flat() {
            return false;
        }
        let spans = self.voxel.column(&*self.grid, x, z);
        spans.len() >= 2 && spans[0].thickness() < MIN_STAND_THICKNESS
    }
    /// Ground height at a world XZ position (meters).
    ///
    /// This is the **topmost** solid surface. It keeps that meaning now that a
    /// column can have several: an entity placed by XZ alone belongs on top of
    /// the world, and the 44 call sites that ask this question want the roof of
    /// the arch, not its underside. Code that needs to be under something asks
    /// [`Self::surface_below`] instead.
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        if self.voxel.is_flat() {
            return self.grid.sample(x, z);
        }
        self.voxel.surface_top(&*self.grid, x, z)
    }

    /// Topmost solid surface at or below `from_y`, or `None` when there is
    /// none — the query for anything standing inside a cave or under a ledge.
    pub fn surface_below(&self, x: f32, z: f32, from_y: f32) -> Option<f32> {
        self.voxel.surface_below(&*self.grid, x, z, from_y)
    }

    /// Every solid interval in this column, top-down. One span is ordinary
    /// ground; two or more mean an overhang, an arch or a cave.
    pub fn column(&self, x: f32, z: f32) -> Vec<Span> {
        self.voxel.column(&*self.grid, x, z)
    }

    /// True when `p` is inside solid rock.
    pub fn is_solid(&self, p: Vec3) -> bool {
        self.voxel.density(&*self.grid, p) < 0.0
    }

    /// Point is inside a water carve zone (`avoid-water`).
    pub fn in_water(&self, x: f32, z: f32) -> bool {
        self.water.iter().any(|w| w.contains(Vec2::new(x, z)))
    }

    /// Water surface Y at a world XZ position when the point sits over a
    /// water body (`None` on dry land) — `in-water` placement anchors to the
    /// BLADE of water, not the carved bed. Overlapping bodies take the
    /// highest surface (a river feeding a lake keeps the pond level at the
    /// junction).
    pub fn water_surface_at(&self, x: f32, z: f32) -> Option<f32> {
        let p = Vec2::new(x, z);
        self.water
            .iter()
            .filter(|w| w.contains(p))
            .filter_map(|w| w.surface_y_at(p))
            .fold(None::<f32>, |acc, y| Some(acc.map_or(y, |max| max.max(y))))
    }

    /// Point is on a road ribbon (`isPointOnRoad`).
    pub fn on_road(&self, x: f32, z: f32) -> bool {
        self.roads.iter().any(|r| r.is_on_road(Vec2::new(x, z)))
    }

    /// Height of the *rendered* surface at a world XZ position.
    ///
    /// Volumetric columns mesh straight from the field (surface nets tracks
    /// the zero crossing to sub-voxel accuracy), so the analytic top IS the
    /// drawn surface there and the bisected [`VoxelField::surface_top`] is
    /// the answer. The lattice reproduction below only matches the heightfield
    /// chunk triangulation, which is what flat chunks actually draw.
    ///
    /// Chunk meshes draw flat triangles between vertices spaced `lod0_step`
    /// apart, so the smoothed analytic sample between two vertices can sit
    /// above the visible surface on ridges — spawned trees floated with the
    /// root flare in the air. This reproduces the chunk lattice (anchored at
    /// `-world_size/2`, spacing [`lod0_step`]) and its triangulation
    /// (`build_chunk_mesh`: triangles a/c/b and b/c/d per cell), keeping props
    /// flush with what is actually drawn. On a lattice vertex the result
    /// equals [`BrushGrid::sample`]. Queries that need the ground UNDER a
    /// ceiling (cave interior, arch opening) use [`Self::surface_below`].
    pub fn sample_mesh_surface(&self, x: f32, z: f32) -> f32 {
        if !self.voxel.is_flat() {
            return self.voxel.surface_top(&*self.grid, x, z);
        }
        let step = lod0_step(&self.spec) as f32;
        let half = self.spec.world_size * 0.5;
        let gx = (x + half) / step;
        let gz = (z + half) / step;
        let x0 = gx.floor();
        let z0 = gz.floor();
        let fx = gx - x0;
        let fz = gz - z0;
        let lx0 = x0 * step - half;
        let lz0 = z0 * step - half;
        // Quad corners, matching build_chunk_mesh's vertex layout and
        // triangulation: a=(x,z) b=(x+1,z) c=(x,z+1) d=(x+1,z+1);
        // triangles (a, c, b) and (b, c, d).
        let h_a = self.grid.sample(lx0, lz0);
        let h_b = self.grid.sample(lx0 + step, lz0);
        let h_c = self.grid.sample(lx0, lz0 + step);
        let h_d = self.grid.sample(lx0 + step, lz0 + step);
        if fx + fz <= 1.0 {
            h_a + fx * (h_b - h_a) + fz * (h_c - h_a)
        } else {
            h_d + (1.0 - fx) * (h_c - h_d) + (1.0 - fz) * (h_b - h_d)
        }
    }
}

/// Terrain feature plugin: consumes [`PendingTerrain`] at startup and builds
/// the carved world. Works headless (no render plugins) — meshes land in
/// `Assets<Mesh>` regardless; visibility only appears with render plugins.
#[derive(Default)]
pub struct TerrainFeaturesPlugin;

impl bevy::app::Plugin for TerrainFeaturesPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_resource::<PendingTerrainTextures>()
            // Idempotente com `textures::TexturesPlugin`: o bootstrap regista
            // texturas world-tiled mesmo em apps headless de teste.
            .init_resource::<crate::textures::WorldTiledTextures>()
            .add_systems(
                bevy::app::Startup,
                bootstrap.after(crate::recipes::spawn::startup),
            )
            .add_systems(
                bevy::app::Update,
                (
                    drop_failed_terrain_textures,
                    // Sem isto o chão do splat ficava com o albedo de dia às
                    // 23:00 (a função existia e nunca corria).
                    super::layer_material::terrain_daynight_tint,
                ),
            );
    }
}

/// Exclusive startup: PendingTerrain → grid → carve → entities → registries.
pub fn bootstrap(world: &mut World) {
    let Some(pending) = world.remove_resource::<PendingTerrain>() else {
        return;
    };
    let Some(spec) = pending.terrain.clone() else {
        return;
    };

    // 1. Height grid.
    let mut spec = spec.clone();
    let map = match &spec.heightmap {
        Some(path) => match load_heightmap(pending.base_dir.as_deref(), path) {
            Ok(loaded) => {
                // The heightmap file describes its own coverage, but the world
                // XML wins when it states one: pads, lakes, rivers, roads and
                // biome polygons are all authored in the XML's coordinate
                // space. `simple-rpg` asks for 4000 m while its `.ahgt` header
                // claims 8000 — taking the file stretched the whole world to
                // double scale and quadrupled the chunk grid (63² → 125²).
                // O campo de biomas vem sempre do ficheiro (o XML não o
                // declara): é autorado com o relevo, no mesmo gerador.
                if !loaded.biomes.is_empty() {
                    spec.biomes = loaded.biomes.clone();
                }
                let authored = spec.extent_authored;
                for (label, from_file, target) in [
                    ("world-size", loaded.world_size, &mut spec.world_size),
                    ("max-height", loaded.max_height, &mut spec.max_height),
                ] {
                    let Some(from_file) = from_file else { continue };
                    if !authored {
                        *target = from_file;
                    } else if (from_file - *target).abs() > 1e-3 {
                        warn!(
                            "heightmap `{path}` declares {label} {from_file}, the world XML says \
                             {} — keeping the authored value",
                            *target
                        );
                    }
                }
                loaded.map
            }
            Err(error) => {
                warn!("heightmap `{path}` unavailable ({error:#}); using the procedural field");
                HeightMapU16::procedural(&spec, spec.resolution.max(1) as usize)
            }
        },
        None => HeightMapU16::procedural(&spec, spec.resolution.max(1) as usize),
    };
    let mut grid = match BrushGrid::from_height_map(
        &map,
        spec.world_size,
        spec.max_height,
        spec.height_smoothing,
    ) {
        Ok(grid) => grid,
        Err(error) => {
            error!("terrain grid rejected its heightmap: {error:#}");
            return;
        }
    };

    // 2. Features (pads → water → roads). Cliffs no longer carve.
    let result = apply_features(&mut grid, &pending.features);

    // 2.1 Cliff bands. Resolved against the CARVED grid, so a wall beside a
    // road or a lake adapts to the ground as built, and resolved BEFORE the
    // mask so the mask can take their footprint exactly instead of hunting
    // for it. Every terrain probe happens here, before a single mod exists —
    // the same determinism rule the carve kept by sampling before it opened
    // its stroke. (If the sharpen pass below rewrites the ground, the bands'
    // HEIGHTS are re-probed against the terraced field before any mod is
    // built; footprints stay as resolved.)
    let texel = grid.texel();
    let mut cliff_bands: Vec<super::voxel::CliffBand> = Vec::new();
    let mut cliff_band_owner: Vec<usize> = Vec::new();
    for (i, spec_cliff) in pending.features.cliffs.iter().enumerate() {
        if let Some(band) = super::voxel::CliffBand::build(spec_cliff, &grid, texel) {
            cliff_band_owner.push(i);
            cliff_bands.push(band);
        }
    }

    // 2.5 Opt-in sharpen — rewrites smooth steep ramps of the FINAL field
    // into terraced cliff bands, but ONLY inside accepted cliff regions: a
    // region-filtered pre-mask gates the pass, and the mask is rebuilt over
    // the terraced field afterwards for every consumer.
    let scan_angle = if spec.sharpen_angle.is_finite() && spec.sharpen_angle > 0.0 {
        spec.sharpen_angle
    } else {
        crate::terrain::spec::DEFAULT_SHARPEN_ANGLE
    };
    // The sharpen pass only adds steps INSIDE this mask's core, so the
    // pre-sharpen mask stays valid as the final consumer mask: rebuilding
    // over the terraced field would fragment into 5-texel riser slivers
    // (the per-texel dither) and the walls would lose their rock.
    let scan_angle = if spec.sharpen
        && spec.sharpen_angle.is_finite()
        && spec.sharpen_angle > 0.0
        && spec.sharpen_angle < spec.cliff_angle
    {
        spec.sharpen_angle
    } else {
        spec.cliff_angle
    };
    let cliff_mask = crate::terrain::cliffs::CliffMask::build_with(
        &grid,
        scan_angle,
        spec.cliff_min_area,
        spec.cliff_min_drop,
        spec.cliff_min_extent,
    );
    // Authored walls and their aprons join the public query layer: grass and
    // spawners inherit the exclusion, the splat paints stone on the wall and
    // gravel on the apron. `build_with` above only found the NATURAL steep
    // ground — the authored walls are not in the height grid any more, so
    // without this they would be invisible to every one of those consumers.
    let mut cliff_mask = cliff_mask;
    cliff_mask.add_talus(&result.cliffs);
    cliff_mask.add_authored_bands(&cliff_bands);

    // Margens voxel (bank="gorge"/"overhang"): paredes sólidas a partir das
    // estações dos corpos de água. Entram na máscara (pedra no splat +
    // exclusão de relva/spawners) e no campo voxel (o sólido 3D).
    let mut bank_bands: Vec<super::voxel::CliffBand> = Vec::new();
    for (body, &(is_lake, i)) in result.water.iter().zip(&result.water_specs) {
        match (is_lake, i) {
            (true, i) if i < pending.features.lakes.len() => {
                let lake = &pending.features.lakes[i];
                if lake.bank.is_voxel()
                    && let Some(band) =
                        super::voxel::riverbank::lake_shore_band(lake, body, &grid, grid.texel())
                {
                    bank_bands.push(band);
                }
            }
            (false, i) if i < pending.features.rivers.len() => {
                let river = &pending.features.rivers[i];
                bank_bands.extend(super::voxel::riverbank::river_banks(
                    river, body, &grid, grid.texel(),
                ));
                // Nascente: ferradura de rocha na estação 0, boca a jusante.
                if river.spring
                    && let Some(band) =
                        super::voxel::riverbank::spring_band(river, body, &grid, grid.texel())
                {
                    bank_bands.push(band);
                }
            }
            _ => {}
        }
    }
    if !bank_bands.is_empty() {
        cliff_mask.add_authored_bands(&bank_bands);
    }
    if spec.sharpen {
        let changed = crate::terrain::cliffs::sharpen_terrain(
            &mut grid,
            &spec,
            &result.water,
            &cliff_mask,
            &result.cliffs,
        );
        if changed > 0 {
            info!("sharpen terraced {changed} texels into cliff bands");
        }
    }

    // 3. Entities. Assets are removed/reinserted to avoid aliasing `&mut World`
    //    (same pattern as `spawn::startup`).
    let mut meshes = world
        .remove_resource::<Assets<Mesh>>()
        .expect("Assets<Mesh> exists before startup systems run");
    let mut materials = world
        .remove_resource::<Assets<StandardMaterial>>()
        .expect("Assets<StandardMaterial> exists before startup systems run");
    // O MaterialPlugin (que registra o asset) só existe no `run` com render —
    // os testes headless correm o bootstrap sem ele, pelo que o slot da água
    // pode não existir (e então não há material para ninguém).
    let mut water_materials = world
        .remove_resource::<Assets<WaterMaterial>>()
        .unwrap_or_default();
    let mut chunk_materials = world
        .remove_resource::<Assets<TerrainChunkMaterial>>()
        .unwrap_or_default();
    let mut images = world.remove_resource::<Assets<Image>>().unwrap_or_default();
    let asset_server = world.get_resource::<AssetServer>().cloned();

    let mut watched: Vec<WatchedTexture> = Vec::new();

    // Terrain layer blend (`layers="…"`): per-chunk materials (4 pool
    // textures + 1 splat plane each). Without the render plugins (headless
    // tests) the asset slot doesn't exist and the world degrades to the
    // legacy single-texture path.
    let layer_map = if spec.layers.is_empty() {
        None
    } else if asset_server.is_none() {
        warn!("terrain layers requested but no AssetServer — using the legacy path");
        None
    } else if std::env::var("VIBER_CHUNK_LAYERS").is_ok_and(|v| v == "0") {
        // Escape hatch: `VIBER_CHUNK_LAYERS=0` volta ao tint legado sem
        // editar o mundo. (Até 2026-09-05 o default era o INVERSO — o
        // sistema estava desligado por um SIGSEGV do driver que se provou
        // ser o `#[uniform]` do material, não as texturas; ver
        // `TerrainChunkMaterial`.)
        warn!("terrain `layers` disabled by VIBER_CHUNK_LAYERS=0 — using the legacy tint");
        None
    } else {
        let (edge, rows) = chunk_grid(&spec);
        Some(spawn_chunk_materials(
            &spec,
            &grid,
            &result,
            &cliff_mask,
            asset_server.as_ref().expect("checked above"),
            world,
            &mut images,
            &mut chunk_materials,
            &mut watched,
            edge,
            rows,
        ))
    };

    // Chunk LODs are picked against the camera the world spawned, so a large
    // world does not materialise at full detail (see `spawn_chunks`).
    let camera_xz = world
        .query_filtered::<&Transform, With<Camera3d>>()
        .iter(world)
        .next()
        .map(|t| Vec2::new(t.translation.x, t.translation.z));

    let root = world
        .spawn((
            Name::new("terrain"),
            Transform::default(),
            Visibility::Inherited,
        ))
        .id();
    // The 3D shape layer. Built AFTER the carve, on purpose: a `<Cave>` takes
    // its height from the terrain, so a tunnel under a road bed has to see the
    // road bed as built, not the terrain as generated.
    //
    // A world that authors no 3D feature gets the mod-less field, every chunk
    // classifies `Flat`, and the heightfield path below runs exactly as it did
    // before any of this existed. `<Cliff>` joins in Phase B.
    let mut voxel_mods: Vec<Box<dyn super::voxel::VoxelMod>> = Vec::new();
    for (i, band) in cliff_bands.iter().enumerate() {
        voxel_mods.extend(band.clone().into_mods(&format!("cliff:{i}")));
    }
    for cave in &pending.features.caves {
        voxel_mods.extend(cave.build(&grid));
    }
    for arch in &pending.features.arches {
        voxel_mods.extend(arch.build(&grid));
    }
    // Margens de água voxel (gorge/overhang) — as mesmas bandas da máscara.
    for (i, band) in bank_bands.iter().enumerate() {
        voxel_mods.extend(band.clone().into_mods(&format!("riverbank:{i}")));
    }
    if !voxel_mods.is_empty() {
        info!(
            "terrain: {} cliff(s) + {} cave(s) + {} arch(es) + {} bank wall(s) -> {} voxel mods",
            cliff_bands.len(),
            pending.features.caves.len(),
            pending.features.arches.len(),
            bank_bands.len(),
            voxel_mods.len()
        );
    }
    let voxel = VoxelField::new(voxel_mods, spec.world_size, spec.chunk_size);

    let chunk_standard = spawn_chunks(
        world,
        &mut meshes,
        &mut materials,
        asset_server.as_ref(),
        root,
        &spec,
        &grid,
        camera_xz,
        &mut watched,
        layer_map.as_ref(),
        if spec.layers.is_empty() {
            None
        } else {
            Some(&cliff_mask)
        },
        &voxel,
    );
    // Chunks `spawn_chunks` skipped as volumetric are covered here instead.
    // Material PRÓPRIO double-sided para o caminho standard dos chunks voxel
    // (sem layers ou VIBER_CHUNK_LAYERS=0): shells que roçam o terreno
    // deixam folhas finas sub-voxel com triângulos inward-wound — com
    // culling leem-se como buracos na parede. O caminho de layers já resolve
    // isto no `specialize` do TerrainChunkMaterial (cull_mode None).
    let voxel_standard = {
        let mut m = materials.get(&chunk_standard).cloned().unwrap_or_default();
        m.double_sided = true;
        materials.add(m)
    };
    let voxel_stats = super::voxel::spawn_voxel_chunks(
        world,
        &mut meshes,
        root,
        &spec,
        &grid,
        &voxel,
        &voxel_standard,
        layer_map.as_ref(),
        lod0_step(&spec) as f32,
        chunk_grid(&spec).0,
    );
    if voxel_stats.meshed > 0 {
        info!(
            "terrain: {} voxel chunks meshed ({} proven uniform, {} empty)",
            voxel_stats.meshed, voxel_stats.skipped_uniform, voxel_stats.empty
        );
    }
    spawn_water(
        world,
        &mut meshes,
        &mut water_materials,
        &mut materials,
        &grid,
        &voxel,
        root,
        &pending.features,
        &result,
    );
    spawn_roads(
        world,
        &mut meshes,
        &mut materials,
        asset_server.as_ref(),
        root,
        &grid,
        &result,
        &mut watched,
    );

    world.insert_resource(PendingTerrainTextures { watched });
    world.insert_resource(meshes);
    world.insert_resource(materials);
    world.insert_resource(water_materials);
    world.insert_resource(chunk_materials);
    world.insert_resource(images);
    world.insert_resource(cliff_mask);
    world.insert_resource(TerrainChunkMaterials {
        layer: layer_map,
        standard: Some(chunk_standard),
    });
    world.insert_resource(TerrainRuntime {
        spec,
        grid: Arc::new(grid),
        water: result.water,
        roads: result.roads,
        pads: result.pads,
        voxel: Arc::new(voxel),
    });
}

/// LOD 0 grid step in whole meters (the chunk builder works on integer
/// steps); `resolution` finer than 1 m/vertex clamps to 1.
fn lod0_step(spec: &TerrainSpec) -> usize {
    let ideal = spec.chunk_size / spec.resolution.max(1) as f32;
    let step = ideal.round().max(1.0) as usize;
    if (spec.chunk_size / step as f32).abs().fract() > 1e-3 {
        1
    } else {
        step
    }
}

/// Chunk grid geometry shared by the chunk materials and the mesh spawner:
/// `(edge meters, rows per world side)`.
fn chunk_grid(spec: &TerrainSpec) -> (f32, u32) {
    let step = lod0_step(spec);
    let segments = (spec.chunk_size / step as f32).round().max(1.0) as usize;
    let edge = segments as f32 * step as f32;
    let rows = (spec.world_size / edge).ceil().max(1.0) as u32;
    (edge, rows)
}

/// Spawns the terrain's chunk meshes.
///
/// Each chunk is built at the LOD its distance from `camera_xz` implies — the
/// same selection [`super::plugin`] applies every frame — and chunks past
/// `render_distance` are left for the plugin to spawn on approach. Building
/// the whole field at LOD 0 is not viable: `simple-rpg` is a 4000 m world with
/// the default 64 m chunk, i.e. 3969 chunks, and at LOD 0 that is ~18 M
/// vertices of resident mesh — the host ran out of memory before the window
/// ever opened. Picking LODs up front keeps distant chunks at a few hundred
/// vertices each.
#[allow(clippy::too_many_arguments)]
fn spawn_chunks(
    world: &mut World,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: Option<&AssetServer>,
    parent: Entity,
    spec: &TerrainSpec,
    grid: &BrushGrid,
    camera_xz: Option<Vec2>,
    watched: &mut Vec<WatchedTexture>,
    layer_map: Option<&ChunkLayerMap>,
    cliff: Option<&super::cliffs::CliffMask>,
    voxel: &VoxelField,
) -> Handle<StandardMaterial> {
    let step = lod0_step(spec);
    let segments = (spec.chunk_size / step as f32).round() as usize;
    if segments == 0 {
        warn!("terrain chunk size is smaller than one grid step — no chunks");
        return materials.add(StandardMaterial::default());
    }
    let edge = segments as f32 * step as f32;
    let rows = (spec.world_size / edge).ceil().max(1.0) as u32;
    let epsilon = grid.texel();

    let mut material = StandardMaterial {
        // White: the authored `base-color` is already folded into the chunk
        // vertex colours by `tint_vertex_color`, and the PBR shader multiplies
        // those in. Putting it here as well would square it.
        base_color: Color::WHITE,
        metallic: 0.0,
        perceptual_roughness: 0.95,
        ..StandardMaterial::default()
    };
    let mut texture_handle = None;
    if let (Some(server), Some(texture)) = (asset_server, spec.texture.as_deref()) {
        let handle = load_world_texture(server, world, texture);
        texture_handle = Some(handle.clone());
        material.base_color_texture = Some(handle);
    }
    let terrain_material = materials.add(material);
    if let Some(texture) = texture_handle {
        watched.push(WatchedTexture::Standard {
            material: terrain_material.clone(),
            texture,
        });
    }
    // Com as camadas ativas o banding morre nas vertex colors (o splat
    // substitui o tint); no caminho legado ele continua inteiro.
    let chunk_tint = spec.chunk_tint();

    let half = spec.world_size * 0.5;
    let max_lod = super::plugin::max_lod_for(spec, edge);
    let margin = super::plugin::hysteresis_margin(spec);
    for cz in 0..rows {
        for cx in 0..rows {
            let origin = Vec3::new(-half + cx as f32 * edge, 0.0, -half + cz as f32 * edge);
            let center = Vec2::new(origin.x + edge * 0.5, origin.z + edge * 0.5);
            let distance = camera_xz.map(|cam| cam.distance(center));
            if let Some(distance) = distance {
                // The plugin spawns these at LOD 0 once the camera approaches.
                if distance > spec.effective_render_distance() {
                    continue;
                }
            }
            // A chunk any 3D feature reaches is not a heightfield chunk at
            // all: `spawn_voxel_chunks` covers it with surface nets. Skipping
            // it here is what keeps the two meshers from drawing the same
            // ground twice and z-fighting over it.
            if voxel.is_volumetric_chunk(origin.x, origin.z, edge) {
                continue;
            }
            // Without a camera every chunk is "near"; that only happens in
            // headless setups with no view, where LOD 0 is the safe answer.
            let lod = distance
                .map(|d| super::plugin::select_lod(d, spec.lod_distance(), 0, max_lod, margin))
                .unwrap_or(0);
            let mut built_lod = lod;
            let params = ChunkMeshParams {
                origin,
                size: edge,
                lod_step: step << lod,
                lod0_step: step,
                skirt_depth: spec.skirt_depth_meters(),
                normal_epsilon: epsilon,
                texture_tile_size: spec.texture_tile_size,
                levels: spec.levels,
                world_size: spec.world_size,
                tint: chunk_tint.clone(),
                cliff_angle: spec.cliff_angle,
            };
            // A LOD step that does not divide the chunk edge yields no mesh;
            // fall back to LOD 0, which always does.
            let data = match build_chunk_mesh(grid, &params, cliff) {
                Ok(Some(data)) => data,
                Ok(None) | Err(_) if lod > 0 => {
                    built_lod = 0;
                    let params = ChunkMeshParams {
                        lod_step: step,
                        ..params
                    };
                    match build_chunk_mesh(grid, &params, cliff) {
                        Ok(Some(data)) => data,
                        _ => continue,
                    }
                }
                Ok(None) => continue,
                Err(error) => {
                    warn!("chunk ({cx},{cz}) failed to build: {error:#}");
                    continue;
                }
            };
            let handle = meshes.add(to_bevy_mesh(&data));
            // Layer material of THIS chunk when the world opted in; the
            // legacy single-texture material covers chunks without one.
            let chunk_material = match layer_map.and_then(|map| map.get(cx, cz)) {
                Some(material) => ChunkMaterialHandle::Layer(material.clone()),
                None => ChunkMaterialHandle::Standard(terrain_material.clone()),
            };
            let mut chunk = world.spawn((
                Name::new(format!("chunk {cz}-{cx}")),
                // Mesh positions are chunk-center relative on XZ.
                Transform::from_translation(Vec3::new(
                    origin.x + edge * 0.5,
                    0.0,
                    origin.z + edge * 0.5,
                )),
                Visibility::Inherited,
                ChildOf(parent),
                super::plugin::TerrainChunk {
                    coords: UVec2::new(cx, cz),
                    lod: built_lod,
                    built_lod,
                },
            ));
            match chunk_material {
                ChunkMaterialHandle::Layer(material) => {
                    chunk.insert((Mesh3d(handle), MeshMaterial3d(material)));
                }
                ChunkMaterialHandle::Standard(material) => {
                    chunk.insert((Mesh3d(handle), MeshMaterial3d(material)));
                }
            }
        }
    }
    terrain_material
}

/// Builds the per-chunk layer materials for a world that opted in via
/// `layers`. Every slot resolves a pool alias (`grass` →
/// `/assets/textures/grass/albedo.ktx2`) or a raw texture path; slots past
/// the authored list reuse the grass texture — EXCETO o slot fixo do leito
/// ([`SLOT_RIVERBED`]): o splat pinta o leito em qualquer mundo com água,
/// por isso ele carrega sempre a textura `pebbles`.
///
/// Per chunk ([`generate_chunk_splats`]): the four pool textures with the
/// highest aggregate weight + the chunk's own RGBA splat plane. Chunks of a
/// mountain carry snow/stone, chunks of a swamp carry mud — different areas
/// render different layer sets, and the material stays at 5 textures + 1
/// uniform (the retired single 13-layer material bound 34 and crashed the
/// NVIDIA driver's pipeline-layout path).
#[allow(clippy::too_many_arguments)]
fn spawn_chunk_materials(
    spec: &TerrainSpec,
    grid: &BrushGrid,
    result: &FeatureResult,
    cliff: &crate::terrain::cliffs::CliffMask,
    server: &AssetServer,
    world: &mut World,
    images: &mut Assets<Image>,
    chunk_materials: &mut Assets<TerrainChunkMaterial>,
    watched: &mut Vec<WatchedTexture>,
    edge: f32,
    rows: u32,
) -> ChunkLayerMap {
    // `spec.layers` vem CANÓNICA do parse (`canonicalize_layers`: posição =
    // slot, buracos = ""). Colocar por slot e não por ordem escrita — sem
    // isto um subconjunto autoral lia a posição como índice de slot e pintava
    // texturas trocadas.
    let mut loaded: Vec<(usize, Handle<Image>)> = Vec::new();
    let mut first = None::<Handle<Image>>;
    for (slot, entry) in spec.layers.iter().enumerate().take(super::splat::LAYER_COUNT) {
        if entry.is_empty() {
            continue;
        }
        let path = pool_albedo(entry).unwrap_or_else(|| entry.clone());
        let handle = load_world_texture(server, world, &path);
        first.get_or_insert_with(|| handle.clone());
        loaded.push((slot, handle));
    }
    // Slots sem textura autoral leem a primeira layer (slot 0): um mundo de
    // 8 layers renderiza bem com os quatro picks mais pesados.
    let fallback = first.unwrap_or_else(|| images.add(solid_white_image()));
    let mut layer_textures = vec![fallback.clone(); super::splat::LAYER_COUNT];
    for (slot, handle) in loaded {
        layer_textures[slot] = handle;
    }
    // O leito é pintado pelo splat independentemente da lista autoral —
    // um mundo que não lista `pebbles` continua com fundo de seixo.
    if layer_textures[SLOT_RIVERBED] == fallback {
        let path = pool_albedo("pebbles").expect("pebbles is a DEFAULT_LAYERS alias");
        layer_textures[SLOT_RIVERBED] = load_world_texture(server, world, &path);
    }

    let params = SplatParams {
        shore_width: spec.shore_width,
        snow_height: spec.tint.snow_height,
        seed: spec.seed,
        texel: spec.splat_texel,
        biomes: spec.biomes.clone(),
    };
    let splats = generate_chunk_splats(
        grid,
        &result.water,
        &result.roads,
        &params,
        edge,
        rows,
        Some(cliff),
    );
    info!(
        "terrain chunk materials: {} chunks × 4 layers ({}² texel splats)",
        splats.len(),
        super::splat::CHUNK_SPLAT_TEXELS
    );

    let half = spec.world_size * 0.5;
    let mut materials = std::collections::HashMap::default();
    for (index, chunk_splat) in splats.iter().enumerate() {
        let cx = index as u32 % rows;
        let cz = index as u32 / rows;
        let origin = [-half + cx as f32 * edge, -half + cz as f32 * edge];
        let slots = chunk_splat.slots;
        let splat_handle = images.add(chunk_splat_image(chunk_splat));
        let params = TerrainChunkParams::from_slots(slots, origin, edge);
        let material = chunk_materials.add(TerrainChunkMaterial {
            layer0: layer_textures[slots[0]].clone(),
            layer1: layer_textures[slots[1]].clone(),
            layer2: layer_textures[slots[2]].clone(),
            layer3: layer_textures[slots[3]].clone(),
            splat: splat_handle,
            params,
        });
        // Watch EVERY layer of the material: a texture that never lands is
        // repointed instead of holding the material unprepared (and the
        // whole chunk invisible). The riverbed falls back to gravel when the
        // chunk carries it, everything else to the dominant layer — MAS
        // nunca para a textura vigiada própria: quando a falhada É a
        // dominante, apontar a ela própria caía no branco sólido e pintava
        // o chunk inteiro (fronteira reta contra os vizinhos vivos).
        for (slot_index, &pool_slot) in slots.iter().enumerate() {
            let texture = layer_textures[pool_slot].clone();
            let repoint = if pool_slot == SLOT_RIVERBED
                && slots.contains(&SLOT_GRAVEL)
                && layer_textures[SLOT_GRAVEL] != texture
            {
                layer_textures[SLOT_GRAVEL].clone()
            } else {
                slots
                    .iter()
                    .map(|&s| layer_textures[s].clone())
                    .find(|t| *t != texture)
                    .unwrap_or_else(|| texture.clone())
            };
            watched.push(WatchedTexture::Layer {
                material: material.clone(),
                slot: slot_index,
                texture,
                repoint,
            });
        }
        materials.insert((cx, cz), material);
    }
    ChunkLayerMap {
        rows,
        edge,
        materials,
    }
}

fn spawn_water(
    world: &mut World,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<WaterMaterial>,
    standard_materials: &mut Assets<StandardMaterial>,
    grid: &BrushGrid,
    voxel: &VoxelField,
    parent: Entity,
    features: &super::features::TerrainFeatures,
    result: &FeatureResult,
) {
    if result.water.is_empty() {
        return;
    }
    // A cor/alpha do corpo (e o fade de margem) chegam pelas VERTEX COLORS;
    // o shader da extensão (`shaders/water.wgsl`) acrescenta ondas, fresnel e
    // glint por cima deste PBR base.
    let water_material = materials.add(WaterMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            metallic: 0.0,
            perceptual_roughness: 0.08,
            reflectance: 0.5,
            alpha_mode: bevy::material::AlphaMode::Blend,
            cull_mode: None,
            ..StandardMaterial::default()
        },
        extension: WaterExtension {},
    });
    // Espuma ambiente: emissores ao longo da linha de água (um a cada
    // FOAM_SPACING m, cap por corpo) — a borda deixa de ser uma linha
    // morta entre o splat e a absorção.
    const FOAM_SPACING: f32 = 6.0;
    const FOAM_MAX_PER_BODY: usize = 40;
    let foam_spec = crate::recipes::ParticleSpec {
        preset: "foam".into(),
        emission_rate: Some(6.0),
        life: None,
        speed: None,
        size: None,
        color: None,
        shape_radius: Some(0.8),
        looping: true,
        world_space: false,
    };
    // Emparelha corpo ↔ spec por IDENTIDADE (water_specs), não por posição:
    // um lago/rio degenerado cujo carve falhou não entra em `result.water`
    // e desalinharia todos os seguintes (espelho de água do vizinho,
    // rio próprio nunca renderizado).
    for (body, &(is_lake, spec_i)) in result.water.iter().zip(&result.water_specs) {
        let (name, mesh) = if is_lake {
            let lake = &features.lakes[spec_i];
            ("lake", lake_water_mesh(lake, body.water_y))
        } else {
            let river = &features.rivers[spec_i];
            ("river", river_water_mesh(river, body))
        };
        if mesh.indices.is_empty() {
            continue;
        }
        let handle = meshes.add(to_bevy_mesh(&mesh));
        world
            .spawn((
                Name::new(format!("{name} {spec_i}")),
                Transform::default(),
                Visibility::Inherited,
                // Água transparente a projetar sombra pintava um anel escuro
                // na margem (o "contorno" que a lia como plataforma sólida).
                NotShadowCaster,
                ChildOf(parent),
            ))
            .insert((Mesh3d(handle), MeshMaterial3d(water_material.clone())));

        // ── Espuma da margem ─────────────────────────────────────────
        let foam_positions: Vec<(Vec2, f32)> = if is_lake {
            let lake = &features.lakes[spec_i];
            let phases = super::water::shape_phases(lake.at);
            let reach = (super::water::waterline_reach(lake.depth, lake.water_offset)
                * super::water::CARVE_MARGIN)
                .clamp(0.5, 1.6);
            let perimeter = 2.0 * std::f32::consts::PI * lake.radius * reach;
            let n = ((perimeter / FOAM_SPACING) as usize).clamp(4, FOAM_MAX_PER_BODY);
            (0..n)
                .map(|i| {
                    let theta = i as f32 / n as f32 * std::f32::consts::TAU;
                    let r = super::water::lake_shape_radius(lake.radius, theta, phases) * reach;
                    let p = lake.at + Vec2::new(theta.cos(), theta.sin()) * r;
                    (p, body.water_y)
                })
                .collect()
        } else {
            let step = (FOAM_SPACING / super::water::RIVER_STATION_SPACING)
                .round()
                .max(1.0) as usize;
            body.stations
                .iter()
                .enumerate()
                .step_by(step)
                .take(FOAM_MAX_PER_BODY)
                .filter_map(|(i, st)| body.surface_y.get(i).map(|&y| (*st, y)))
                .collect()
        };
        for (p, y) in foam_positions {
            // Margens voxel: um emissor dentro da parede sólida nunca é
            // visto — consulta o campo (a lâmina + 10 cm tem de ser ar).
            if voxel.density(grid, Vec3::new(p.x, y + 0.1, p.y)) < 0.0 {
                continue;
            }
            crate::particles::spawn_looping(
                world,
                meshes,
                standard_materials,
                &foam_spec,
                Vec3::new(p.x, y + 0.03, p.y),
            );
        }

        // Névoa de cascata (na lâmina do caldeirão) e boca da nascente.
        if !is_lake {
            let river = &features.rivers[spec_i];
            let mist_spec = crate::recipes::ParticleSpec {
                preset: "mist".into(),
                emission_rate: Some(9.0),
                shape_radius: Some(0.9),
                ..foam_spec.clone()
            };
            let mut mist_at = Vec::new();
            for &lip in &body.cascades {
                if let (Some(st), Some(&y)) = (
                    body.stations.get(lip + 1),
                    body.surface_y.get(lip + 1),
                ) {
                    mist_at.push((*st, y));
                }
            }
            if river.spring && let Some(st) = body.stations.first() {
                mist_at.push((*st, body.surface_y.first().copied().unwrap_or(body.water_y)));
            }
            for (p, y) in mist_at {
                if voxel.density(grid, Vec3::new(p.x, y + 0.2, p.y)) < 0.0 {
                    continue;
                }
                crate::particles::spawn_looping(
                    world,
                    meshes,
                    standard_materials,
                    &mist_spec,
                    Vec3::new(p.x, y + 0.05, p.y),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_roads(
    world: &mut World,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: Option<&AssetServer>,
    parent: Entity,
    grid: &BrushGrid,
    result: &FeatureResult,
    watched: &mut Vec<WatchedTexture>,
) {
    for (i, (path, spec)) in result.roads.iter().zip(&result.road_specs).enumerate() {
        let mesh = super::roads::road_ribbon_mesh(grid, path, spec);
        if mesh.indices.is_empty() {
            continue;
        }
        let mut material = StandardMaterial {
            base_color: Color::srgb(0.62, 0.60, 0.56), // stone fallback
            metallic: 0.0,
            perceptual_roughness: 0.9,
            alpha_mode: bevy::material::AlphaMode::Blend,
            ..StandardMaterial::default()
        };
        let mut texture_handle = None;
        if let (Some(server), Some(texture)) = (asset_server, spec.texture.as_deref()) {
            let image = load_world_texture(server, world, texture);
            texture_handle = Some(image.clone());
            material.base_color_texture = Some(image);
        }
        let handle = materials.add(material);
        if let Some(texture) = texture_handle {
            watched.push(WatchedTexture::Standard {
                material: handle.clone(),
                texture,
            });
        }
        let mesh_handle = meshes.add(to_bevy_mesh(&mesh));
        world
            .spawn((
                Name::new(path.name.clone().unwrap_or_else(|| format!("road {i}"))),
                Transform::default(),
                Visibility::Inherited,
                // estrada não projeta sombra dura — a sombra própria
                // escurecia as bordas na relva (polish loop 6)
                NotShadowCaster,
                ChildOf(parent),
            ))
            .insert((Mesh3d(mesh_handle), MeshMaterial3d(handle)));
    }

    // Fusion discs (VibeGame junctions.ts): círculo opaco nas junções multi-braço
    // cobrindo as costuras das ribbons.
    for (i, junction) in result.road_junctions.iter().enumerate() {
        let mesh = super::roads::junction_disc_mesh(grid, junction);
        if mesh.indices.is_empty() {
            continue;
        }
        let mut material = StandardMaterial {
            base_color: Color::srgb(0.62, 0.60, 0.56),
            metallic: 0.0,
            perceptual_roughness: 0.9,
            alpha_mode: bevy::material::AlphaMode::Blend,
            ..StandardMaterial::default()
        };
        let mut texture_handle = None;
        if let (Some(server), Some(texture)) = (asset_server, junction.texture.as_deref()) {
            let image = load_world_texture(server, world, texture);
            texture_handle = Some(image.clone());
            material.base_color_texture = Some(image);
        }
        let handle = materials.add(material);
        if let Some(texture) = texture_handle {
            watched.push(WatchedTexture::Standard {
                material: handle.clone(),
                texture,
            });
        }
        let mesh_handle = meshes.add(to_bevy_mesh(&mesh));
        world
            .spawn((
                Name::new(format!("junction {i}")),
                Transform::default(),
                Visibility::Inherited,
                // Um decal transparente a 10 cm do chão que projecta sombra
                // desenha a sua própria silhueta de volta no chão: era isso
                // que dava o anel escuro à volta do disco do portão norte e
                // o fazia ler como um objecto pousado na relva.
                NotShadowCaster,
                ChildOf(parent),
            ))
            .insert((Mesh3d(mesh_handle), MeshMaterial3d(handle)));
    }

    // Ground decals (`<GroundDecal>`): plaza floors, market aprons — the
    // draped, feathered replacement for hard-edged `<Plane>` decals.
    for (i, dec) in result.decals.iter().enumerate() {
        let mesh = super::decal::ground_decal_mesh(grid, dec);
        if mesh.indices.is_empty() {
            continue;
        }
        let mut material = StandardMaterial {
            base_color: Color::srgb(dec.base_color[0], dec.base_color[1], dec.base_color[2]),
            metallic: 0.0,
            perceptual_roughness: dec.roughness,
            alpha_mode: bevy::material::AlphaMode::Blend,
            ..StandardMaterial::default()
        };
        let mut texture_handle = None;
        if let (Some(server), Some(texture)) = (asset_server, dec.texture.as_deref()) {
            let image = load_world_texture(server, world, texture);
            texture_handle = Some(image.clone());
            material.base_color_texture = Some(image);
        }
        let handle = materials.add(material);
        if let Some(texture) = texture_handle {
            watched.push(WatchedTexture::Standard {
                material: handle.clone(),
                texture,
            });
        }
        let mesh_handle = meshes.add(to_bevy_mesh(&mesh));
        world
            .spawn((
                Name::new(
                    dec.name
                        .clone()
                        .unwrap_or_else(|| format!("ground decal {i}")),
                ),
                Transform::default(),
                Visibility::Inherited,
                NotShadowCaster,
                ChildOf(parent),
            ))
            .insert((Mesh3d(mesh_handle), MeshMaterial3d(handle)));
    }
}

/// Converts pure [`super::mesh::ChunkMeshData`] buffers into a Bevy mesh
/// (CPU-resident, so tests/tools can inspect it).
pub(crate) fn to_bevy_mesh(data: &super::mesh::ChunkMeshData) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, data.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs.clone());
    // Only surfaces that actually use vertex colours get the attribute:
    // terrain chunks carry the height/slope tint, roads their edge alpha.
    if !data.colors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, data.colors.clone());
    }
    mesh.insert_indices(Indices::U32(data.indices.clone()));
    mesh
}

/// Loads a PNG heightmap (8-bit grayscale is upscaled to the full 16-bit
/// range like the VibeGame loader). Relative paths resolve against the world
/// XML directory first, then the process CWD.
/// Loaded heightmap plus the world size / max height its own metadata
/// declares. These only fill the spec when the world XML did not state them
/// (`TerrainSpec::extent_authored`); an authored extent always wins.
pub struct LoadedHeightmap {
    pub map: HeightMapU16,
    pub world_size: Option<f32>,
    pub max_height: Option<f32>,
    /// Campo de biomas declarado pelo `.ahgt` (vazio para PNG / sem bloco).
    pub biomes: crate::terrain::splat::BiomeField,
}

/// Teto de dimensão para heightmaps PNG — 16 k² por eixo cobre qualquer
/// mundo declarável e o `to_luma16` duplica a memória do decode.
const MAX_HM_IMAGE_EDGE: usize = 16384;

fn load_heightmap(base_dir: Option<&Path>, path: &str) -> anyhow::Result<LoadedHeightmap> {
    // `/assets/…`-style paths are site-root relative: resolve against the
    // world dir (the folder that contains `assets/`).
    let rel = path.trim_start_matches('/');
    let resolved = match base_dir {
        Some(dir) => {
            let candidate = dir.join(rel);
            if candidate.exists() {
                candidate
            } else {
                PathBuf::from(path)
            }
        }
        None => PathBuf::from(path),
    };
    let bytes =
        std::fs::read(&resolved).map_err(|e| anyhow::anyhow!("{}: {e}", resolved.display()))?;
    if path.to_ascii_lowercase().ends_with(".ahgt") {
        let (map, meta) = HeightMapU16::from_ahgt(&bytes)
            .map_err(|e| anyhow::anyhow!("{}: {e}", resolved.display()))?;
        return Ok(LoadedHeightmap {
            map,
            world_size: meta.world_size,
            max_height: meta.max_height,
            biomes: meta.biomes,
        });
    }
    let img = image::load_from_memory(&bytes)
        .map_err(|e| anyhow::anyhow!("{}: {e}", resolved.display()))?;
    let (width, depth) = (img.width() as usize, img.height() as usize);
    if width > MAX_HM_IMAGE_EDGE || depth > MAX_HM_IMAGE_EDGE {
        anyhow::bail!(
            "{}: {width}x{depth} above the {MAX_HM_IMAGE_EDGE}px heightmap cap",
            resolved.display()
        );
    }
    let data = img.to_luma16().into_raw();
    Ok(LoadedHeightmap {
        map: HeightMapU16 { width, depth, data },
        world_size: None,
        max_height: None,
        biomes: crate::terrain::splat::BiomeField::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::spec::TerrainSpec;

    #[test]
    fn test_lod0_step_matches_chunk_division() {
        let mut spec = TerrainSpec {
            resolution: 64, // 64 m chunks
            ..Default::default()
        };
        assert_eq!(lod0_step(&spec), 1);
        spec.resolution = 32;
        assert_eq!(lod0_step(&spec), 2);
        spec.resolution = 128; // finer than 1 m: clamps to the 1 m step
        assert_eq!(lod0_step(&spec), 1);
        spec.chunk_size = 50.0;
        spec.resolution = 20;
        assert_eq!(lod0_step(&spec), 1, "50/2.5 is not an exact meter step");
    }

    #[test]
    fn test_sample_mesh_surface_matches_rendered_lattice() {
        // Peak flank h(x) = 40 − (x−7.5)²/8 on a 1 m/texel grid (16 texels
        // over 15 m): the invariant that matters is the RENDERED surface —
        // chunk meshes draw flat triangles between vertices, so between two
        // vertices sample_mesh_surface must equal the linear chord of the
        // vertex heights (where the analytic sample may differ; anchoring
        // props to the chord is what keeps them flush with the mesh). On the
        // vertices both agree exactly.
        let max_h = 65.535;
        let height = |x: usize| 40.0 - (x as f32 - 7.5).powi(2) / 8.0;
        let raw: Vec<u16> = (0..16usize)
            .flat_map(|z| (0..16usize).map(move |x| (x, z)))
            .map(|(x, _)| (height(x) / max_h * 65535.0).round() as u16)
            .collect();
        let grid = BrushGrid::new(raw, 16, 16, 15.0, max_h, 1.0).expect("grid");
        let runtime = TerrainRuntime {
            spec: TerrainSpec {
                world_size: 15.0,
                resolution: 64, // chunk 64 m / 64 → lod0 step 1 m
                ..Default::default()
            },
            grid: Arc::new(grid),
            water: Vec::new(),
            roads: Vec::new(),
            pads: Vec::new(),
            voxel: Arc::new(VoxelField::default()),
        };
        for i in 0..16 {
            let x = i as f32 - 7.5;
            let mesh = runtime.sample_mesh_surface(x, 0.0);
            let analytic = runtime.sample(x, 0.0);
            assert!(
                (mesh - analytic).abs() < 1e-2,
                "lattice vertex {x}: mesh {mesh} vs analytic {analytic}"
            );
        }
        // Mid-cell: exactly the chord between the two vertex heights, even
        // where the smoothed analytic sample disagrees.
        let mesh = runtime.sample_mesh_surface(-7.0, 0.0);
        let chord = (runtime.sample(-7.5, 0.0) + runtime.sample(-6.5, 0.0)) * 0.5;
        assert!(
            (mesh - chord).abs() < 1e-3,
            "mid-cell {mesh} must be the vertex chord {chord}"
        );
    }

    /// End-to-end smoke: PendingTerrain (procedural terrain + all features)
    /// through the exclusive bootstrap — chunks, water and road entities land
    /// with registries, fully headless.
    #[test]
    fn test_bootstrap_builds_a_carved_world_headless() {
        use crate::recipes::spawn::PendingTerrain;
        use bevy::math::Vec2;

        let mut app = bevy::app::App::new();
        app.add_plugins(bevy::MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>();

        let features = super::super::features::TerrainFeatures {
            decals: Vec::new(),
            caves: Vec::new(),
            arches: Vec::new(),
            pads: vec![crate::terrain::TerrainPadSpec {
                at: Vec2::ZERO,
                size: Vec2::splat(24.0),
                falloff: 8.0,
                corner_radius: 4.0,
                height: None,
            }],
            lakes: vec![crate::terrain::LakeSpec {
                at: Vec2::new(-30.0, 30.0),
                radius: 10.0,
                ..crate::terrain::LakeSpec::default()
            }],
            rivers: vec![crate::terrain::RiverSpec {
                path: vec![Vec2::new(10.0, -40.0), Vec2::new(50.0, -40.0)],
                ..crate::terrain::RiverSpec::default()
            }],
            cliffs: Vec::new(),
            roads: vec![],
            networks: vec![crate::terrain::RoadNetworkSpec {
                ways: vec![
                    crate::terrain::WaySpec {
                        id: "a".into(),
                        at: Vec2::new(-30.0, 20.0),
                        width: None,
                    },
                    crate::terrain::WaySpec {
                        id: "b".into(),
                        at: Vec2::new(30.0, 20.0),
                        width: None,
                    },
                ],
                segments: vec![crate::terrain::SegmentSpec {
                    a: "a".into(),
                    b: "b".into(),
                    via: Vec::new(),
                    width: None,
                    profile: None,
                }],
                ..crate::terrain::RoadNetworkSpec::default()
            }],
        };
        app.insert_resource(PendingTerrain {
            base_dir: None,
            terrain: Some(TerrainSpec {
                world_size: 128.0,
                max_height: 40.0,
                chunk_size: 64.0,
                seed: 3,
                ..TerrainSpec::default()
            }),
            features,
        });

        app.add_systems(bevy::app::Startup, bootstrap);
        app.update(); // MinimalPlugins Startup runs on the first update

        let runtime = app
            .world()
            .get_resource::<TerrainRuntime>()
            .expect("terrain built");
        assert_eq!(runtime.water.len(), 2, "lake + river registered");
        assert_eq!(runtime.roads.len(), 1, "network expanded into a road");
        assert_eq!(runtime.pads.len(), 1);
        // Pad core is flat.
        assert!((runtime.grid.sample(0.0, 0.0) - runtime.pads[0].height).abs() < 0.05);
        // Lake bowl is below its mirror.
        assert!(runtime.sample(-30.0, 30.0) < runtime.water[0].water_y);
        assert!(runtime.in_water(-30.0, 30.0));
        assert!(runtime.on_road(0.0, 20.0));
        assert!(!runtime.on_road(0.0, 60.0));

        // Entities: terrain root + chunks + lake/river/road visuals.
        let world = app.world_mut();
        let names: Vec<String> = world
            .query::<&Name>()
            .iter(world)
            .map(|n| n.to_string())
            .collect();
        assert!(names.iter().any(|n| n.starts_with("chunk ")), "{names:?}");
        assert!(names.iter().any(|n| n == "lake 0"), "{names:?}");
        assert!(names.iter().any(|n| n == "river 0"), "{names:?}");
        assert!(names.iter().any(|n| n.contains("net/a-b")), "{names:?}");
    }
}

//! Terrain chunk lifecycle — the core LOD plugin.
//!
//! Complements [`super::runtime::TerrainFeaturesPlugin`] (which bootstraps
//! the carved world and spawns LOD-0 chunk meshes at startup): this plugin
//! owns the **runtime** half of the chunk lifecycle:
//!
//! 1. **Adopt** — one-shot scan of the terrain root for the `chunk {cz}-{cx}`
//!    meshes spawned by the bootstrap, tagging them with [`TerrainChunk`].
//! 2. **Select** — LOD per chunk from the camera distance: the flat
//!    `2^lod`-step scheme of `bevy_mesh_terrain`, corrected with the VibeGame
//!    hysteresis + camera-move reselect gate so crossing a boundary never
//!    thrashes rebuilds.
//! 3. **Rebuild** — meshes rebuild inline with a per-frame budget
//!    ([`super::spec::DEFAULT_MAX_MESH_BUILDS_PER_FRAME`]). Deliberately *no*
//!    async tasks: the upstream crate lost chunks forever to a poll-once
//!    orphan-task bug (`finish_chunk_build_tasks`); small heightfield grids
//!    build in well under a frame, so inline + budget eliminates the whole
//!    bug class.
//! 4. **Cull** — chunks beyond `render_distance` (measured to the chunk's XZ
//!    AABB, not its center) are despawned and respawned — at the LOD their
//!    distance implies — when the camera approaches, matching VibeGame
//!    behavior.
//!
//! Works headless: only `Assets<Mesh>` and the camera transform are needed;
//! `analyze` never runs this (no Update schedule).

use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;

use bevy::prelude::*;
use super::runtime::{ChunkMaterialHandle, TerrainChunkMaterials, TerrainRuntime};
use super::spec::{DEFAULT_LOD_HYSTERESIS, DEFAULT_LOD_RESELECT_DISTANCE, TerrainSpec};
use crate::profiler::{Group, timed};

/// Tag on every terrain chunk mesh entity managed by this plugin.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct TerrainChunk {
    /// Chunk grid coordinates (x, z), 0-based from the world's -X/-Z corner.
    pub coords: UVec2,
    /// LOD the chunk *should* render at (selection output).
    pub lod: u8,
    /// LOD of the mesh currently attached (rebuild output).
    pub built_lod: u8,
}

/// Per-terrain tracking state for the LOD plugin.
#[derive(Resource, Debug, Default)]
pub struct ChunkLodState {
    /// `true` after the one-shot adopt scan.
    adopted: bool,
    /// Terrain root entity (chunks attach here on respawn).
    root: Option<Entity>,
    /// Material captured from an adopted chunk, reused for respawns. Worlds
    /// that use the layer blend publish their (typed) handle via
    /// [`TerrainChunkMaterials`], which wins over this capture.
    material: Option<Handle<StandardMaterial>>,
    /// Camera XZ at the last full LOD evaluation (reselect gate).
    last_cam: Option<Vec2>,
    /// Work remained when the frame budget ran out (keeps draining).
    pending: bool,
    /// Live chunk index by grid coords (despawn removes the entry).
    chunks: HashMap<UVec2, Entity>,
}

impl ChunkLodState {
    /// Number of chunks currently tracked.
    pub fn tracked_chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Entity of the chunk at `coords`, if tracked.
    pub fn chunk_entity(&self, coords: UVec2) -> Option<Entity> {
        self.chunks.get(&coords).copied()
    }
}

/// Terrain chunk LOD plugin — see module docs.
///
/// Does **not** register [`super::runtime::TerrainFeaturesPlugin`]; the CLI
/// wires both explicitly (double registration would panic).
#[derive(Default)]
pub struct TerrainPlugin;

/// Sets do terreno, para quem precisa de correr DEPOIS da grelha de colunas.
///
/// O `timed(...)` embrulha o sistema noutro tipo, por isso um
/// `.after(update_voxel_columns)` de fora não casa — a ordenação passa por
/// este set explícito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum TerrainSet {
    /// Spawn/refino/cull das colunas voxel (`update_voxel_columns`).
    Columns,
}

impl bevy::app::Plugin for TerrainPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_resource::<ChunkLodState>().add_systems(
            bevy::app::Update,
            (
                timed(Group::Terrain, adopt_chunks),
                timed(Group::Terrain, update_voxel_columns),
            )
                .in_set(TerrainSet::Columns),
        );
    }
}

/// Name prefix used by the bootstrap for chunk entities (`chunk {cz}-{cx}`).
const CHUNK_NAME_PREFIX: &str = "chunk ";

/// Entity name for a chunk, mirroring the bootstrap's naming scheme.
pub fn chunk_name(coords: UVec2) -> String {
    format!("chunk {}-{}", coords.y, coords.x)
}

/// Parses `chunk {cz}-{cx}` into `(cx, cz)` grid coords.
fn parse_chunk_name(name: &str) -> Option<UVec2> {
    let rest = name.strip_prefix(CHUNK_NAME_PREFIX)?;
    let (cz, cx) = rest.split_once('-')?;
    let (cx, cz) = (cx.trim().parse().ok()?, cz.trim().parse().ok()?);
    Some(UVec2::new(cx, cz))
}

/// One-shot: tag the bootstrap's chunk entities with [`TerrainChunk`], capture
/// the terrain root and the shared chunk material.
///
/// A coluna voxel é um grupo nomeado cujas caixas têm o mesh — a coluna já
/// nasce etiquetada pelo bootstrap, mas testes e respawn contam com a captura
/// do material a partir das caixas.
fn adopt_chunks(
    mut state: ResMut<ChunkLodState>,
    roots: Query<(Entity, &Name, &Children)>,
    tagged: Query<(), With<TerrainChunk>>,
    box_materials: Query<&MeshMaterial3d<StandardMaterial>, With<super::voxel::VoxelChunk>>,
    named: Query<&Name>,
    columns: Query<&Children>,
    mut commands: Commands,
) {
    if state.adopted {
        return;
    }
    state.adopted = true;
    for (root, name, children) in &roots {
        if name.as_str() != "terrain" {
            continue;
        }
        state.root = Some(root);
        for child in children.iter() {
            // Voxel column: a named group; the material comes from any box.
            let Ok(child_name) = named.get(child) else {
                continue;
            };
            let Some(coords) = parse_chunk_name(child_name.as_str()) else {
                continue;
            };
            if let Ok(kids) = columns.get(child) {
                for kid in kids.iter() {
                    if let Ok(mat) = box_materials.get(kid) {
                        state.material.get_or_insert(mat.0.clone());
                    }
                }
            }
            if tagged.get(child).is_ok() {
                continue;
            }
            commands.entity(child).insert(TerrainChunk {
                coords,
                lod: 0,
                built_lod: 0,
            });
        }
    }
}

// ── Ladder de colunas voxel (o caminho default da migração volumétrica) ─────

/// Caixas meshadas por frame no caminho voxel. Uma caixa 32³ custa ~39 k
/// amostras do campo (~2-5 ms) — o mesmo espaço que os 4 builds de chunk do
/// heightfield, contado em CAIXAS para uma coluna LOD-0 complete em um ou
/// dois passes em vez de hitchar um frame único.
const VOXEL_MAX_MESH_BUILDS_PER_FRAME: u32 = 4;

/// Estado de construção de uma coluna a meio de uma troca de LOD: as caixas
/// do `target` nascem escondidas (`staged`) sob o orçamento do frame; quando
/// `remaining` esvazia, as caixas velhas morrem e as staged ficam visíveis —
/// a troca é atómica (nunca dois LODs do mesmo chão desenhados ao mesmo
/// tempo, nem buraco durante o rebuild).
#[derive(Component)]
struct ColumnBuild {
    target: u8,
    staged: Vec<Entity>,
    remaining: Vec<super::voxel::VoxelBoxSpec>,
}

/// Per-frame LOD pass over VOXEL columns: same select/hysteresis/budget/cull
/// machinery as the heightfield pass, but a "rebuild" swaps a whole stack of
/// surface-nets boxes, staged hidden under the frame budget.
#[allow(clippy::too_many_arguments)]
fn update_voxel_columns(
    mut state: ResMut<ChunkLodState>,
    runtime: Option<Res<TerrainRuntime>>,
    published: Option<Res<TerrainChunkMaterials>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut columns: Query<(Entity, &mut TerrainChunk, Option<&mut ColumnBuild>)>,
    boxes: Query<(), With<super::voxel::VoxelChunk>>,
    children_q: Query<&Children>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if !state.adopted {
        return;
    }
    let Ok(cam) = cameras.single() else {
        return;
    };
    let cam_xz = Vec2::new(cam.translation().x, cam.translation().z);

    // Reselect gate — idêntico ao caminho heightfield.
    let moved = state
        .last_cam
        .is_none_or(|last| last.distance(cam_xz) > DEFAULT_LOD_RESELECT_DISTANCE);
    if !moved && !state.pending {
        return;
    }
    if moved {
        state.last_cam = Some(cam_xz);
    }

    // Sync the chunk index with reality (adoptions and despawns).
    let dead: Vec<UVec2> = state
        .chunks
        .iter()
        .filter(|(_, e)| columns.get(**e).is_err())
        .map(|(c, _)| *c)
        .collect();
    for coords in dead {
        state.chunks.remove(&coords);
    }
    for (entity, chunk, _) in columns.iter() {
        state.chunks.insert(chunk.coords, entity);
    }

    let mut budget = VOXEL_MAX_MESH_BUILDS_PER_FRAME;
    state.pending = false;

    let spec = &runtime.spec;
    let edge = chunk_edge(spec);
    let half = spec.world_size * 0.5;
    let max_lod = max_lod_for(spec, edge);
    let margin = hysteresis_margin(spec);
    let render_distance = spec.effective_render_distance();
    let lod0_cell = lod0_step(spec) as f32;
    let grid = &runtime.grid;
    let voxel = &runtime.voxel;
    // Captura ANTES do loop de colunas (o índice usa `state` mutável).
    let standard = state.material.clone();

    let center_of = |coords: UVec2| {
        Vec2::new(
            -half + coords.x as f32 * edge + edge * 0.5,
            -half + coords.y as f32 * edge + edge * 0.5,
        )
    };

    // 1. Cull + re-select + construção staged.
    for (entity, mut chunk, mut build) in columns.iter_mut() {
        let dist = center_of(chunk.coords).distance(cam_xz);
        if chunk_aabb_distance(dist, edge) > render_distance {
            state.chunks.remove(&chunk.coords);
            // Despawn recursivo: as caixas da coluna morrem com ela.
            commands.entity(entity).despawn();
            continue;
        }
        chunk.lod = select_lod(dist, spec.lod_distance(), chunk.built_lod, max_lod, margin);
        match build.as_mut() {
            // O alvo mudou a meio da construção: as staged (escondidas)
            // morrem e a fila recomputa para o novo alvo.
            Some(b) if b.target != chunk.lod => {
                for e in b.staged.drain(..) {
                    commands.entity(e).despawn();
                }
                b.remaining = super::voxel::column_boxes(
                    spec, grid, voxel, edge, lod0_cell, chunk.lod, chunk.coords,
                );
                b.target = chunk.lod;
                state.pending = true;
            }
            Some(b) => {
                let material = published
                    .as_ref()
                    .and_then(|p| p.layer.as_ref())
                    .and_then(|m| m.get(chunk.coords.x, chunk.coords.y).cloned())
                    .map(ChunkMaterialHandle::Layer)
                    .or_else(|| standard.clone().map(ChunkMaterialHandle::Standard));
                let Some(material) = material else {
                    continue;
                };
                while budget > 0 {
                    let Some(box_spec) = b.remaining.pop() else {
                        break;
                    };
                    let Some(data) =
                        super::voxel::build_box_mesh(spec, grid, voxel, &box_spec)
                    else {
                        // Provou-se vazio depois de tudo — sem custo de
                        // orçamento, segue para a próxima caixa.
                        continue;
                    };
                    b.staged.push(super::voxel::spawn_box_entity(
                        &mut commands,
                        &mut meshes,
                        entity,
                        &box_spec,
                        data,
                        &material,
                        false,
                    ));
                    budget -= 1;
                }
                if b.remaining.is_empty() {
                    // Swap atómico: caixas do LOD velho morrem, staged ficam
                    // visíveis, a coluna aponta o LOD construído.
                    if let Ok(kids) = children_q.get(entity) {
                        for kid in kids.iter() {
                            if boxes.get(kid).is_ok() && !b.staged.contains(&kid) {
                                commands.entity(kid).despawn();
                            }
                        }
                    }
                    for e in &b.staged {
                        commands.entity(*e).insert(Visibility::Inherited);
                    }
                    chunk.built_lod = b.target;
                    commands.entity(entity).remove::<ColumnBuild>();
                }
            }
            None if chunk.lod != chunk.built_lod => {
                let remaining = super::voxel::column_boxes(
                    spec, grid, voxel, edge, lod0_cell, chunk.lod, chunk.coords,
                );
                commands.entity(entity).insert(ColumnBuild {
                    target: chunk.lod,
                    staged: Vec::new(),
                    remaining,
                });
                // O componente entra por commands (deferido) — sem isto o
                // passe seguinte nem corria (gate de movimento de câmara).
                state.pending = true;
            }
            None => {}
        }
    }
    if budget == 0 {
        state.pending = true;
    }

    // 2. Respawn de colunas em falta (culled antes, ou fora do raio no
    //    bootstrap) — sem skip volumétrico: TODA a grelha é voxel agora.
    let Some(root) = state.root else {
        return;
    };
    if published.as_ref().and_then(|p| p.layer.as_ref()).is_none() && standard.is_none() {
        return;
    }
    let rows = (spec.world_size / edge).ceil().max(1.0) as u32;
    let clamp_coord = |axis: f32| -> u32 {
        (((axis - render_distance + half) / edge).floor().max(0.0) as u32).min(rows - 1)
    };
    let clamp_hi = |axis: f32| -> u32 {
        (((axis + render_distance + half) / edge).floor().max(0.0) as u32).min(rows - 1)
    };
    let min_cx = clamp_coord(cam_xz.x);
    let max_cx = clamp_hi(cam_xz.x);
    let min_cz = clamp_coord(cam_xz.y);
    let max_cz = clamp_hi(cam_xz.y);
    for cz in min_cz..=max_cz {
        for cx in min_cx..=max_cx {
            if budget == 0 {
                state.pending = true;
                return;
            }
            let coords = UVec2::new(cx, cz);
            if state.chunks.contains_key(&coords) {
                continue;
            }
            let dist = center_of(coords).distance(cam_xz);
            if chunk_aabb_distance(dist, edge) > render_distance {
                continue;
            }
            let Some(material) = published
                .as_ref()
                .and_then(|p| p.layer.as_ref())
                .and_then(|m| m.get(cx, cz).cloned())
                .map(ChunkMaterialHandle::Layer)
                .or_else(|| standard.clone().map(ChunkMaterialHandle::Standard))
            else {
                continue;
            };
            // Respawn já no LOD cru da distância (sem histerese — coluna nova
            // não tem histórico). As caixas da coluna constroem TODAS neste
            // frame: um chão a meio é pior do que ultrapassar o orçamento
            // uma vez; o `saturating_sub` limita a uma coluna destes por
            // frame.
            let lod = raw_lod(dist, spec.lod_distance(), max_lod);
            let column_boxes = super::voxel::column_boxes(
                spec, grid, voxel, edge, lod0_cell, lod, coords,
            );
            budget = budget.saturating_sub(column_boxes.len() as u32);
            let (entity, built) = super::voxel::spawn_column(
                &mut commands,
                &mut meshes,
                root,
                coords,
                lod,
                &material,
                &column_boxes,
                grid,
                voxel,
                spec,
            );
            if built > 0 {
                state.chunks.insert(coords, entity);
            } else {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Distância da câmara à AABB XZ do chunk: `max(0, dist_centro − meia-edge)`.
/// Chunks são quadrados axis-aligned — 0 com a câmara dentro do quadrado;
/// fora, a folga até à borda mais próxima. Cull e respawn partilham a
/// métrica para nenhum dos dois ignorar um chunk cujo corpo ainda cabe no
/// raio de render.
fn chunk_aabb_distance(dist_center: f32, edge: f32) -> f32 {
    (dist_center - edge * 0.5).max(0.0)
}

/// LOD cru para uma distância — o esquema plano `2^lod` de
/// `bevy_mesh_terrain`, antes da histerese. O passe de seleção alimenta com
/// isto a decisão de troca; o respawn usa-o diretamente (um chunk novo não
/// tem histórico para a histerese decidir).
pub(crate) fn raw_lod(dist: f32, lod_distance: f32, max_lod: u8) -> u8 {
    if lod_distance <= 0.0 || !dist.is_finite() {
        return 0;
    }
    (((dist / lod_distance).log2() + 1.0)
        .floor()
        .clamp(0.0, f32::from(max_lod))) as u8
}

/// LOD selection with hysteresis: boundaries sit at `lod_distance * 2^(l-1)`;
/// coarsening requires crossing `boundary * margin`, refining
/// `boundary / margin`.
pub(crate) fn select_lod(
    dist: f32,
    lod_distance: f32,
    current: u8,
    max_lod: u8,
    margin: f32,
) -> u8 {
    let raw = raw_lod(dist, lod_distance, max_lod);
    if raw == current {
        return current;
    }
    let margin = if margin.is_finite() && margin >= 1.0 {
        margin
    } else {
        1.0
    };
    if raw > current {
        let boundary = lod_distance * 2f32.powi(i32::from(current));
        if dist > boundary * margin {
            raw
        } else {
            current
        }
    } else {
        // A fronteira de refine é o TOPO da banda do LOD alvo (ld·2^raw):
        // com a base (raw−1) o chunk só refinava a metade da distância e
        // toda a aproximação da câmara renderizava um nível mais grossário.
        let boundary = lod_distance * 2f32.powi(i32::from(raw));
        if dist < boundary / margin {
            raw
        } else {
            current
        }
    }
}

/// `sqrt(hysteresis)` — a factor 1.2 shifts each boundary by ~±9.5%, giving
/// symmetric dead-zones around it.
pub(crate) fn hysteresis_margin(spec: &TerrainSpec) -> f32 {
    let h = if spec.lod_hysteresis >= 1.0 {
        spec.lod_hysteresis
    } else {
        DEFAULT_LOD_HYSTERESIS
    };
    h.sqrt()
}

/// Coarsest LOD whose integer grid step still divides the chunk edge evenly.
pub(crate) fn max_lod_for(spec: &TerrainSpec, edge: f32) -> u8 {
    let base_segments = (edge / lod0_step(spec) as f32).round() as u32;
    let mut lod = 0u8;
    while lod + 1 < spec.levels && base_segments.is_multiple_of(1 << (lod + 1)) {
        lod += 1;
    }
    lod
}

/// Full chunk edge in meters (the bootstrap's `edge`, which falls back to
/// whole meters when `resolution` does not divide `chunk_size` exactly).
pub(crate) fn chunk_edge(spec: &TerrainSpec) -> f32 {
    let step = lod0_step(spec);
    (spec.chunk_size / step as f32).round().max(1.0) * step as f32
}

/// LOD-0 grid step — mirrors `runtime::lod0_step` (kept local to avoid
/// touching the features module; a `pub(crate)` dedupe is a follow-up).
pub(crate) fn lod0_step(spec: &TerrainSpec) -> usize {
    let ideal = spec.chunk_size / spec.resolution.max(1) as f32;
    let step = ideal.round().max(1.0) as usize;
    if (spec.chunk_size / step as f32).abs().fract() > 1e-3 {
        1
    } else {
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::brush::BrushGrid;
    use crate::terrain::heightmap::HeightMapU16;

    fn spec() -> TerrainSpec {
        TerrainSpec {
            world_size: 32.0,
            chunk_size: 16.0,
            levels: 3,
            ..TerrainSpec::default()
        }
    }

    /// App mínimo com o runtime real e 4 COLUNAS voxel bootstrap-style
    /// (grelha 2×2, edge 16) — base dos testes de cull/respawn. Cada coluna
    /// tem uma caixa `VoxelChunk` dummy (o adopt captura daí o material e os
    /// censos de caixas têm o que contar). A câmara é criada por cada teste.
    fn lod_app(render_distance: f32) -> bevy::app::App {
        let mut app = bevy::app::App::new();
        app.add_plugins(bevy::MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(TerrainPlugin);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();

        let mut spec = spec();
        spec.render_distance = Some(render_distance);
        let map = HeightMapU16 {
            width: 33,
            depth: 33,
            data: (0..33 * 33).map(|i| (i % 1000) as u16).collect(),
        };
        let grid = BrushGrid::from_height_map(&map, spec.world_size, spec.max_height, 1.0)
            .expect("grid from heightmap");
        app.insert_resource(TerrainRuntime {
            spec: spec.clone(),
            grid: Arc::new(grid),
            water: Vec::new(),
            roads: Vec::new(),
            pads: Vec::new(),
            voxel: Arc::new(crate::terrain::voxel::VoxelField::default()),
        });

        let root = app
            .world_mut()
            .spawn((
                Name::new("terrain"),
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let edge = chunk_edge(&spec);
        let half = spec.world_size * 0.5;
        let mesh0 = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(bevy::math::primitives::Cuboid::new(
                1.0, 1.0, 1.0,
            )));
        for cz in 0..2u32 {
            for cx in 0..2u32 {
                let origin = Vec3::new(-half + cx as f32 * edge, 0.0, -half + cz as f32 * edge);
                let column = app
                    .world_mut()
                    .spawn((
                        Name::new(chunk_name(UVec2::new(cx, cz))),
                        Transform::default(),
                        Visibility::Inherited,
                        ChildOf(root),
                        TerrainChunk {
                            coords: UVec2::new(cx, cz),
                            lod: 0,
                            built_lod: 0,
                        },
                    ))
                    .id();
                app.world_mut().spawn((
                    Name::new(format!("voxel dummy {cx}-{cz}")),
                    Transform::from_translation(origin),
                    Visibility::Inherited,
                    Mesh3d(mesh0.clone()),
                    MeshMaterial3d(material.clone()),
                    ChildOf(column),
                    crate::terrain::voxel::VoxelChunk {
                        coords: bevy::math::IVec3::new(cx as i32, 0, cz as i32),
                        origin,
                        extent: edge,
                        voxel_size: 1.0,
                    },
                ));
            }
        }
        app
    }

    /// Move a câmara (única) e corre dois updates: o 1.º sincroniza
    /// Transform → GlobalTransform (PostUpdate), o 2.º corre o passe de LOD
    /// contra a posição nova.
    fn move_camera(app: &mut bevy::app::App, x: f32, z: f32) {
        let mut cam = app
            .world_mut()
            .query::<(&Camera3d, &mut Transform)>()
            .single_mut(app.world_mut())
            .expect("one camera");
        cam.1.translation = Vec3::new(x, 20.0, z);
        app.update();
        app.update();
    }

    #[test]
    fn test_chunk_name_roundtrip() {
        let coords = UVec2::new(3, 5);
        assert_eq!(parse_chunk_name(&chunk_name(coords)), Some(coords));
        assert_eq!(parse_chunk_name("chunk 0-0"), Some(UVec2::new(0, 0)));
        assert_eq!(parse_chunk_name("lake 1"), None);
        assert_eq!(parse_chunk_name("chunk x-0"), None);
    }

    #[test]
    fn test_select_lod_flat_scheme() {
        // lod_distance = 32 (chunk 16 × ratio 2): boundaries at 32 and 64.
        let (ld, margin) = (32.0, 1.0);
        assert_eq!(select_lod(10.0, ld, 0, 2, margin), 0);
        assert_eq!(select_lod(40.0, ld, 0, 2, margin), 1);
        assert_eq!(select_lod(100.0, ld, 0, 2, margin), 2);
        assert_eq!(select_lod(100.0, ld, 1, 2, margin), 2);
        // Beyond max_lod clamps.
        assert_eq!(select_lod(10_000.0, ld, 0, 2, margin), 2);
        // Degenerate distance metric.
        assert_eq!(select_lod(10.0, 0.0, 0, 2, margin), 0);
    }

    #[test]
    fn test_select_lod_hysteresis_dead_zone() {
        // Boundary at 32; margin ~1.095 for hysteresis 1.2 → dead zone
        // (32/1.095, 32×1.095) ≈ (29.2, 35.0): no switch inside it.
        let margin = 1.2f32.sqrt();
        assert_eq!(select_lod(33.0, 32.0, 0, 2, margin), 0, "stays coarse");
        assert_eq!(select_lod(31.0, 32.0, 1, 2, margin), 1, "stays fine");
        assert_eq!(select_lod(36.0, 32.0, 0, 2, margin), 1, "coarsens");
        assert_eq!(select_lod(28.0, 32.0, 1, 2, margin), 0, "refines");
    }

    #[test]
    fn test_select_lod_refines_at_top_of_target_band() {
        // Banda do LOD 1 com ld=128: [128, 256). Um chunk a LOD 2 (d ≥ 256)
        // aproxima-se para d = 200 — ainda DENTRO da banda 1, logo refinou
        // para 1. Com a fronteira na base da banda (128) só refinava a
        // d < 117 e a aproximação inteira ficava um nível mais grossária.
        let margin = 1.2f32.sqrt();
        assert_eq!(select_lod(200.0, 128.0, 2, 3, margin), 1);
        assert_eq!(select_lod(130.0, 128.0, 2, 3, margin), 1);
        // No topo da banda ainda não trocou (histerese).
        assert_eq!(select_lod(250.0, 128.0, 2, 3, margin), 2);
    }

    #[test]
    fn test_max_lod_respects_grid_divisibility() {
        // step 1 → 16 segments: divisible by 4 → max LOD 2 with 3 levels.
        let edge = chunk_edge(&spec());
        assert_eq!(max_lod_for(&spec(), edge), 2);
        // Odd segment counts stop earlier.
        let mut odd = spec();
        odd.chunk_size = 15.0;
        let edge = chunk_edge(&odd);
        assert_eq!(edge, 15.0);
        assert_eq!(
            max_lod_for(&odd, edge),
            0,
            "15 segments: not even divisible"
        );
    }

    #[test]
    fn test_chunk_edge_falls_back_to_whole_meters() {
        let mut coarse = spec();
        coarse.resolution = 32;
        coarse.chunk_size = 16.0;
        assert!((chunk_edge(&coarse) - 16.0).abs() < 1e-4);
        let mut inexact = spec();
        inexact.chunk_size = 10.0; // 10/64 rounds to step 1 → edge 10
        inexact.resolution = 64;
        assert!((chunk_edge(&inexact) - 10.0).abs() < 1e-4);
    }

    /// Integration: adopt → LOD switch → render-distance cull/respawn, all
    /// headless over a real `BrushGrid`.
    #[test]
    fn test_plugin_adopts_switches_lod_and_culls() {
        let mut app = bevy::app::App::new();
        app.add_plugins(bevy::MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(TerrainPlugin);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();

        let spec = spec();
        let map = HeightMapU16 {
            width: 33,
            depth: 33,
            data: (0..33 * 33).map(|i| (i % 1000) as u16).collect(),
        };
        let grid = BrushGrid::from_height_map(&map, spec.world_size, spec.max_height, 1.0)
            .expect("grid from heightmap");
        app.insert_resource(TerrainRuntime {
            spec: spec.clone(),
            grid: Arc::new(grid),
            water: Vec::new(),
            roads: Vec::new(),
            pads: Vec::new(),
            voxel: Arc::new(crate::terrain::voxel::VoxelField::default()),
        });

        let root = app
            .world_mut()
            .spawn((
                Name::new("terrain"),
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let edge = chunk_edge(&spec);
        let half = spec.world_size * 0.5;
        let mesh0 = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(bevy::math::primitives::Cuboid::new(
                1.0, 1.0, 1.0,
            )));
        for cz in 0..2u32 {
            for cx in 0..2u32 {
                let origin = Vec3::new(-half + cx as f32 * edge, 0.0, -half + cz as f32 * edge);
                let column = app
                    .world_mut()
                    .spawn((
                        Name::new(chunk_name(UVec2::new(cx, cz))),
                        Transform::default(),
                        Visibility::Inherited,
                        ChildOf(root),
                        TerrainChunk {
                            coords: UVec2::new(cx, cz),
                            lod: 0,
                            built_lod: 0,
                        },
                    ))
                    .id();
                app.world_mut().spawn((
                    Name::new(format!("voxel dummy {cx}-{cz}")),
                    Transform::from_translation(origin),
                    Visibility::Inherited,
                    Mesh3d(mesh0.clone()),
                    MeshMaterial3d(material.clone()),
                    ChildOf(column),
                    crate::terrain::voxel::VoxelChunk {
                        coords: bevy::math::IVec3::new(cx as i32, 0, cz as i32),
                        origin,
                        extent: edge,
                        voxel_size: 1.0,
                    },
                ));
            }
        }
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_xyz(half, 20.0, half),
            GlobalTransform::default(),
        ));

        app.update();
        app.update();
        {
            let state = app.world().resource::<ChunkLodState>();
            assert_eq!(state.tracked_chunks(), 4, "all chunks adopted");
            assert!(state.material.is_some(), "material captured");
        }
        let lod = |app: &mut bevy::app::App| {
            let mut q = app.world_mut().query::<&TerrainChunk>();
            let mut lods: Vec<u8> = q.iter(app.world()).map(|c| c.built_lod).collect();
            lods.sort_unstable();
            lods
        };
        assert_eq!(lod(&mut app), vec![0, 0, 0, 0], "camera at center: LOD 0");

        // Pin the render distance while the LOD ladder is under test: the
        // default is now derived from the resident-chunk budget
        // (`TerrainSpec::effective_render_distance`), and at `chunk_size: 16`
        // it culls at ~408 m — which is closer than the "fly far away" camera
        // below. Culling gets its own assertions right after.
        {
            let mut runtime = app.world_mut().resource_mut::<TerrainRuntime>();
            runtime.spec.render_distance = Some(10_000.0);
        }

        // Fly far away → coarser LOD everywhere (budget 4 covers 4 chunks).
        // Três updates no caminho voxel: o 1.º sincroniza a transform e
        // agenda o ColumnBuild (deferido), o 2.º constrói as caixas staged e
        // faz o swap; o 3.º cobre colunas cujas caixas não couberam no
        // orçamento do 2.º. (O heightfield reconstruía inline no próprio
        // passe; o voxel constrói caixas staged sob orçamento.)
        let mut cam = app
            .world_mut()
            .query::<(&Camera3d, &mut Transform)>()
            .single_mut(app.world_mut())
            .expect("one camera");
        cam.1.translation = Vec3::new(half + 500.0, 20.0, half);
        app.update();
        app.update();
        app.update();
        assert_eq!(lod(&mut app), vec![2, 2, 2, 2], "far camera: coarsest LOD");

        // Render distance: cull everything, then respawn on approach.
        let mut runtime = app.world_mut().resource_mut::<TerrainRuntime>();
        runtime.spec.render_distance = Some(100.0);
        app.update();
        {
            let state = app.world().resource::<ChunkLodState>();
            assert_eq!(state.tracked_chunks(), 0, "all chunks culled");
        }
        let count = |app: &mut bevy::app::App| {
            let mut q = app
                .world_mut()
                .query::<&crate::terrain::voxel::VoxelChunk>();
            q.iter(app.world()).count()
        };
        assert_eq!(count(&mut app), 0);
        let mut cam = app
            .world_mut()
            .query::<(&Camera3d, &mut Transform)>()
            .single_mut(app.world_mut())
            .expect("one camera");
        cam.1.translation = Vec3::new(half, 20.0, half);
        app.update();
        app.update();
        assert_eq!(count(&mut app), 4, "chunks respawned near the camera");
        // O respawn constrói já no LOD cru da distância ao centro (sem
        // histerese — chunk novo não tem histórico): os três chunks a 11-25 m
        // ficam em LOD 0; o canto oposto, a 33,9 m do centro da câmara
        // (16, 16), passa a fronteira de 32 m e re-entra em LOD 1.
        assert_eq!(lod(&mut app), vec![0, 0, 0, 1]);
    }

    /// Cull pela AABB do chunk: com a câmara a `render_distance < dist_centro
    /// < render_distance + meia-edge`, o corpo do chunk ainda entra no raio
    /// e NÃO é despawnado (o métrico antigo, ao centro, despawnava-o —
    /// popping à frente do horizonte). O simétrico — AABB fora do raio —
    /// continua a ser culled.
    #[test]
    fn test_cull_uses_chunk_aabb_distance() {
        let mut app = lod_app(20.0);
        // edge 16 → meia-edge 8. Câmara a 24 m do centro do chunk (1,1)
        // (centro em (8, 8)): AABB = 24 − 8 = 16 ≤ 20 → fica. Os outros três
        // chunks ficam com a AABB fora do raio (≥ 20.8 m).
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_xyz(32.0, 20.0, 8.0),
            GlobalTransform::default(),
        ));
        app.update();
        app.update();
        {
            let state = app.world().resource::<ChunkLodState>();
            assert_eq!(
                state.tracked_chunks(),
                1,
                "só o (1,1) fica: AABB 16 m dentro do raio 20 (o centro, 24 m, já saía)"
            );
            assert!(
                state.chunk_entity(UVec2::new(1, 1)).is_some(),
                "chunk parcialmente visível não é culled pelo centro"
            );
        }

        // Simétrico: câmara a 31 m do centro → AABB = 31 − 8 = 23 > 20 →
        // culled (movimento > gate de 6 m para o passe correr).
        move_camera(&mut app, 39.0, 8.0);
        let state = app.world().resource::<ChunkLodState>();
        assert_eq!(state.tracked_chunks(), 0, "AABB fora do raio: culled");
        let mut q = app
            .world_mut()
            .query::<&crate::terrain::voxel::VoxelChunk>();
        assert_eq!(q.iter(app.world()).count(), 0, "as caixas morrem com a coluna");
    }

    /// Respawn no LOD que a distância pede: chunks que re-entram no raio a
    /// 70-88 m do centro (AABB 62-79 m, raio 80) constroem-se já no LOD cru
    /// da distância — floor(log2(dist/32) + 1) = 2, o max_lod do spec — e
    /// não sempre em LOD 0. O orçamento de 4 builds/frame cobre os quatro.
    #[test]
    fn test_respawn_builds_at_distance_implied_lod() {
        let mut app = lod_app(80.0);
        // Câmara longe: tudo culled (AABB ≈ 484-500 m > 80).
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_xyz(500.0, 20.0, 8.0),
            GlobalTransform::default(),
        ));
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<ChunkLodState>().tracked_chunks(),
            0,
            "câmara longe: tudo culled"
        );

        // Re-entrada: o (1,1) fica a 70 m do centro (LOD cru 2); os restantes
        // a 72-88 m, mesmo escalão. Antes do fix, todos reconstruíam em LOD 0
        // e a re-seleção para o LOD 2 só chegava num passe seguinte.
        move_camera(&mut app, 78.0, 8.0);
        let mut q = app.world_mut().query::<&TerrainChunk>();
        let mut lods: Vec<u8> = q.iter(app.world()).map(|c| c.built_lod).collect();
        lods.sort_unstable();
        assert_eq!(
            lods,
            vec![2, 2, 2, 2],
            "respawn constrói já no LOD 2 da distância, não em LOD 0"
        );
    }

    #[test]
    fn test_plugin_is_idle_without_runtime() {
        let mut app = bevy::app::App::new();
        app.add_plugins(bevy::MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(TerrainPlugin);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.world_mut()
            .spawn((Name::new("terrain"), Transform::default()));
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::default(),
            GlobalTransform::default(),
        ));
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<ChunkLodState>().tracked_chunks(),
            0,
            "no terrain runtime: nothing tracked, no panics"
        );
    }
}

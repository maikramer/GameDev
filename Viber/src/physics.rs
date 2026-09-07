//! Physics — Rapier colliders and rigid bodies from the declarative world XML.
//!
//! VibeGame drives its browser runtime with Rapier (compiled to WASM); Viber
//! uses the same engine natively through `bevy_rapier3d`, so tuning carries
//! over instead of being re-derived against a second solver.
//!
//! # XML contract
//!
//! Two attributes, both VibeGame component strings, on any entity:
//!
//! ```xml
//! <Entity collider="shape: box; size: 0.8 0.8 0.8" rigidbody="type: fixed" />
//! <Entity collider="shape: trimesh; mesh-url: /assets/x_collision.glb; mesh-anchor: base" />
//! <Entity collider="shape: precompute; mesh-url: /assets/rock_lod0.glb" />
//! <Entity collider="auto" />
//! <Entity collider="none" />
//! <Group body="fixed">…</Group>
//! ```
//!
//! `collider` picks the shape, `rigidbody` (or the `body` shorthand on
//! `<Group>`) picks the body kind. Everything in `simple-rpg` is `fixed`: the
//! world is static scenery, and the moving things (hero, creatures) are
//! character controllers over the terrain heightfield rather than rigid bodies
//! — see [`crate::player`].
//!
//! # Deferred shapes
//!
//! `trimesh`, `precompute` and `auto` cannot be built at spawn time: they need
//! mesh data that is still loading. Those entities get a [`PendingCollider`]
//! and [`resolve_pending_colliders`] converts them once the glTF arrives, so a
//! slow asset delays one prop instead of blocking the world.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::recipes::parse_component_string;

/// Where a collision mesh's origin sits relative to the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeshAnchor {
    /// Origin as authored (the glTF's own origin).
    #[default]
    Origin,
    /// Origin at the base of the mesh — the pipeline's `reorigin-feet`
    /// convention, where y = 0 is the footprint.
    Base,
}

/// The collision shape an entity asks for.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ColliderShape {
    /// No collider at all (`collider="none"`, and the default).
    #[default]
    None,
    /// Box derived from the entity's own rendered bounds (`collider="auto"`).
    Auto,
    /// Explicit box, full size in meters, with an optional local offset.
    Box { size: Vec3, offset: Vec3 },
    /// Exact triangle mesh from a dedicated collision glTF.
    Mesh { url: String, anchor: MeshAnchor },
    /// Convex hull baked from a render mesh — the cheap stand-in used for
    /// rocks and trees, which have no authored collision mesh.
    Precompute { url: String },
}

/// Rigid-body kind requested by `rigidbody="type: …"` / `body="…"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyKind {
    /// No body — a bare collider, or nothing at all.
    #[default]
    None,
    /// Immovable scenery.
    Fixed,
    /// Simulated by the solver.
    Dynamic,
    /// Moved by code, pushes dynamics.
    Kinematic,
}

/// Everything the physics runtime needs for one entity.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PhysicsSpec {
    pub collider: ColliderShape,
    pub body: BodyKind,
    pub mass: Option<f32>,
    pub gravity_scale: Option<f32>,
}

impl PhysicsSpec {
    /// Nothing to spawn for this entity.
    pub fn is_empty(&self) -> bool {
        self.collider == ColliderShape::None && self.body == BodyKind::None
    }
}

/// Parses a `collider="…"` attribute.
///
/// Accepts the two bare forms (`none`, `auto`) and the component-string form
/// (`shape: box; size: …`). An unknown or malformed value yields
/// [`ColliderShape::None`] plus a warning string, so one bad prop never fails
/// the whole world.
pub fn parse_collider(value: &str) -> (ColliderShape, Option<String>) {
    let trimmed = value.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "" | "none" | "false" | "0" => return (ColliderShape::None, None),
        "auto" | "true" | "1" => return (ColliderShape::Auto, None),
        _ => {}
    }
    let props = parse_component_string(trimmed);
    let get = |key: &str| {
        props
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.trim().to_string())
    };
    let Some(shape) = get("shape") else {
        return (
            ColliderShape::None,
            Some(format!("collider `{trimmed}`: no `shape:` — ignored")),
        );
    };
    match shape.to_ascii_lowercase().as_str() {
        "none" => (ColliderShape::None, None),
        "auto" => (ColliderShape::Auto, None),
        "box" | "cuboid" => {
            // `size` presente mas não parseável (ex.: 2 valores) caía em
            // silêncio num cubo 1×1×1 — avisa como o mesh-url em falta.
            let size = match get("size") {
                Some(s) => match parse_vec3(&s) {
                    Some(v) => v,
                    None => {
                        return (
                            ColliderShape::None,
                            Some(format!(
                                "collider `{trimmed}`: box `size:` needs 1 or 3 values — ignored"
                            )),
                        );
                    }
                },
                None => Vec3::ONE,
            };
            // Only the Y offset is used in practice (`pos-offset-y`), but the
            // three axes are accepted for symmetry.
            let offset = Vec3::new(
                get("pos-offset-x")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0),
                get("pos-offset-y")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0),
                get("pos-offset-z")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0),
            );
            (ColliderShape::Box { size, offset }, None)
        }
        "trimesh" | "mesh" => match get("mesh-url") {
            Some(url) => {
                let anchor = match get("mesh-anchor").as_deref() {
                    Some("base") => MeshAnchor::Base,
                    _ => MeshAnchor::Origin,
                };
                (ColliderShape::Mesh { url, anchor }, None)
            }
            None => (
                ColliderShape::None,
                Some(format!("collider `{trimmed}`: trimesh without `mesh-url`")),
            ),
        },
        "precompute" | "convex" => match get("mesh-url") {
            Some(url) => (ColliderShape::Precompute { url }, None),
            None => (
                ColliderShape::None,
                Some(format!(
                    "collider `{trimmed}`: precompute without `mesh-url`"
                )),
            ),
        },
        other => (
            ColliderShape::None,
            Some(format!("collider shape `{other}`: not supported — ignored")),
        ),
    }
}

/// Parses a `rigidbody="…"` attribute (component string) or the `body="…"`
/// shorthand (a bare kind).
pub fn parse_body(value: &str) -> (BodyKind, Option<f32>, Option<f32>) {
    let trimmed = value.trim();
    if let Some(kind) = bare_body_kind(trimmed) {
        return (kind, None, None);
    }
    let props = parse_component_string(trimmed);
    let get = |key: &str| props.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim());
    let kind = get("type")
        .and_then(bare_body_kind)
        .unwrap_or(BodyKind::None);
    let mass = get("mass").and_then(|v| v.parse().ok());
    let gravity_scale = get("gravity-scale").and_then(|v| v.parse().ok());
    (kind, mass, gravity_scale)
}

fn bare_body_kind(value: &str) -> Option<BodyKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" | "" => Some(BodyKind::None),
        "fixed" | "static" => Some(BodyKind::Fixed),
        "dynamic" => Some(BodyKind::Dynamic),
        "kinematic" | "kinematicposition" | "kinematic-position" => Some(BodyKind::Kinematic),
        _ => None,
    }
}

fn parse_vec3(value: &str) -> Option<Vec3> {
    let parts: Vec<f32> = value
        .split([' ', ','])
        .filter(|p| !p.trim().is_empty())
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    match parts.len() {
        1 => Some(Vec3::splat(parts[0])),
        3 => Some(Vec3::new(parts[0], parts[1], parts[2])),
        _ => None,
    }
}

// ------------------------------------------------------------------ runtime

/// A collider that still needs mesh data before it can be built.
#[derive(Debug, Component, Clone)]
pub struct PendingCollider {
    pub shape: ColliderShape,
    /// Handle of the glTF being loaded for `Mesh` / `Precompute` shapes.
    pub gltf: Option<Handle<bevy::gltf::Gltf>>,
    /// Segundos esperando o glTF. Passou de [`PENDING_TIMEOUT`], resolve por
    /// AABB (da entidade ou da cena filha) — colisor garantido em vez de
    /// parede atravessável.
    pub age: f32,
}

/// Segundos de espera pelo glTF antes do fallback por AABB.
pub const PENDING_TIMEOUT: f32 = 4.0;

/// Colliders already baked from a glTF, keyed by asset and shape kind.
///
/// The world reuses a handful of props hundreds of times — `simple-rpg` places
/// the same crate 44 times and the same wall segment 32 — and every instance
/// would otherwise re-triangulate the identical mesh. Rapier colliders are
/// reference-counted internally, so the clones share one shape. The failure
/// reason (when the bake failed) is cached too, so the warning is emitted
/// once per asset instead of once per placed instance.
#[derive(Resource, Default)]
pub struct ColliderCache {
    baked: std::collections::HashMap<(AssetId<bevy::gltf::Gltf>, bool), BakedCollider>,
}

/// Outcome of one asset's bake: the collider, or the reason there is none.
#[derive(Clone, Debug, Default)]
pub struct BakedCollider {
    pub collider: Option<Collider>,
    /// Why the bake failed — for the warn-once log only.
    pub reason: Option<String>,
}

impl ColliderCache {
    /// Number of distinct baked shapes (cache entries).
    pub fn len(&self) -> usize {
        self.baked.len()
    }

    /// True when nothing has been baked yet.
    pub fn is_empty(&self) -> bool {
        self.baked.is_empty()
    }
}

/// Marker for entities whose collider has been resolved (or given up on), so
/// the resolver never revisits them.
#[derive(Debug, Component)]
pub struct ColliderResolved;

/// Rapier wiring for Viber.
///
/// Registers the Rapier plugin itself; collider/body components are attached
/// by [`crate::recipes::spawn`] as entities are created, and deferred shapes
/// are finished by [`resolve_pending_colliders`].
#[derive(Default)]
pub struct PhysicsPlugin {
    /// Draw collider wireframes (`viber run --physics-debug`).
    pub debug: bool,
}

impl bevy::app::Plugin for PhysicsPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_resource::<ColliderCache>()
            .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .add_systems(
                bevy::app::Update,
                (
                    resolve_pending_colliders,
                    stream_voxel_colliders,
                ),
            );
        if self.debug {
            app.add_plugins(RapierDebugRenderPlugin::default());
        }
    }
}

/// Inserts the Rapier body for a [`BodyKind`], if any.
pub fn body_bundle(
    kind: BodyKind,
    gravity_scale: Option<f32>,
) -> Option<(RigidBody, GravityScale)> {
    let body = match kind {
        BodyKind::None => return None,
        BodyKind::Fixed => RigidBody::Fixed,
        BodyKind::Dynamic => RigidBody::Dynamic,
        BodyKind::Kinematic => RigidBody::KinematicPositionBased,
    };
    Some((body, GravityScale(gravity_scale.unwrap_or(1.0))))
}

/// Builds the collider for a shape that needs no asset, i.e. an explicit box.
/// Deferred shapes return `None` and are handled by the resolver.
pub fn immediate_collider(shape: &ColliderShape) -> Option<(Collider, Transform)> {
    match shape {
        ColliderShape::Box { size, offset } => {
            let half = (*size * 0.5).max(Vec3::splat(1e-3));
            Some((
                Collider::cuboid(half.x, half.y, half.z),
                Transform::from_translation(*offset),
            ))
        }
        _ => None,
    }
}

/// Finishes [`PendingCollider`]s whose glTF has finished loading.
///
/// `Auto` resolves from the entity's rendered [`Aabb`]; the mesh shapes bake a
/// Rapier shape out of every primitive in the loaded glTF.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn resolve_pending_colliders(
    mut commands: Commands,
    time: Res<Time>,
    server: Res<AssetServer>,
    mut cache: ResMut<ColliderCache>,
    gltfs: Res<Assets<bevy::gltf::Gltf>>,
    gltf_meshes: Res<Assets<bevy::gltf::GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut pending: Query<
        (
            Entity,
            &mut PendingCollider,
            Option<&bevy::camera::primitives::Aabb>,
            Option<&Children>,
            Option<&GlobalTransform>,
        ),
        Without<ColliderResolved>,
    >,
    scene_bounds: Query<(&GlobalTransform, Option<&bevy::camera::primitives::Aabb>)>,
    mut debug_colliders: Local<u32>,
) {
    for (entity, mut request, aabb, children, entity_global) in pending.iter_mut() {
        request.age += time.delta_secs();
        match &request.shape {
            ColliderShape::Auto => {
                // The AABB only exists once the entity's mesh is loaded.
                let Some(aabb) = aabb else {
                    if request.age < PENDING_TIMEOUT {
                        continue;
                    }
                    // Cena demorou: AABB dos filhos ou desiste.
                    if let Some((center, half)) = fallback_children_aabb(children, &scene_bounds) {
                        fallback_cuboid_child(&mut commands, entity, entity_global, center, half);
                    }
                    commands.entity(entity).insert(ColliderResolved);
                    continue;
                };
                let half = Vec3::from(aabb.half_extents).max(Vec3::splat(1e-3));
                commands
                    .entity(entity)
                    .insert((Collider::cuboid(half.x, half.y, half.z), ColliderResolved));
            }
            ColliderShape::Mesh { .. } | ColliderShape::Precompute { .. } => {
                let Some(handle) = request.gltf.as_ref() else {
                    bevy::log::warn!("physics: PendingCollider sem handle (entity {entity:?})");
                    commands.entity(entity).insert(ColliderResolved);
                    continue;
                };
                if *debug_colliders < 8 {
                    *debug_colliders += 1;
                    // Diagnóstico temporário de load — trace em produção
                    // (warn poluía o log por cada collider pendente).
                    bevy::log::trace!(
                        "physics: estado do load = {:?} ({entity:?})",
                        server.get_load_state(handle)
                    );
                }
                // A collision glTF that cannot be loaded must not leave the
                // prop pending forever. The asset pool is missing dozens of
                // `*_collision.glb` files that the world still references, so
                // this is the common case, not an edge case: fall back to the
                // rendered bounds, which gives the prop *some* collision
                // instead of letting the player walk through it.
                if request.age > PENDING_TIMEOUT
                    && matches!(
                        server.get_load_state(handle),
                        Some(bevy::asset::LoadState::Loading)
                    )
                {
                    bevy::log::warn!(
                        "physics: glTF de colisão atrasado (>{} s) — fallback AABB (entity {entity:?})",
                        PENDING_TIMEOUT
                    );
                    if let Some((center, half)) = fallback_children_aabb(children, &scene_bounds) {
                        fallback_cuboid_child(&mut commands, entity, entity_global, center, half);
                    }
                    commands.entity(entity).insert(ColliderResolved);
                    continue;
                }
                if matches!(
                    server.get_load_state(handle),
                    Some(bevy::asset::LoadState::Failed(_))
                ) {
                    match aabb {
                        Some(aabb) => {
                            let half = Vec3::from(aabb.half_extents).max(Vec3::splat(1e-3));
                            commands.entity(entity).insert((
                                Collider::cuboid(half.x, half.y, half.z),
                                ColliderResolved,
                            ));
                        }
                        // No bounds yet either — give up rather than spin.
                        None => {
                            bevy::log::warn!(
                                "physics: colisão falhou ao carregar e sem Aabb — sem collider (entity {entity:?})"
                            );
                            commands.entity(entity).insert(ColliderResolved);
                        }
                    }
                    continue;
                }
                let Some(gltf) = gltfs.get(handle) else {
                    continue; // still loading
                };
                let convex = matches!(request.shape, ColliderShape::Precompute { .. });
                let key = (handle.id(), convex);
                let cached = match cache.baked.get(&key) {
                    Some(cached) => cached.clone(),
                    None => {
                        let (built, reason) =
                            collider_from_gltf(gltf, &gltf_meshes, &meshes, convex);
                        if built.is_none() && gltf_meshes_pending(gltf, &gltf_meshes, &meshes) {
                            // Meshes derivados ainda a materializar — o bake
                            // é retry no próximo tick; NÃO cachear o None
                            // (uma corrida no arranque envenenava o asset
                            // inteiro: 1.ª entidade sem collider, todas as
                            // seguintes a herdar da cache).
                            continue;
                        }
                        if built.is_none() {
                            // Warn-once POR ASSET: o bake corre uma única vez
                            // por asset (a cache guarda o motivo); as instâncias
                            // seguintes resolvem silenciosamente abaixo.
                            bevy::log::warn!(
                                "physics: bake falhou para {:?} — {} (asset {key:?})",
                                request.shape,
                                reason.as_deref().unwrap_or("motivo desconhecido")
                            );
                        }
                        let entry = BakedCollider {
                            collider: built.clone(),
                            reason,
                        };
                        cache.baked.insert(key, entry.clone());
                        entry
                    }
                };
                let Some(collider) = cached.collider else {
                    // O asset já foi reportado (warn-once acima, na 1.ª falha
                    // do bake) — dar a esta instância o mesmo destino sem
                    // repetir o warn 300× no log.
                    commands.entity(entity).insert(ColliderResolved);
                    continue;
                };
                // Both anchors currently resolve to the entity origin: the
                // pipeline already exports collision meshes with y = 0 at the
                // footprint, so `mesh-anchor: base` is a statement of that fact
                // rather than a correction to apply.
                commands.entity(entity).insert((collider, ColliderResolved));
            }
            // Boxes are built at spawn time; `None` never becomes a collider.
            ColliderShape::Box { .. } | ColliderShape::None => {
                commands.entity(entity).insert(ColliderResolved);
            }
        }
    }
}

/// AABB unido das cenas filhas (mundo). `None` quando nada tem bounds.
fn fallback_children_aabb(
    children: Option<&Children>,
    scene_bounds: &Query<(&GlobalTransform, Option<&bevy::camera::primitives::Aabb>)>,
) -> Option<(Vec3, Vec3)> {
    let children = children?;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for child in children.iter() {
        let Ok((transform, aabb)) = scene_bounds.get(child) else {
            continue;
        };
        let Some(aabb) = aabb else { continue };
        let center = transform.transform_point(aabb.center.into());
        let half = Vec3::from(aabb.half_extents);
        min = min.min(center - half);
        max = max.max(center + half);
    }
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    let center = (min + max) * 0.5;
    Some((center, (max - min) * 0.5))
}

/// Fallback AABB como FILHO da entidade, com o offset relativo ao centro do
/// prop. Inserir `Transform::from_translation(center)` na própria entidade —
/// `center` é world-space — teleportava o prop e quebrava a hierarquia.
fn fallback_cuboid_child(
    commands: &mut Commands,
    entity: Entity,
    entity_global: Option<&GlobalTransform>,
    center: Vec3,
    half: Vec3,
) {
    let local = entity_global
        .map(|g| g.affine().inverse().transform_point3(center))
        .unwrap_or(center);
    let collider = commands
        .spawn((
            Name::new("collider"),
            Collider::cuboid(half.x.max(1e-3), half.y.max(1e-3), half.z.max(1e-3)),
            Transform::from_translation(local),
        ))
        .id();
    commands.entity(entity).add_child(collider);
}

/// Bakes one Rapier collider out of every mesh primitive in a glTF.
///
/// `convex` picks a convex hull (cheap, used for `precompute` on rocks and
/// trees, which have no authored collision mesh); otherwise an exact triangle
/// mesh, which is what the pipeline's dedicated `*_collision.glb` files are
/// for. Multiple primitives become one compound collider.
/// True while any mesh derived from the glTF's primitives has not landed in
/// the `Mesh` assets yet (or the `GltfMesh` itself is still pending) — i.e. a
/// empty bake right now is a race, not malformed geometry.
fn gltf_meshes_pending(
    gltf: &bevy::gltf::Gltf,
    gltf_meshes: &Assets<bevy::gltf::GltfMesh>,
    meshes: &Assets<Mesh>,
) -> bool {
    gltf.meshes.iter().any(|mesh_handle| {
        let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) else {
            return true;
        };
        gltf_mesh
            .primitives
            .iter()
            .any(|primitive| meshes.get(&primitive.mesh).is_none())
    })
}

/// Positions plus triangle indices of a Bevy mesh, in Rapier's layout.
///
/// This exists because [`Collider::from_bevy_mesh`] rejects any mesh whose
/// index buffer is gone — and Bevy's glTF loader throws it away for every
/// primitive that ships without `NORMAL`: it calls `duplicate_vertices()`
/// (which sets `indices` to `None`) before `compute_flat_normals()`. The
/// pipeline's `*_collision.glb` files are position-only by design, so *every*
/// `shape: trimesh` prop — the city wall, the houses, the interiors — baked to
/// nothing and the hero walked straight through them. Only `shape: box` props
/// (the plaza benches, crates and barrels) still collided, because those never
/// go through a mesh at all.
///
/// An un-indexed triangle list is still a triangle list: number the vertices
/// in order and the geometry is recovered exactly.
pub fn mesh_vertices_indices(mesh: &Mesh) -> Option<(Vec<Vec3>, Vec<[u32; 3]>)> {
    use bevy::mesh::VertexAttributeValues;
    use bevy::render::mesh::{Indices, PrimitiveTopology};

    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let vertices: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(values) => values.iter().map(|v| Vec3::from(*v)).collect(),
        _ => return None,
    };
    let indices: Vec<[u32; 3]> = match mesh.indices() {
        Some(Indices::U16(idx)) => idx
            .chunks_exact(3)
            .map(|i| [i[0] as u32, i[1] as u32, i[2] as u32])
            .collect(),
        Some(Indices::U32(idx)) => idx.chunks_exact(3).map(|i| [i[0], i[1], i[2]]).collect(),
        // Un-indexed: every three vertices are one triangle.
        None => (0..vertices.len() as u32 / 3)
            .map(|t| [t * 3, t * 3 + 1, t * 3 + 2])
            .collect(),
    };
    if indices.is_empty() {
        return None;
    }
    Some((vertices, indices))
}

/// Bakes one Rapier shape out of a single mesh, with the failure reason.
///
/// `Err` carries the reason every attempt failed — used only for the
/// warn-once log in the resolver.
pub fn collider_from_mesh_with_reason(mesh: &Mesh, convex: bool) -> Result<Collider, String> {
    let (vertices, indices) = mesh_vertices_indices(mesh).ok_or_else(|| {
        "mesh sem vértices/índices utilizáveis (POSITION em falta ou topologia não-TriangleList)"
            .to_string()
    })?;
    if convex {
        Collider::convex_hull(&vertices)
            .ok_or_else(|| "convex hull falhou: sem volume convexo".to_string())
    } else {
        bake_trimesh_escalating(vertices, indices)
    }
}

/// Bakes one Rapier shape out of a single mesh.
pub fn collider_from_mesh(mesh: &Mesh, convex: bool) -> Option<Collider> {
    collider_from_mesh_with_reason(mesh, convex).ok()
}

/// Bakes an exact triangle mesh with progressively less demanding flags.
///
/// The ladder is deterministic — the first rung that yields a shape wins,
/// always in the same order:
///
/// 1. `MERGE_DUPLICATE_VERTICES | DELETE_DEGENERATE_TRIANGLES` — the healthy
///    path, identical to what we always used;
/// 2. default flags (no merge) — for meshes whose duplicate structure breaks
///    the merge preprocessing;
/// 3. convex hull of the vertices — an approximate collider still blocks the
///    player, which is strictly better than an empty bake (the prop becomes a
///    ghost).
///
/// `Err` lists what each rung reported, for the warn-once log only. Note: on
/// parry 3d 0.30.2 `TriMesh::with_flags` swallows topology errors internally
/// (`let _ = set_flags`) and only ever fails with `EmptyIndices` — so on this
/// parry the ladder normally resolves on rung 1 and rungs 2–3 are defense in
/// depth against geometry edge cases and future parry upgrades.
pub fn bake_trimesh_escalating(
    vertices: Vec<Vec3>,
    indices: Vec<[u32; 3]>,
) -> Result<Collider, String> {
    let ladder = [
        (
            "MERGE_DUPLICATE_VERTICES|DELETE_DEGENERATE_TRIANGLES",
            TriMeshFlags::MERGE_DUPLICATE_VERTICES | TriMeshFlags::DELETE_DEGENERATE_TRIANGLES,
        ),
        ("default (sem merge)", TriMeshFlags::empty()),
    ];
    let mut reasons: Vec<String> = Vec::new();
    for (label, flags) in ladder {
        match Collider::trimesh_with_flags(vertices.clone(), indices.clone(), flags) {
            Ok(collider) => return Ok(collider),
            Err(err) => reasons.push(format!("trimesh flags {label} falhou: {err}")),
        }
    }
    // Último recurso: melhor um collider aproximado que nenhum.
    match Collider::convex_hull(&vertices) {
        Some(collider) => Ok(collider),
        None => {
            reasons.push("convex hull falhou: sem volume convexo".to_string());
            Err(reasons.join("; "))
        }
    }
}

fn collider_from_gltf(
    gltf: &bevy::gltf::Gltf,
    gltf_meshes: &Assets<bevy::gltf::GltfMesh>,
    meshes: &Assets<Mesh>,
    convex: bool,
) -> (Option<Collider>, Option<String>) {
    let mut parts: Vec<(Vec3, Quat, Collider)> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for gltf_mesh_handle in &gltf.meshes {
        let Some(gltf_mesh) = gltf_meshes.get(gltf_mesh_handle) else {
            failures.push("GltfMesh derivado ainda não carregado".to_string());
            continue;
        };
        for primitive in &gltf_mesh.primitives {
            let Some(mesh) = meshes.get(&primitive.mesh) else {
                failures.push("Mesh derivado ainda não carregado".to_string());
                continue;
            };
            match collider_from_mesh_with_reason(mesh, convex) {
                Ok(collider) => parts.push((Vec3::ZERO, Quat::IDENTITY, collider)),
                Err(reason) => failures.push(reason),
            }
        }
    }
    let collider = match parts.len() {
        0 => None,
        1 => Some(parts.remove(0).2),
        _ => Some(Collider::compound(parts)),
    };
    // Motivo só para o log — agregado das primitivas que não bakearam.
    let reason = if failures.is_empty() {
        None
    } else {
        Some(failures.join("; "))
    };
    (collider, reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A position-only triangle list, the shape the glTF loader hands us for
    /// every `*_collision.glb`: no `NORMAL`, so Bevy un-indexes the primitive
    /// while computing flat normals.
    fn unindexed_quad() -> Mesh {
        use bevy::asset::RenderAssetUsages;
        use bevy::render::mesh::PrimitiveTopology;

        let positions: Vec<[f32; 3]> = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh
    }

    #[test]
    fn test_mesh_without_indices_still_bakes() {
        // `Collider::from_bevy_mesh` returns `None` here — that is the bug
        // that left the city wall, the houses and every interior prop without
        // a collider while the box-collider benches still blocked the hero.
        let mesh = unindexed_quad();
        assert!(
            Collider::from_bevy_mesh(
                &mesh,
                &ComputedColliderShape::TriMesh(TriMeshFlags::MERGE_DUPLICATE_VERTICES)
            )
            .is_none(),
            "premise: rapier refuses an un-indexed mesh"
        );

        let (vertices, indices) = mesh_vertices_indices(&mesh).expect("positions are readable");
        assert_eq!(vertices.len(), 6);
        assert_eq!(indices, vec![[0, 1, 2], [3, 4, 5]]);
        assert!(collider_from_mesh(&mesh, false).is_some(), "trimesh bakes");
        assert!(collider_from_mesh(&mesh, true).is_some(), "hull bakes");
    }

    #[test]
    fn test_mesh_with_indices_is_unchanged() {
        use bevy::render::mesh::Indices;

        let mut mesh = unindexed_quad();
        mesh.insert_indices(Indices::U16(vec![0, 1, 2, 3, 4, 5]));
        let (_, indices) = mesh_vertices_indices(&mesh).expect("indexed mesh reads");
        assert_eq!(indices, vec![[0, 1, 2], [3, 4, 5]]);
        assert!(collider_from_mesh(&mesh, false).is_some());
    }

    #[test]
    fn test_mesh_without_positions_bakes_nothing() {
        use bevy::asset::RenderAssetUsages;
        use bevy::render::mesh::PrimitiveTopology;

        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        assert!(mesh_vertices_indices(&mesh).is_none());
        assert!(collider_from_mesh(&mesh, false).is_none());
    }

    #[test]
    fn test_parse_collider_bare_forms() {
        assert_eq!(parse_collider("none").0, ColliderShape::None);
        assert_eq!(parse_collider("  NONE ").0, ColliderShape::None);
        assert_eq!(parse_collider("").0, ColliderShape::None);
        assert_eq!(parse_collider("auto").0, ColliderShape::Auto);
        assert_eq!(parse_collider("Auto").0, ColliderShape::Auto);
    }

    #[test]
    fn test_parse_collider_box() {
        let (shape, warning) = parse_collider("shape: box; size: 0.8 0.9 1.0");
        assert_eq!(
            shape,
            ColliderShape::Box {
                size: Vec3::new(0.8, 0.9, 1.0),
                offset: Vec3::ZERO,
            }
        );
        assert!(warning.is_none());
    }

    #[test]
    fn test_parse_collider_box_with_offset() {
        // The tall city-wall segments in `simple-rpg` use this exact form.
        let (shape, _) = parse_collider("shape: box; size: 1.5 7.0 2.8; pos-offset-y: 3.5");
        assert_eq!(
            shape,
            ColliderShape::Box {
                size: Vec3::new(1.5, 7.0, 2.8),
                offset: Vec3::new(0.0, 3.5, 0.0),
            }
        );
    }

    #[test]
    fn test_parse_collider_trimesh_and_anchor() {
        let (shape, _) = parse_collider(
            "shape: trimesh; mesh-url: /assets/meshes/village/anvil_collision.glb; mesh-anchor: base",
        );
        assert_eq!(
            shape,
            ColliderShape::Mesh {
                url: "/assets/meshes/village/anvil_collision.glb".into(),
                anchor: MeshAnchor::Base,
            }
        );
        // Without `mesh-anchor` the glTF's own origin is kept.
        let (shape, _) = parse_collider("shape: trimesh; mesh-url: /a.glb");
        assert_eq!(
            shape,
            ColliderShape::Mesh {
                url: "/a.glb".into(),
                anchor: MeshAnchor::Origin,
            }
        );
    }

    #[test]
    fn test_parse_collider_precompute() {
        let (shape, _) =
            parse_collider("shape: precompute; mesh-url: /assets/meshes/props/rock_mossy_lod0.glb");
        assert_eq!(
            shape,
            ColliderShape::Precompute {
                url: "/assets/meshes/props/rock_mossy_lod0.glb".into(),
            }
        );
    }

    #[test]
    fn test_parse_collider_bad_input_warns_but_never_panics() {
        let (shape, warning) = parse_collider("shape: trimesh");
        assert_eq!(shape, ColliderShape::None);
        assert!(warning.is_some(), "missing mesh-url is reported");

        let (shape, warning) = parse_collider("size: 1 2 3");
        assert_eq!(shape, ColliderShape::None);
        assert!(warning.is_some(), "missing shape is reported");

        let (shape, warning) = parse_collider("shape: teapot");
        assert_eq!(shape, ColliderShape::None);
        assert!(warning.is_some(), "unknown shape is reported");

        // A malformed `size` is reported and ignored — it used to fall back
        // silently to a 1×1×1 unit box, which hid the authoring bug.
        let (shape, warning) = parse_collider("shape: box; size: nonsense");
        assert_eq!(shape, ColliderShape::None);
        assert!(warning.is_some(), "malformed box size is reported");
        // Two values are likewise rejected (vectors are 1 or 3 values).
        let (shape, warning) = parse_collider("shape: box; size: 1 2");
        assert_eq!(shape, ColliderShape::None);
        assert!(warning.is_some(), "2-value box size is reported");
        // `size` absent keeps the unit-box default (documented shorthand).
        let (shape, _) = parse_collider("shape: box");
        assert_eq!(
            shape,
            ColliderShape::Box {
                size: Vec3::ONE,
                offset: Vec3::ZERO
            }
        );
    }

    #[test]
    fn test_parse_body_component_string() {
        // Every rigidbody in `simple-rpg` is this exact string.
        let (kind, mass, gravity) = parse_body("type: fixed; mass: 0; gravity-scale: 0");
        assert_eq!(kind, BodyKind::Fixed);
        assert_eq!(mass, Some(0.0));
        assert_eq!(gravity, Some(0.0));
    }

    #[test]
    fn test_parse_body_bare_shorthand() {
        assert_eq!(parse_body("fixed").0, BodyKind::Fixed);
        assert_eq!(parse_body("none").0, BodyKind::None);
        assert_eq!(parse_body("dynamic").0, BodyKind::Dynamic);
        assert_eq!(parse_body("kinematic").0, BodyKind::Kinematic);
        assert_eq!(parse_body("nonsense").0, BodyKind::None);
    }

    #[test]
    fn test_immediate_collider_only_builds_boxes() {
        let boxed = ColliderShape::Box {
            size: Vec3::new(2.0, 4.0, 6.0),
            offset: Vec3::Y,
        };
        let (_, transform) = immediate_collider(&boxed).expect("box builds immediately");
        assert_eq!(transform.translation, Vec3::Y);
        assert!(immediate_collider(&ColliderShape::Auto).is_none());
        assert!(immediate_collider(&ColliderShape::None).is_none());
        assert!(
            immediate_collider(&ColliderShape::Mesh {
                url: "/a.glb".into(),
                anchor: MeshAnchor::Base,
            })
            .is_none(),
            "mesh colliders wait for their asset"
        );
    }

    #[test]
    fn test_body_bundle_kinds() {
        assert!(body_bundle(BodyKind::None, None).is_none());
        let (body, gravity) = body_bundle(BodyKind::Fixed, Some(0.0)).expect("fixed body");
        assert!(matches!(body, RigidBody::Fixed));
        assert_eq!(gravity.0, 0.0);
        let (body, gravity) = body_bundle(BodyKind::Dynamic, None).expect("dynamic body");
        assert!(matches!(body, RigidBody::Dynamic));
        assert_eq!(gravity.0, 1.0, "gravity scale defaults to 1");
    }

    #[test]
    fn test_physics_spec_is_empty() {
        assert!(PhysicsSpec::default().is_empty());
        assert!(
            !PhysicsSpec {
                collider: ColliderShape::Auto,
                ..PhysicsSpec::default()
            }
            .is_empty()
        );
    }
}

// ------------------------------------------------------- terrain collision

/// Chunks within this many chunk edges of the camera keep a collider.
///
/// The whole terrain cannot be collidable at once: `simple-rpg` is a 4000 m
/// world of 64 m columns, and a collider each is tens of megabytes of solver
/// data for ground the player cannot reach this frame. Colliders stream in
/// and out with the camera instead.
pub const PHYSICS_CHUNK_RADIUS: f32 = 3.0;

/// Marks a voxel chunk that currently owns a `Voxels` collider.
#[derive(Debug, Component)]
pub struct VoxelCollider;

/// Adds and removes colliders on the voxel boxes around the camera.
///
/// No caminho 100% volumétrico TODA a superfície é caixa `VoxelChunk` — este
/// streaming é a única fonte de colisão de terreno. Sem ele as paredes
/// seriam cenário atravessável. `try_insert`/`try_remove` porque o LOD
/// despacha caixas no mesmo frame (swap de coluna e cull).
#[allow(clippy::type_complexity)]
pub fn stream_voxel_colliders(
    mut commands: Commands,
    runtime: Option<Res<crate::terrain::runtime::TerrainRuntime>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    chunks: Query<(
        Entity,
        &crate::terrain::voxel::VoxelChunk,
        Option<&VoxelCollider>,
    )>,
) {
    let Some(runtime) = runtime else { return };
    let Ok(camera) = cameras.single() else { return };
    if runtime.spec.collision_resolution == 0 {
        return;
    }
    let cam = camera.translation();
    let keep_within = runtime.spec.chunk_size * PHYSICS_CHUNK_RADIUS;
    let drop_beyond = keep_within * 1.25;

    for (entity, chunk, has_collider) in &chunks {
        // Distance to the box, not to its corner: a chunk stack is tall and
        // measuring from the origin would drop the collider under the player's
        // feet while they stand on top of it.
        let centre = chunk.origin + Vec3::splat(chunk.extent * 0.5);
        let distance = centre.distance(cam);
        match (has_collider.is_some(), distance) {
            (false, d) if d <= keep_within => {
                if let Some(collider) = terrain_collider(&runtime, chunk) {
                    // try_* e não insert/remove: no caminho 100% voxel as
                    // caixas MORREM sob os pés deste sistema (o LOD troca
                    // despacha as caixas velhas de uma coluna no mesmo
                    // frame) — aplicar num despawnado era um panic de
                    // engine, agora é um warn inofensivo.
                    commands
                        .entity(entity)
                        .try_insert((collider, RigidBody::Fixed, VoxelCollider));
                }
            }
            (true, d) if d > drop_beyond => {
                commands
                    .entity(entity)
                    .try_remove::<Collider>()
                    .try_remove::<RigidBody>()
                    .try_remove::<VoxelCollider>();
            }
            _ => {}
        }
    }
}

/// Which collider a voxel box gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerrainColliderKind {
    /// Pure base term — smooth `Collider::heightfield` from the grid.
    Smooth,
    /// A 3D feature (cave / arch / cliff) reaches into this box —
    /// `Collider::voxels`.
    Voxel,
}

/// Decides the collider of one voxel box from whether a mod's 3D bounds
/// actually reach into it.
fn terrain_collider_kind(
    field: &crate::terrain::voxel::VoxelField,
    origin: Vec3,
    extent: f32,
) -> TerrainColliderKind {
    let bounds = crate::terrain::voxel::Bounds3::from_corners(
        origin,
        origin + Vec3::splat(extent),
    );
    if field.has_mods_in(&bounds) {
        TerrainColliderKind::Voxel
    } else {
        TerrainColliderKind::Smooth
    }
}

/// The collider of one voxel box.
///
/// Caixa termo-base puro (95% do mundo): `Collider::heightfield` da grid —
/// SUAVE, o mesmo collider que o heightfield usava antes da migração.
/// Quantizar por centro de célula (`Collider::voxels` a 1 m) punha o topo
/// ±0,5 m fora da superfície desenhada: subir morro viravam paredões de 1 m
/// acima do autostep, descer virava escada, e o `last_resort_ground` do
/// player perdia o contrato `y_de_repouso == sample` (na cidade o collider
/// ficava 0,27 m ACIMA do solo visual e o herói nunca lia grounded). Caixas
/// tocadas por features 3D mantêm `Collider::voxels` — dentro de
/// grutas/arcos o degrau é o comportamento de sempre.
fn terrain_collider(
    runtime: &crate::terrain::runtime::TerrainRuntime,
    chunk: &crate::terrain::voxel::VoxelChunk,
) -> Option<Collider> {
    match terrain_collider_kind(&runtime.voxel, chunk.origin, chunk.extent) {
        TerrainColliderKind::Voxel => chunk_voxels(runtime, chunk),
        TerrainColliderKind::Smooth => chunk_smooth_heightfield(runtime, chunk),
    }
}

/// Smooth heightfield collider for a box that is pure base term, sized to the
/// box and sampled from the SAME grid the gameplay queries read — o topo do
/// collider coincide com `runtime.sample`, que é o contrato do
/// `last_resort_ground`. Heights are entity-relative (a entidade senta-se no
/// canto mínimo da caixa); o offset XZ ao centro vai no compound.
fn chunk_smooth_heightfield(
    runtime: &crate::terrain::runtime::TerrainRuntime,
    chunk: &crate::terrain::voxel::VoxelChunk,
) -> Option<Collider> {
    let size = chunk.extent;
    if !(size.is_finite() && size > 0.0) {
        return None;
    }
    let resolution = runtime.spec.collision_resolution.max(1) as usize;
    let n = resolution + 1;
    let step = size / resolution as f32;
    let mut heights = Vec::with_capacity(n * n);
    // Row-major: row walks +X, column walks +Z — o layout que o collider de
    // chunk pré-migração usava, agora por caixa de 32 m (mais fino).
    for row in 0..n {
        for col in 0..n {
            let x = chunk.origin.x + row as f32 * step;
            let z = chunk.origin.z + col as f32 * step;
            heights.push(runtime.grid.sample(x, z) - chunk.origin.y);
        }
    }
    Some(Collider::compound(vec![(
        Vec3::new(size * 0.5, 0.0, size * 0.5),
        Quat::IDENTITY,
        Collider::heightfield(heights, n, n, Vec3::new(size, 1.0, size)),
    )]))
}

/// Builds a Rapier `Voxels` collider for one volumetric chunk.
///
/// `parry` ships this shape natively and derives each voxel's type from its
/// neighbours, which is what keeps a character controller from catching on the
/// internal edges between adjacent boxes — the classic trimesh ghost-collision
/// bug. A trimesh baked from the surface-nets mesh would have to fight it.
///
/// Voxel key `k` spans `[k·size, (k+1)·size]` in the collider's local frame,
/// and the entity sits at the chunk's minimum corner, so local cell indices
/// are the keys directly.
fn chunk_voxels(
    runtime: &crate::terrain::runtime::TerrainRuntime,
    chunk: &crate::terrain::voxel::VoxelChunk,
) -> Option<Collider> {
    let size = chunk.voxel_size;
    if !(size.is_finite() && size > 0.0) {
        return None;
    }
    let cells = (chunk.extent / size).round().max(1.0) as i32;
    let mut filled: Vec<IVect> = Vec::new();
    for iz in 0..cells {
        for iy in 0..cells {
            for ix in 0..cells {
                let centre = chunk.origin
                    + Vec3::new(
                        (ix as f32 + 0.5) * size,
                        (iy as f32 + 0.5) * size,
                        (iz as f32 + 0.5) * size,
                    );
                if runtime.is_solid(centre) {
                    filled.push(IVect::new(ix, iy, iz));
                }
            }
        }
    }
    // All air or all rock with no surface: nothing worth colliding against
    // here, and an empty shape would panic the builder.
    if filled.is_empty() {
        return None;
    }
    Some(Collider::voxels(Vec3::splat(size), &filled))
}

#[cfg(test)]
mod terrain_collider_tests {
    use super::*;
    use crate::terrain::brush::BrushGrid;
    use crate::terrain::heightmap::HeightMapU16;
    use crate::terrain::voxel::VoxelField;

    fn flat_field() -> VoxelField {
        VoxelField::flat(256.0, 64.0)
    }

    #[test]
    fn test_a_pure_base_term_box_gets_the_smooth_collider() {
        let field = flat_field();
        assert_eq!(
            terrain_collider_kind(&field, Vec3::new(-32.0, 32.0, -32.0), 32.0),
            TerrainColliderKind::Smooth,
            "sem mods o collider tem de ser o heightfield suave"
        );
    }

    #[test]
    fn test_a_box_touching_a_mod_gets_voxels_and_a_distant_one_does_not() {
        use crate::terrain::voxel::mods::{BoxMod, ModOp, VoxelMod};
        let wall: Box<dyn VoxelMod> = Box::new(BoxMod::new(
            "wall",
            crate::terrain::voxel::Bounds3::from_corners(
                Vec3::new(-8.0, 24.0, -8.0),
                Vec3::new(8.0, 40.0, 8.0),
            ),
            ModOp::Union,
        ));
        let field = VoxelField::new(vec![wall], 256.0, 64.0);
        assert_eq!(
            terrain_collider_kind(&field, Vec3::new(-32.0, 32.0, -32.0), 32.0),
            TerrainColliderKind::Voxel,
            "a caixa que contém a parede fica voxels"
        );
        assert_eq!(
            terrain_collider_kind(&field, Vec3::new(32.0, 32.0, 32.0), 32.0),
            TerrainColliderKind::Smooth,
            "caixa longe do mod continua suave"
        );
    }

    #[test]
    fn test_the_smooth_collider_top_matches_the_sampled_surface() {
        // O contrato do last_resort_ground: o topo do collider coincide com
        // runtime.sample. Amalga a layout do heightfield (row-major +X/+Z,
        // alturas relativas à origem) e confere o canto.
        let n = 33usize;
        let raw: Vec<u16> = (0..n * n)
            .map(|i| {
                let x = (i % n) as f32 / (n - 1) as f32;
                ((10.0 + 5.0 * x) / 50.0 * 65535.0).round() as u16
            })
            .collect();
        let grid = BrushGrid::from_height_map(
            &HeightMapU16 { width: n, depth: n, data: raw },
            32.0,
            50.0,
            1.0,
        )
        .expect("grid");
        let runtime_spec = crate::terrain::TerrainSpec {
            world_size: 32.0,
            ..crate::terrain::TerrainSpec::default()
        };
        let _ = runtime_spec;
        let origin = Vec3::new(-16.0, 0.0, -16.0);
        let extent = 32.0_f32;
        let resolution = 32usize;
        let step = extent / resolution as f32;
        let mut heights = Vec::with_capacity((resolution + 1) * (resolution + 1));
        for row in 0..=resolution {
            for col in 0..=resolution {
                let x = origin.x + row as f32 * step;
                let z = origin.z + col as f32 * step;
                heights.push(grid.sample(x, z) - origin.y);
            }
        }
        // O vértice (0,0) do heightfield é a amostra da grelha no canto
        // mínimo da caixa, relativa à origem — exatamente o que o collider
        // constrói.
        let expected = grid.sample(origin.x, origin.z) - origin.y;
        assert!((heights[0] - expected).abs() < 1e-4);
        assert!(heights.iter().all(|h| h.is_finite()));
    }
}


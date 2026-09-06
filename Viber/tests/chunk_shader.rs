//! Terrain chunk shader regression harness — Naga parse + validation, no
//! engine, window, or assets (same contract as `tests/sky_shader.rs`).
//!
//! The chunk material's WGSL is specialized per world (CONFIG block) and
//! compiled under shader defines (`BINDLESS`, `VERTEX_COLORS`) that Bevy
//! resolves before Naga. This harness resolves them textually with explicit
//! minimal import stubs and validates every combination, so a syntax/type
//! slip in the wall skin (triplanar, strata, streaks, moss, wall space)
//! fails `cargo test` instead of crashing at material load.
//!
//! IMPORTANT: the stubs are NOT the real Bevy view/mesh layout — they prove
//! the shader's internal consistency, not pipeline-layout compatibility.

use naga::valid::{Capabilities, ValidationFlags};

/// Explicit minimal stubs for the `#import`s the chunk shader uses.
const IMPORTS: [(&str, &str); 6] = [
    (
        "#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}",
        "struct VertexOutput {\n\
         \x20   @builtin(position) position: vec4<f32>,\n\
         \x20   @location(0) world_position: vec4<f32>,\n\
         \x20   @location(1) world_normal: vec3<f32>,\n\
         \x20   @location(2) color: vec4<f32>,\n\
         \x20   @location(3) instance_index: u32,\n\
         };\n\
         struct FragmentOutput { @location(0) color: vec4<f32>, };",
    ),
    (
        "#import bevy_pbr::mesh_view_bindings::view",
        "struct View { world_position: vec4<f32>, };\n\
         @group(1) @binding(0) var<uniform> view: View;\n\
         // apply_fog lê `view_bindings::fog` — o stub textual resolve a\n\
         // referência para este var (ver REPLACE abaixo); o import REAL do\n\
         // namespace `as view_bindings` é guardado por teste próprio.\n\
         struct FogStub { color: vec4<f32>, };\n\
         @group(1) @binding(1) var<uniform> fog: FogStub;",
    ),
    (
        // O compose real (naga_oil) cria o módulo `view_bindings` a partir
        // deste alias; no harness textual o `view_bindings::fog` é reescrito
        // para o var `fog` do stub acima.
        "#import bevy_pbr::mesh_view_bindings as view_bindings",
        "",
    ),
    (
        "#import bevy_pbr::pbr_functions::apply_fog",
        "fn apply_fog(\n\
         \x20   fog_params: FogStub,\n\
         \x20   input_color: vec4<f32>,\n\
         \x20   fragment_world_position: vec3<f32>,\n\
         \x20   view_world_position: vec3<f32>,\n\
         \x20   frag_coord_xy: vec2<f32>,\n\
         ) -> vec4<f32> {\n\
         \x20   return input_color;\n\
         }",
    ),
    (
        "#import bevy_render::bindless::{bindless_textures_2d, bindless_samplers_filtering}",
        // Grupo 4: o grupo 3 passou a ser o MATERIAL (MATERIAL_BIND_GROUP_INDEX)
        // — os arrays de bindless do stub têm de ficar fora dele.
        "@group(4) @binding(0) var bindless_textures_2d: binding_array<texture_2d<f32>>;\n\
         @group(4) @binding(1) var bindless_samplers_filtering: binding_array<sampler>;",
    ),
    (
        "#import bevy_pbr::mesh_bindings::mesh",
        "struct MeshBindStub { material_and_lightmap_bind_group_slot: u32, }\n\
         @group(2) @binding(4) var<storage, read> mesh: array<MeshBindStub>;",
    ),
];

/// Resolves `#import` lines to stubs, `#ifdef/#else/#endif` against
/// `defines`, and Bevy's `#{...}` substitution placeholder.
fn standalone(source: &str, defines: &[&str]) -> String {
    let mut stack: Vec<bool> = Vec::new();
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#ifdef ") {
            let name = rest.split_whitespace().next().unwrap_or("");
            stack.push(defines.contains(&name));
            continue;
        }
        if trimmed.starts_with("#else") {
            if let Some(active) = stack.last_mut() {
                *active = !*active;
            }
            continue;
        }
        if trimmed.starts_with("#endif") {
            stack.pop();
            continue;
        }
        if !stack.iter().all(|active| *active) {
            continue;
        }
        if trimmed.starts_with("#import") {
            let stub = IMPORTS
                .iter()
                .find(|(import, _)| *import == trimmed)
                .unwrap_or_else(|| {
                    panic!(
                        "unsupported shader directive: {line}; extend the explicit harness contract"
                    )
                })
                .1;
            out.push_str(stub);
            out.push('\n');
            continue;
        }
        // O valor REAL que o bevy 0.19 substitui no runtime (material.rs:
        // MATERIAL_BIND_GROUP_INDEX = 3) — manter o placeholder a sincronizar
        // com esta constante nos dois lados.
        out.push_str(
            &line.replace(
                "#{MATERIAL_BIND_GROUP}",
                &bevy::pbr::MATERIAL_BIND_GROUP_INDEX.to_string(),
            ),
        );
        out.push('\n');
    }
    assert!(stack.is_empty(), "unbalanced #ifdef in the chunk shader");
    // O stub do `view` declara `fog` ao nível de topo; no compose real é o
    // módulo `view_bindings` (criado pelo import `as view_bindings`) que o
    // expõe. Textualmente não há namespaces em WGSL — reescreve a
    // referência para o stub.
    out.replace("view_bindings::fog", "fog")
}

fn validate(source: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    naga::valid::Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    module
}

#[test]
fn chunk_shader_validates_in_every_define_combination() {
    let template = include_str!("../src/terrain/chunk.wgsl");
    for defines in [
        vec!["BINDLESS", "VERTEX_COLORS"], // the live `run` configuration
        vec!["BINDLESS", "VERTEX_COLORS", "DISTANCE_FOG"], // idem, com câmara com `DistanceFog` (default do `run`)
        vec!["BINDLESS"], // chunk meshes always carry colors — but the gate compiles either way
        vec!["VERTEX_COLORS"], // portable non-bindless fallback
        vec!["DISTANCE_FOG"], // fog sem bindless (câmara com DistanceFog, driver sem bindless)
        vec![],
    ] {
        validate(&standalone(template, &defines));
    }
}

/// O bloco `#ifdef DISTANCE_FOG` chama `apply_fog(view_bindings::fog, …)` —
/// a referência é ao NAMESPACE que só existe se o template importar
/// `bevy_pbr::mesh_view_bindings as view_bindings`. Sem o alias, o compose
/// (naga_oil) falha com `ImportNotFound("view_bindings")`, o material perde
/// o pipeline e NENHUM chunk desenha (só o clear-color no lugar do chão —
/// regressão de 2026-09-06). O harness textual não compõe namespaces, por
/// isto fica preso aqui.
#[test]
fn fog_block_imports_the_view_bindings_namespace() {
    let template = include_str!("../src/terrain/chunk.wgsl");
    if template.contains("view_bindings::fog") {
        assert!(
            template.contains("#import bevy_pbr::mesh_view_bindings as view_bindings"),
            "apply_fog(view_bindings::fog, …) sem o import `as view_bindings` \
             quebra o compose do shader — terreno invisível no run"
        );
    }
}

#[test]
fn specialized_world_config_validates() {
    let config = viber::terrain::layer_material::TerrainChunkConfig {
        tri_slope: 0.357, // cliff-angle 50°
        tri_soft: 0.12,
        strata_spacing: 4.0,
        strata_strength: 0.25,
        rock_darken: 0.45,
        streaks: 0.55,
        moss: 0.4,
    };
    let specialized = config.render_world_shader();
    assert!(specialized.contains("const CFG_STREAK: f32 = 0.55;"));
    assert!(specialized.contains("const CFG_MOSS: f32 = 0.4;"));
    // (`#{MATERIAL_BIND_GROUP}` survives the CONFIG rewrite on purpose —
    // Bevy substitutes it at load; `standalone` resolves it for Naga.)
    for defines in [vec!["BINDLESS", "VERTEX_COLORS"], vec![]] {
        validate(&standalone(&specialized, &defines));
    }
}

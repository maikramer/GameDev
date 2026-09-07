# AGENTS.md — src/terrain

Escopo: terreno **100% volumétrico** declarativo — heightfield como INPUT
(termo-base do SDF), carve de features (pads/lagos/rios/estradas), colunas
voxel com surface nets + ladder de LOD, colliders `Collider::voxels`.
Contrato XML completo: `AGENTS.md` da raiz.

## Modelo de dados (o contrato que define tudo)

- **O `VoxelField` é a única fonte de geometria.** `density(p) =
  p.y − grid.sample(p.xz)` combinado em CSG com os mods
  (`min` = union, `max(d, −m)` = subtract; **negativo = sólido**). O
  `BrushGrid` não é renderizado nem colide directamente — é o TERMO-BASE do
  SDF e o alvo dos carves.
- **Heightmap continua a ser input:** PNG 8/16-bit / `.ahgt` / procedural FBM.
  Mundos existentes mantêm a silhueta; o que morreu foi o heightfield como
  mesh/collider/LOD (2.5D), não como dado.
- **A superfície desenhada É o zero do SDF.** `sample_mesh_surface` devolve
  sempre `voxel.surface_top` (bisseccionado, ~0,12 mm); o surface nets segue
  o campo a precisão sub-voxel, e o LOD só muda o tamanho da célula.
- **`sample(x, z)` continua a ser a superfície MAIS ALTA** (o telhado do
  arco, não o lado de baixo). Quem precisa de estar por baixo chama
  `surface_below(x, z, from_y)` — devolve `None` quando não há chão.

## Ficheiros

| Ficheiro | Responsabilidade |
|----------|------------------|
| `spec.rs` | contrato XML: `TerrainSpec`, `TerrainPadSpec`, tint. `resolution` = célula do LOD0 voxel (`chunk_size/resolution`, 1 m no default); `collision-resolution` = resolução do collider voxel por chunk edge; `levels`/`lod-distance-ratio`/`lod-hysteresis`/`render-distance` alimentam o ladder de colunas |
| `sampler.rs` | `HeightSampler` + guard `apply_pads` (só para testes) |
| `heightmap.rs` | PNG 8/16-bit ou `.ahgt` (header JSON + grid u16 deflate); sem ficheiro → procedural determinístico via `seed` |
| `brush.rs` | brush engine: modos blend/lower/raise, **journal por owner** (`pad:0`, `road:3`…) com revert para re-carve idempotente, `min_effective` (larguras < 1.5 texéis promovidas). O produção é deliberadamente **unguarded** |
| `features.rs` | **ordem de carve:** Pads → Lakes → Rivers → Roads (arteriais primeiro, **pontes por último**); estradas saltam núcleos de pads e zonas de água. Cliffs não carvam — são sólidos voxel |
| `cliffs.rs` | `CliffSpec` (parser) + **`CliffMask`** — scan de declive → abertura morfológica → BFS → filtro regional (área/queda/extent) → camadas core/dilatada + wall space (canal R). Consumidores: splat (pedra no core), relva/spawners (exclusão), shader (gate triplanar). `sharpen_terrain` (opt-in): terraceia rampas > `sharpen-angle` do campo FINAL dentro do core da máscara |
| `water.rs` | `LakeSpec`/`RiverSpec` + `WaterBody` (queries `contains`/`is_near`/`surface_y_at`/`distance_to_waterline`); lake = contorno orgânico lower-only; river = Chaikin + prefixo-mínimo (água nunca sobe) |
| `splat.rs` | **blend de solo** (`layers="…"`): gerador puro do splat map por chunk (top-4 slots), leito forçado a `pebbles`. Aliases → `/assets/textures/<alias>/albedo.ktx2` |
| `layer_material.rs` | `TerrainChunkMaterial` bindless (4 texturas + splat por chunk), shader `chunk.wgsl` reescrito por mundo; `terrain_daynight_tint` |
| `water_material.rs` / `water_fx.rs` | `WaterMaterial` (Beer–Lambert, fresnel, glint) + splash/esteira |
| `roads.rs` | `RoadSpec`/`RoadNetworkSpec` + `RoadPath`; profiles, flatten com teto de grade, ribbons + discos de junção |
| `decal.rs` | `GroundDecalSpec` + `ground_decal_mesh` — manchas de chão drapejadas, só visuais |
| `voxel/` | **a forma 3D inteira.** `field.rs` (`VoxelField` SDF: `surface_top`/`surface_below`/`column`/`region_state`), `mods.rs` (trait `VoxelMod` + `BoxMod`/`CapsuleMod`/`ArchMod`), `index.rs` (`ModIndex` bucket XZ O(1)), `cliff.rs` (`<Cliff>` 3D — `CliffBand` + `profile_offset` negativo = undercut), `cave.rs` (`<Cave>` cápsulas subtractivas), `arch.rs` (`<Arch>` união com vão), `riverbank.rs` (margens gorge/overhang + nascente), `surface_nets.rs` (mesher dual com QEF-lite + seals de coluna), `spawn.rs` (`lod_shape`/`column_boxes`/`build_box_mesh`/spawn de colunas) |
| `paths.rs` | `chaikin_smooth` / `resample` |
| `mesh.rs` | infraestrutura partilhada: `HeightField` (termo-base), `ChunkMeshData`, `TintParams`/`tint_vertex_color` (tint legado + canais R/A de parede no caminho layers) |
| `plugin.rs` | `TerrainPlugin`: ladder de COLUNAS voxel — adopt → select com histerese → construção staged sob budget de CAIXAS (4/frame) → swap atómico → cull por `render-distance` → respawn no LOD cru |
| `runtime.rs` | `TerrainFeaturesPlugin`: bootstrap one-shot (heightmap → carve → mods → colunas voxel → água/ribbons); gera splats + materiais e publica `TerrainChunkMaterials`/`TerrainRuntime` |

## Regras duras (não quebrar)

- **Sampler CPU único:** nenhuma segunda fonte de verdade de altura. O
  `BrushGrid` é o base(p) do SDF; numa coluna sem mod o campo É o heightfield
  (mesmo valor, custo de Catmull-Rom).
- **Visual do chão, dois caminhos exclusivos:** tint LEGADO por
  altura/inclinação em vertex colors OU o splat blend (`layers="…"`), que
  SUBSTITUI o tint — `TerrainSpec::chunk_tint()` zera o banding com camadas
  ativas, no bootstrap E nos rebuilds (desalinhados, caixas vizinhas
  discutem). Com `layers`, R das vertex colors = wall space e A = cliff
  factor; no legado, RGBA = tint integral (o StandardMaterial multiplica).
- **Colliders por caixa, dois formatos** (`terrain_collider_kind`):
  caixa termo-base puro → `Collider::heightfield` SUAVE da grid (o topo
  coincide com `runtime.sample` — o contrato do `last_resort_ground` do
  player; quantizar por centro de célula punha o topo ±0,5 m fora da
  superfície desenhada: subir morro a saltos, descer em escada, o herói
  nunca lia grounded na cidade); caixa tocada por um mod 3D (gruta/arco/
  cliff) → `Collider::voxels`. Streaming `stream_voxel_colliders` com
  try_insert/try_remove (o LOD despacha caixas no mesmo frame). Célula =
  `chunk_size/collision-resolution`; `0` desliga tudo.
- Todo o mutate passa pelo brush engine com journal — carve tem de ser
  **idempotente**.
- **Decals são só visuais** — `GroundDecal` nunca toca no heightfield.
- **Camadas transparentes de chão têm ordem fixa:** `DECAL_LIFT` (0.04) <
  `RIBBON_LIFT` (0.06) < `JUNCTION_LIFT` (0.10).
- **Nada de decal transparente a projetar sombra** (`NotShadowCaster`); a
  água sob a mesma regra.
- **Contrato de vértice da água** (`water.rs` ↔ `water.wgsl`): `COLOR.rgb` =
  cor do corpo, `COLOR.a` = máscara de margem, `UV.x` = escala de extinção,
  `UV.y` = radial/transversal.
- **A opacidade da água NÃO é um alpha constante** (depth prepass →
  Beer–Lambert); **fresnel com normal quase plana**; hash de ruído INTEIRO
  (u32) em WGSL world-space; FBM com rotação por oitava.
- **`sharpen` é opt-in e nunca default** — muda alturas (colisão, spawns,
  quests movem-se).
- **WGSL do muro: derivadas só em fluxo uniforme**; `textureSampleGrad` com
  gradientes calculados uma vez no topo do fragment.

## Colunas voxel — regras duras

- **Ladder por coluna, não por caixa:** `lod_shape(lod0_cell, edge, lod)`
  deriva `(cells, per_edge)` — LOD0 2×2 caixas de 32³ @1 m, LOD1 1×64³ @2 m,
  LOD2 16³ @4 m (chunk 64 m). Determinístico e global: vizinhos ao mesmo LOD
  derivam o MESMO shape — é isso que fecha costuras por coincidência de
  vértices.
- **`region_state` antes de amostrar** — céu/bedrock provados em O(1) pelo
  `range_over` da grid; sem isso uma coluna de 200 m de relevo custa ~1 M
  avaliações.
- **Construção staged:** as caixas do novo LOD nascem `Visibility::Hidden`
  sob o budget; quando a fila esvazia, as velhas morrem e as novas ficam
  visíveis — troca atómica, nunca dois LODs do mesmo chão nem buraco.
- **Seals de fronteira de coluna:** saia vertical (`seal_depth =
  max(4×célula, 2 m)`) nas faces ±X/±Z na fronteira, SÓ em arestas com
  traverso horizontal. "Sempre selar" é deliberado (parede enterrada quando
  o vizinho concorda; saber o LOD do vizinho acoplava colunas).
- **As caixas são iteradas a partir da grelha de chunks de terreno**, não de
  uma grelha voxel própria — as origens não coincidem num mundo de 4 km
  (−2000 não é múltiplo de 32 m); transbordar é z-fighting.
- **Material por coluna do `ChunkLayerMap`**; o standard fallback é
  double-sided (folhas finas sub-voxel com culling leem-se como buracos).
- **Despawn de coluna é recursivo** (Bevy 0.19): as caixas morrem com ela no
  cull.
- **Normais vêm do gradiente do campo** — contínuas através das fronteiras,
  sem estado partilhado entre builds.
- **Flaps sub-voxel conhecidos:** folhas finas (lips de carve a centímetros)
  produzem ~0,4% de triângulos degenerados num mundo carvado — teto honesto
  em `test_carved_boxes_have_no_degenerate_triangles`; o fix real é
  refinamento de voxel perto de features, não mesher novo.

## Desvios conhecidos vs VibeGame (documentados, não afetam o simple-rpg)

Estações de road a 1 m (vs 0.35); sem berms/cross-slope; decks de ponte são
ribbons planas (GLB chega com glTF).

## Verificar

```bash
cd Viber && cargo test          # inclui tests/terrain_mesh_health.rs (voxel)
cargo test --release --test chunk_build_bench -- --nocapture
cargo run -- analyze worlds/terrain.xml   # mundo demo de terreno
```

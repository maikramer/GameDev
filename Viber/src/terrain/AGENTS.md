# AGENTS.md — src/terrain

Escopo: terreno heightfield declarativo — sampler, carve de features
(pads/lagos/rios/estradas), chunks de mesh + colliders, LOD runtime.
Port de `bevy_mesh_terrain` (MIT) corrigido + contratos do plugin terrain do
VibeGame. Contrato XML completo: `AGENTS.md` da raiz.

## Ficheiros

| Ficheiro | Responsabilidade |
|----------|------------------|
| `spec.rs` | contrato XML: `TerrainSpec`, `TerrainPadSpec`, tint |
| `sampler.rs` | `HeightSampler` — **autoridade única da forma**; mesh, colliders, pads e queries de gameplay leem a mesma grid. Versão com guard (`apply_pads`) existe só para testes |
| `heightmap.rs` | PNG 8/16-bit ou `.ahgt` (header JSON + grid u16 deflate); sem ficheiro → procedural determinístico via `seed` |
| `brush.rs` | brush engine: modos blend/lower/raise, **journal por owner** (`pad:0`, `road:3`…) com revert para re-carve idempotente, `min_effective` (larguras < 1.5 texéis promovidas) |
| `features.rs` | **ordem de carve:** Pads → Lakes → Rivers → **Cliffs** → Roads (arteriais primeiro, **pontes por último**); estradas saltam núcleos de pads e zonas de água. Cliffs vão ANTES das roads de propósito — a parede já existe quando a road faz o survey e o `limit_grade` reage a ela |
| `cliffs.rs` | **cliffs procedurais 2.5D** — o único carve que QUER o degrau vertical. `CliffSpec` (`<Cliff path width height angle profile side noise seed>`): banda one-side ao longo do creste; planos top/toe amostrados ANTES do stroke (determinístico, byte-igual entre runs); perfis `vertical` (S-curve) / `concave` (`t^0.4`, vertical no creste) / `convex` (`t^2.2`, abaulado — sobrancelha redonda, pé a pico) / `columnar` (basalto do wargame: partição do creste em colunas 1.6–4.2 m com offset plano por coluna ±0.28·largura — o salto de 1 texel entre colunas é a fenda escura; facetas piecewise-linear com 2 quebras = lajes de corte reto; jitter de topo ±0.8 m) / `terraced` (quantizado por CORRIDA ~2.5 m — pisos planos onde o splat pinta relva, riscas de pedra). `height` ausente = auto (a diferença natural do lugar); `height` autoral fixa o toe em `top − height` (pedreira em chão plano). `sharpen_terrain` (opt-in `<Terrain sharpen>`): terraceia rampas > `sharpen-angle` do campo FINAL (salta texéis subaquáticos; roads nunca disparam — grade limitada abaixo do ângulo). `CliffMask` v2 (recurso): pipeline com LEITURA DE VIZINHANÇA — scan bruto → abertura morfológica (erode+dilate mata speckle de 1 texel) → componentes conexos BFS → filtro regional (área ≥ `cliff-min-area`, queda ≥ `cliff-min-drop`, extensão ≥ `cliff-min-extent`) → camadas **core** (aceites) e **dilatada** (+2 texéis). Consumidores: o mesh coza `factor()` bilinear no ALPHA das vertex colors e o shader multiplica o gate triplanar por ele (declive espúrio = 0 = sem pedra); o splat pinta PEDRA SÓLIDA no core e esbate relva/neve no anel; o sharpen só terraceia o core e nunca as bandas `<Cliff>` autorais; o LOD peak-preserving só faz snap com fator > 0.5; a relva (grass.rs) rejeita a camada dilatada |
| `water.rs` | `LakeSpec`/`RiverSpec` + `WaterBody` (queries `contains`/`is_near`/`surface_y_at`/`distance_to_waterline`/`blend_surface_y`); lake = contorno orgânico lower-only; river = Chaikin + prefixo-mínimo (água nunca sobe) |
| `splat.rs` | **blend de solo** (`layers="…"`): gerador puro do splat map — 3 planos RGBA8 com os weights das 12 camadas do pool (altitude, declive, margens de água, ombros de estrada, noise determinístico p/ variações). Aliases → `/assets/textures/<alias>/albedo.ktx2` |
| `layer_material.rs` | `TerrainLayerMaterial` = `StandardMaterial` + `TerrainLayerExtension` (WGSL `shaders/terrain_layers.wgsl`, bindings 40–69 no grupo de material): 12 albedos + 3 splats, albedo/roughness blended antes do PBR stock. CFG = world span + consts do muro de cliff (gate triplanar derivado de `cliff-angle`, estratos) (mesma arquitetura de consts do `sky.rs`/`water_material.rs`) |
| `water_material.rs` | `WaterMaterial` = `StandardMaterial` + `WaterExtension` (WGSL `shaders/water.wgsl` especializado por mundo a partir de `<DayCycle>`/`<Weather>`): ondas por vento (normais animadas), **volume por Beer–Lambert sobre o depth prepass**, espuma de margem, fresnel, tint de céu e glint sol/lua. Zero uniforms — consts + `Globals` (mesma arquitetura do `src/sky.rs`) |
| `water_fx.rs` | `WaterFxPlugin` — splash de entrada/saída e esteira de ondas (anéis `Ripple` + bursts `splash`/`wade`) de quem caminha dentro de água. Vive no ECS porque a lâmina é estática e o shader não pode receber a posição do herói |
| `roads.rs` | `RoadSpec`/`RoadNetworkSpec` + `RoadPath` (`is_on_road`/`distance_to_road`); profiles, flatten com teto de grade |
| `decal.rs` | `GroundDecalSpec` + `ground_decal_mesh` — manchas de chão drapejadas (chão de praça, apron de mercado, discos de junção). Anéis concêntricos (não um leque: um leque é planar entre o centro e a borda e corta o terreno em declive), borda ondulada por harmónicos **periódicos** (fecha a costura), alpha 1→0 em smootherstep |
| `voxel/` | **camada de forma 3D** sobre o heightfield. `cliff.rs` (`<Cliff>` 3D — `CliffBand` + `profile_offset`), `cave.rs` (`CaveSpec` — a tag `<Cave>`), `field.rs` (`VoxelField` — SDF em camadas: `density(p) = p.y - grid.sample(p.xz)` combinado em CSG com os mods; `surface_top`/`surface_below`/`column`/`region_state`), `mods.rs` (trait `VoxelMod` + `Bounds3` + primitivas `BoxMod`/`CapsuleMod`), `index.rs` (`ModIndex` — bucket XZ que responde "há algo 3D nesta coluna?" em O(1); **não aloca nada** num mundo sem mods), `surface_nets.rs` (mesher dual, chunks de 32³), `spawn.rs` (entidades `VoxelChunk`) |
| `paths.rs` | `chaikin_smooth` / `resample` |
| `mesh.rs` | `HeightField` (com `range_over` para o LOD), `ChunkMeshData` (skirts + frontier normals — **sem stitching**), `TerrainColliderData` por chunk. **LOD peak-preserving:** a step > 1, um vértice cuja célula contém range cru mais íngreme que `cliff-angle` faz snap ao extremo (crista/corte) em vez de point-sample — cristas-faca deixam de se afundar quando a câmara afasta |
| `plugin.rs` | `TerrainPlugin`: LOD por distância da câmara com histerese + gate de reselect, rebuild com budget/frame, cull por `render-distance` |
| `runtime.rs` | `TerrainFeaturesPlugin`: bootstrap one-shot (heightmap → carve → entidades de chunks/água/ribbons); com `layers`, gera splats + material de camadas e publica `TerrainChunkMaterials` para o plugin de LOD |

## Regras duras (contratos portados — não quebrar)

- **Sampler CPU único:** nenhuma segunda fonte de verdade de altura.
- **Skirts + frontier normals** em vez de stitching entre chunks/LODs.
- **Visual do chão, dois caminhos exclusivos:** tint LEGADO por
  altura/inclinação em vertex colors (sem WGSL) OU o splat blend
  (`layers="…"`), que SUBSTITUI o tint — com camadas ativas o `base-color`
  autoral é IGNORADO: as vertex colors transportam dados de parede/região
  para `terrain_chunk.wgsl` e o tint global é o `day_tint` day/night do
  uniform (`TerrainSpec::chunk_tint` zera o banding no bootstrap E nos
  rebuilds de LOD; desalinhados, chunks vizinhos discutem na fronteira).- O splat fecha a soma dos weights em 1.0 (relva = resto); pesagem
  < 0.004 no shader salta o fetch (branch não-uniforme ⇒ `textureSampleGrad`
  com gradientes calculados UMA vez no topo do fragment).
- Colliders heightfield por chunk com `collision-resolution` independente
  (0 desliga os colliders).
- Todo o mutate passa pelo brush engine com journal — carve tem de ser
  **idempotente**.
- **Decals são só visuais** — `GroundDecal` nunca toca no heightfield.
- **Camadas transparentes de chão têm ordem fixa:** `DECAL_LIFT` (0.04) <
  `RIBBON_LIFT` (0.06) < `JUNCTION_LIFT` (0.10). São todas alpha-blend sobre a
  mesma textura e os mesmos UVs world-space; sem a separação brigam no depth.
- **Raio do disco de junção segue a ribbon que cobre** (meia-largura × flare ×
  wobble + folga). O piso antigo `default_width × 2` dava um círculo de 16 m a
  uma estrada de 4 m — a regressão do portão norte.
- **Nada de decal transparente a projetar sombra** (`NotShadowCaster`): a
  silhueta cai de volta no chão e desenha um anel escuro à volta da mancha.
  A água (lakes/rivers) está sob a mesma regra.
- **Contrato de vértice da água** (`water.rs` ↔ `water.wgsl`): `COLOR.rgb` =
  cor do corpo, `COLOR.a` = **só a máscara de margem** (geometria), `UV.x` =
  a `opacity` do `<Lake>`/`<River>` lida como **escala de extinção** da coluna,
  `UV.y` = coordenada radial / transversal. Um material branco partilhado
  serve todos os corpos. Não sampleamos textura nenhuma — é por isso que
  `UV.x` está livre; quem adicionar uma textura à água tem de mover a
  extinção para outro sítio primeiro.
- **A opacidade da água NÃO é um alpha constante.** O alpha sai da
  profundidade real da coluna (depth prepass → Beer–Lambert) somada ao
  Fresnel; `opacity` no XML só diz quão turva a água é. Voltar a um alpha
  fixo devolve o efeito "celofane" (lâmina transparente sem volume).
- **O Fresnel da água usa uma normal QUASE PLANA**, não a normal ondulada.
  Alimentá-lo com as cristas fazia a opacidade variar onda a onda e desenhava
  um xadrez de faixas rectas por cima do fundo — a regressão visível no lago
  do pântano.
- **Nada de senos de onda com direcções ortogonais e λ longo** (o campo antigo
  era ±vento/±perpendicular com λ 11–30 m): as cristas alinham-se em duas
  famílias e lêem-se como grelha. Direcções não ortogonais, λ curtos e
  incomensuráveis, e FBM com **rotação por oitava** (senão as oitavas
  partilham os eixos da grelha do `value_noise`).
- **O hash do ruído da água é INTEIRO (u32), não `fract(p * 123.34)`.** O hash
  float clássico colapsa em coordenadas de mundo grandes: num mapa de 4 km,
  `x·123.34 ≈ 2.5e5`, onde o espaçamento do f32 já é ~0.015 — o `fract` fica
  com uma mão-cheia de valores distintos e o ruído degenera em **degraus
  rectos alinhados**, que é a grelha de faixas que se via a atravessar os
  lagos. Vale para qualquer WGSL novo que amostre ruído em world-space (o
  `sky.wgsl` usa o hash float sem problema porque amostra **direcções**, de
  magnitude ≤ 1). Diagnóstico: escrever `n * 0.5 + 0.5` no `out.color` — se as
  faixas aparecem no debug da normal, é o ruído, não a geometria.
- O `BrushGrid` de produção é deliberadamente **unguarded** (o clamp
  lower-only ao anel de stencil fabricava falésias); não "corrigir" sem ler o
  histórico.
- **O carve de cliff é a exceção consciente às convenções de feather** — todos
  os outros carvers evitam o degrau; o cliff quer. Amostragens (top/toe) ANTES
  do `begin_stroke`; nunca ler a grid a meio do carve próprio.
- **`sharpen` é opt-in e nunca default** — muda alturas (colisão, spawns,
  quests movem-se). Ativar num mundo existente só após QA visual.
- **WGSL do muro: derivadas só em fluxo uniforme.** `dpdx/dpdy` e os
  gradientes dos DOIS eixos laterais calculados fora do ramo `tri`; dentro do
  ramo só `textureSampleGrad` (gradientes explícitos, legal em fluxo
  não-uniforme). O `rough` do muro entra pré-multiplicado por `total` porque
  o blend splat divide por `total` depois.

## Cliffs 3D (Fase B) — o que mudou

**`<Cliff>` já NÃO carva.** `carve_cliff` deixou de ser chamado; o
`features.rs` documenta a ausência. A parede é agora um sólido no campo voxel
(`voxel/cliff.rs`), construído pelo bootstrap a partir das mesmas specs.

- **A parametrização inverteu-se, e é aí que mora o overhang.** O carve
  perguntava *"a que altura está o chão à distância lateral `t`?"*
  (`profile_s(t)`) — necessariamente uni-valorado, necessariamente sem
  saliência. O sólido pergunta *"a que distância está a face à profundidade
  `v`?"* (`profile_offset`), que pode ser **negativa**: a face recua por baixo
  do creste e fica rocha por cima da cabeça.
- **Perfis:** `vertical` é agora mesmo aprumado (a 2.5D espalhava a queda pelo
  terço médio da banda — uma rampa de ~60° com nome de parede); `concave` tem
  undercut real; `convex` faz a sobrancelha exceder o próprio pé; `columnar` põe
  o offset de coluna em geometria em vez de o simular por amostragem; `terraced`
  ganha lábio (o recuo vive só no ARRANQUE do degrau — espalhá-lo pelo piso
  inclina o degrau e devolve a rampa).
- **Um mod por segmento do creste, não um por cliff.** Um mod da polyline
  inteira punha cada amostra de densidade a percorrer todas as estações (~90
  numa parede de 180 m × ~39 k amostras por chunk).
- **A `CliffMask` autorada vem do índice** (`add_authored_bands`), não do
  pipeline morfológico. O scan erode/dilate/BFS fica para o declive NATURAL do
  heightmap, que esse sim tem de ser descoberto. `wall_at` sai da geometria em
  vez da janela ±8 texéis.
- **Colisão:** chunks volumétricos ganham `Collider::voxels`
  (`physics.rs::stream_voxel_colliders`). Sem isto a parede era cenário
  atravessável — ela já não está na grid, e a grid é tudo o que o collider
  heightfield conhece.
- **Consequência a saber:** os cliffs carvavam ENTRE a água e as estradas de
  propósito, para o survey da estrada ler a parede acabada e o `limit_grade`
  reagir. Com a parede fora da grid isso deixou de acontecer. Nenhum mundo
  entregue faz isso — o `simple-rpg` mantém os cliffs a ~30 m de qualquer
  artéria — mas um mundo que tentasse passava a ribbon por baixo da rocha.

## Voxel (Fase A) — regras duras

- **O heightfield não foi substituído, foi despromovido a termo.** O
  `BrushGrid` continua a ser carvado por pads/lagos/rios/estradas pelo mesmo
  brush engine journalado, e é o `base(p)` do SDF. Numa coluna sem mod, o
  `VoxelField` É o heightfield — mesmo valor, mesmo custo. Não pode nascer uma
  segunda fonte de altura.
- **Sinal: negativo = sólido.** `density(p) = p.y - altura(p.xz)`, combinado
  com `min` (union) e `max(d, -m)` (subtract).
- **`sample(x, z)` continua a ser a superfície MAIS ALTA.** É o que os 44 call
  sites querem (o telhado do arco, não o lado de baixo). Quem precisa de estar
  por baixo de alguma coisa chama `surface_below(x, z, from_y)`, que devolve
  `None` quando não há chão lá em baixo — devolver `Some(from_y)` punha o
  jogador dentro do monte.
- **Um chunk de terreno é OU heightfield OU volumétrico, nunca os dois.**
  `spawn_chunks` salta os chunks que o `ModIndex` classifica `Volumetric` e o
  `spawn_voxel_chunks` cobre-os. Desenhar os dois é z-fighting garantido.
- **`region_state` antes de amostrar.** Um chunk de terreno de 64 m sobre 200 m
  de relevo são ~28 caixas de 32³; amostrar todas às cegas é ~1 M avaliações de
  densidade inline, num frame. O `range_over` do heightfield prova em O(1) que
  uma caixa é céu ou bedrock antes de se tocar nela.
- **Chunks voxel são 32³, não o chunk de terreno.** O `plugin.rs` constrói
  meshes inline de propósito (o crate upstream perdia chunks num bug de orphan
  task) e a justificação é "small heightfield grids build in well under a
  frame" — 131 k células não são. 32³ cabe no frame e dá streaming/culling em
  **Y**, que as grutas precisam.
- **As costuras fecham por coincidência, não por stitching.** Chunks vizinhos
  meshiam com uma célula de sobreposição e derivam os vértices do mesmo campo
  analítico, portanto caem em posições byte-iguais. Mesma barganha das saias +
  frontier normals do caminho heightfield: paga-se duplicação, ganha-se
  builders independentes. Coberto por
  `test_neighbouring_chunks_agree_on_the_shared_boundary`.
- **Normais vêm do gradiente do campo**, não dos triângulos — contínuas através
  das fronteiras, sem estado partilhado entre builds.
- **As caixas voxel são iteradas a partir da grelha de CHUNKS DE TERRENO**, não
  de uma grelha voxel própria. As duas grelhas não partilham origem — a do
  terreno começa em `-world_size/2`, que no mundo de 4 km é −2000, não múltiplo
  da caixa de 32 m. Uma caixa ancorada na sua própria grelha transborda para o
  chunk vizinho, meshia chão que o mesher de heightfield também desenha, e o par
  faz z-fighting em toda a sobreposição (moiré de ecrã inteiro, não uma costura
  discreta). Coberto por
  `test_no_voxel_box_lands_in_a_chunk_the_heightfield_still_owns`, que corre a
  256 / 300 / 4000 m precisamente pelos casos desalinhados.
- **O `plugin.rs` TEM de saltar os mesmos chunks que o bootstrap saltou.** Ele
  respawna qualquer chunk sem entidade ("or skipped by the bootstrap", comentário
  original) — sem o guard, tudo o que o bootstrap entregou ao voxel volta a nascer
  por cima. Um só classificador (`VoxelField::is_volumetric_chunk`), dois
  chamadores, sem deriva.
- **Chunks voxel usam o material de camadas do MESMO chunk de terreno**
  (`ChunkLayerMap::get(cx, cz)`). Com `layers` ativo o terreno é desenhado pelo
  `TerrainChunkMaterial` e o handle standard é só fallback — dar o fallback às
  caixas voxel pinta-as de branco liso ao lado de uma encosta texturada.
- **Vertex colors do voxel usam o MESMO `tint_vertex_color`** (e o
  `spec.chunk_tint()`, que zera o banding quando `layers` está ativo) do mesher
  de heightfield. Sem isso a região voxel renderiza a outra cor e a fronteira
  entre os dois meshers salta à vista.

## Desvios conhecidos vs VibeGame (documentados, não afetam o simple-rpg)

Estações de road a 1 m (vs 0.35); sem berms/cross-slope; decks de ponte são
ribbons planas (GLB chega com glTF).

## Verificar

```bash
cd Viber && cargo test          # inclui tests/terrain_mesh_health.rs
cargo run -- analyze worlds/terrain.xml   # mundo demo de terreno
```

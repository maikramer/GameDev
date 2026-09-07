# Performance — o que domina o frame no Viber

Medido no `simple-rpg` (mundo de 4 km, ~9700 cenas glTF, 61k entidades) numa
**RTX 4050 Laptop (6 GiB)**. Todos os números vêm de
`viber debug prof --samples 10` e `viber debug lua 'return viber.debug.stats()'`.

## O resultado

| Cenário | frame (média) | chunks | VRAM |
|---------|---------------|--------|------|
| Como estava: perfil dev, `opt-level = 0` | **114 ms** (8 fps) | 2047 | 2817 MiB |
| Release, sem culling/LOD | 29.3 ms (35 fps) | 2048 | 2817 MiB |
| Release + culling + ladder de LOD + `render-distance` | **10.7–17.4 ms** (57–94 fps) | 691 | 2817 MiB |
| **Tudo, com texturas KTX2** | 10.7–17.4 ms (57–94 fps) | 691 | **1340 MiB** |

**Cite o `frame_ms_avg`, não o `fps`.** O `fps` de uma amostra é instantâneo
e salta entre 22 e 148 no mesmo segundo. E mesmo o frame médio oscila entre
corridas nesta máquina: a RTX 4050 é portátil e o SM anda entre 2535 e
3105 MHz conforme a temperatura — as corridas a 10.7 ms e a 17.4 ms têm
composição idêntica (mesmos chunks, mesmos `culled`, mesmos meshes) e diferem
só no boost. Por isso a última linha é um intervalo, não um número.

As texturas KTX2 **não mexem no fps**, mexem na VRAM: um A/B com o chão em
`.webp` e em `.ktx2` deu 17.36 vs 17.35 ms.

A linha final corre com `cull-distance` 320 m em vez de 240 — 7% de fps por
33% mais alcance de vista.

**Resumo honesto: 114 ms → ~11–17 ms de frame (7–11x) e −52% de VRAM**, no
mesmo mundo e na mesma GPU.

Estado ao vivo (`viber.debug.stats()`): 10019 instâncias com `CullDistance`,
**7009 escondidas**; ladder de LOD com 2540 no tier 0, 559 no tier 1 e 2422
no tier 2.

## As quatro causas (por ordem de impacto)

### 1. O jogo corria sem optimizações

`viber run` delegava em `cargo run` **sem** `--release`, e o `--release` que
existia era emitido como `cargo --release run` — ordem inválida, portanto a
flag nunca funcionou. Bevy e Rapier a `opt-level = 0` são 4–10x mais lentos.

Corrigido em dois sítios:

* `viber run` corre **release por omissão** (`--debug` volta ao perfil dev);
* o `Cargo.toml` optimiza as dependências mesmo no perfil dev
  (`[profile.dev] opt-level = 1` + `[profile.dev.package."*"] opt-level = 3`),
  para o `--debug` continuar jogável.

### 2. Nada era cortado por distância

Cada `<StaticSpawner>` / `<Vegetation>` spawnava a cena glTF completa e ela
ficava no mundo de render para sempre: 61k entidades extraídas, testadas
contra o frustum e desenhadas em **quatro cascatas de sombra**, num mapa onde
o jogador vê ~300 m.

[`src/render_lod.rs`](../src/render_lod.rs) trata disso com o mesmo padrão do
`ScriptActivation` (o "LOD de IA"):

* `CullDistance` no **root** da instância — a visibilidade é herdada em Bevy,
  logo esconder o root apaga a subárvore inteira sem componente por malha;
* `NoShadowSubtree` marca a erva como não-projectora (`NotShadowCaster` é lido
  na entidade da malha, por isso este precisa mesmo de propagação);
* atributos XML `cull-distance` e `cast-shadows` em `<StaticSpawner>` e
  `<Vegetation>`; defaults 320 m (props), 80 m (erva, sem sombra), 160 m
  (criaturas de `<DynamicSpawner>` — o script já congela aos 45 m).

`cull-distance="0"` = nunca cortar. A faixa de horizonte
(`world/frontier/horizon.xml`) autora `cull-distance="1000"` porque a sua
silhueta é conteúdo de 450–840 m.

### 3. A ladder de LOD estava no XML e era ignorada

Os mundos migrados do VibeGame trazem `lod1-url`, `lod2-url`,
`lod-threshold-near` e `lod-threshold-mid` em 131 `<GLTFLoader>` — o Viber
listava-os como *dropped attrs* e desenhava a malha **hero** de cada pinheiro
a 200 m (`pine_dark_lod0` 1.4 MB vs `lod2` 0.32 MB).

`MeshLod` restaura a ladder. A troca é feita mutando `WorldAssetRoot`: o
`world_instance_spawner` do Bevy reage a `Changed<WorldAssetRoot>`,
despawna a subárvore antiga e spawna a nova — só **um** tier fica residente
por instância. Há histerese de 8% e um orçamento de 24 trocas por frame, para
uma viagem rápida não tentar re-tierar 6000 props num só frame.

Criaturas ficam de fora da ladder de propósito: trocar a subárvore
re-spawnaria o `AnimationPlayer` a meio de um clip.

### 4. `render-distance` do terreno cobria quase o mapa todo

Sem `render-distance` autorada, `effective_render_distance()` devolve o raio
que cabe `DEFAULT_RESIDENT_CHUNK_BUDGET` (2048) colunas — **1634 m** num mundo
de 4 km com colunas de 64 m. O far plane da câmara é ~1000 m, portanto metade
das colunas eram entidades que nunca chegavam ao ecrã mas pagavam
visibilidade, LOD e streaming de colliders todos os frames.

`world.xml` passou a autorar `render-distance="950"` → **689 colunas voxel
(900 caixas surface-nets)**.

## Terreno 100% volumétrico (custos próprios)

O terreno inteiro sai do campo voxel por surface nets, com ladder de LOD por
coluna (célula 1→2→4 m). Custos medidos (`chunk_build_bench`, release):

```
caixa 32³ @1 m (LOD0): ~5,8 ms   ← budget de 4 caixas/frame no rebuild
caixa 64³ @2 m (LOD1): ~7,2 ms
caixa 16³ @4 m (LOD2): ~1,6 ms
```

No `simple-rpg` (4 km, QA pós-migração): boot spawna 689 colunas / 900 caixas,
~89–103 fps de média — paridade com o ladder heightfield anterior. Os
colliders de terreno são todos `Collider::voxels` das caixas, streamados num
raio de 3 chunk-edges com histerese; `collision-resolution="0"` desliga-os.

## VRAM: as texturas eram PNG

169 GLBs do `simple-rpg` traziam texturas **PNG** (85 delas 2048x2048) — sem
`KHR_texture_basisu`. Um PNG é comprimido em disco e **descomprimido na VRAM**:
o GPU guarda RGBA8, 4 bytes por texel.

```
texturas em RGBA8 + mips : ~2121 MiB   ← 2.1 dos 2.8 GiB medidos
as mesmas em KTX2 → BC7  :  ~530 MiB
```

Medido depois de converter os GLBs e as texturas soltas: **2817 → 1340 MiB**,
sem uma única alteração às malhas ou aos materiais.

Estado final (verificado pelo próprio script): **559 GLBs de runtime com
textura, 100% KTX2, todos com supercompressão Zstd e cadeia de mipmaps
completa** (12 níveis a 2048², 11 a 1024², 10 a 512²), mais 189 texturas
soltas convertidas no `simple-rpg` e no pool.

`scripts/ktx2_compress_pool.py` converte um pool para KTX2/UASTC,
verificando cada ficheiro antes de substituir o original:

```bash
# GLBs (texturas embutidas) — substitui in-place
python3 Viber/scripts/ktx2_compress_pool.py --assets Viber/examples/simple-rpg/assets
# + texturas soltas (.png/.jpg/.webp) → ficheiro .ktx2 irmão
python3 Viber/scripts/ktx2_compress_pool.py --assets ... --loose
python3 Viber/scripts/ktx2_compress_pool.py --assets ... --dry-run
```

Duas exclusões deliberadas:

* **`_intermediate/`** fica de fora (`--include-intermediate` força). São
  entradas do pipeline (`rigging3d`, `animator3d`, `text3d lod`) lidas por
  bpy, que não lê KTX2 sem passar por
  `aigamekit_shared.gltf_decode.bpy_readable_glb`. Nunca chegam ao GPU:
  encodá-las não poupa um byte de VRAM e mete um round-trip lossy de UASTC
  em tudo o que for gerado a partir delas.
* **heightmaps e `images/`** — o primeiro é *dados* (um codec lossy destrói o
  terreno), o segundo é arte de referência que o motor nunca carrega.

As texturas soltas saem como `.ktx2` ao lado do original e as referências do
mundo (`texture=`, `src=`) passam a apontar para elas; normal/roughness/AO
são encodados como **linear** e o resto como **sRGB**. O `patch_image` em
`src/textures.rs` aplica o sampler REPEAT + anisotropia 8 *antes* de sair nos
formatos comprimidos, por isso o chão tiled continua correcto — mas o `.ktx2`
tem de trazer os seus próprios mipmaps (`--generate-mipmap`), porque a
geração de mips na engine só corre para RGBA8. O verificador do script recusa
qualquer `.ktx2` acima de 1x1 que venha com `levelCount = 1`.

Aviso ganho a doer: os campos do header KTX2 são `uint32` corridos a seguir
aos 12 bytes do identificador — `levelCount` está no **byte 40** e
`supercompressionScheme` no **44**. Lê-los nos offsets errados não dá erro
nenhum: dá "sem supercompressão" para todos os ficheiros, o que deixaria
passar precisamente o BasisLZ que o Bevy não carrega.

Tem de ser `uastc`, nunca `etc1s`: o Bevy 0.19 descomprime só ZLIB e Zstd, e
o ETC1S vem em BasisLZ. UASTC sem supercompressão (`scheme = 0`) serve — a
feature `basis-universal` transcodifica o *formato* para BC7 depois, que é de
onde vem a poupança.

O pool partilhado (`examples/shared-assets/public`) merece o mesmo tratamento;
o script aceita qualquer raiz de assets.

## Como medir

```bash
viber session up                       # porta livre automática
viber debug prof --samples 10          # média/pior/melhor — NÃO uma amostra só
viber debug lua 'return viber.debug.stats()'   # meshes/colliders/luzes com sombra
viber debug lua 'return viber.debug.physics()' # tempos do step do Rapier
```

O campo `fps` de **uma** amostra é instantâneo e oscila brutalmente enquanto o
terreno faz streaming (12 e 60 fps no mesmo mundo, com segundos de diferença).
`--samples` existe por causa disso.

Para isolar o efeito do culling + ladder sem trocar de binário:

```bash
VIBER_RENDER_LOD=0 viber run world.xml --no-cargo --bridge 15711
```

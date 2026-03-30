---
name: materialize-cli
description: Gera mapas PBR (height, normal, metallic, smoothness, edge, AO) a partir de texturas difusas via compute shaders wgpu. Use com Materialize CLI, baking PBR, pipeline diffuse→material, WGSL, texture2d.materialize no GameAssets, ou quando o utilizador mencionar materialize-cli ou mapas PBR a partir de imagem.
---

# Materialize CLI

## Quando usar

- Explicar o **CLI** (argumentos, ficheiros de saída, formatos).
- Alterar ou adicionar um **mapa PBR** (shader WGSL + pipeline Rust + I/O).
- Depurar ou estender o pipeline **GPU** (wgpu, bind groups, formatos).
- Escrever ou atualizar documentação (`README`, `docs/`).

## Visão geral para utilizadores

**Comando:** `materialize <entrada> [opções]`

**Saídas (6 mapas):** `{stem}_height.*`, `_normal.*`, `_metallic.*`, `_smoothness.*`, `_edge.*`, `_ao.*`

**Opções:** `-o` / `--output` (pasta), `-f` / `--format` (png \| jpg \| tga \| exr), `-p` / `--preset` (default \| skin \| floor \| metal \| fabric \| wood \| stone), `-q` / `--quality` (0–100, JPEG), `-v` / `--verbose`, `--quiet` (sem listar ficheiros em sucesso).

**Presets de material:** Use `-p` para otimizar para o tipo de textura:
- `default` — Uso geral (comportamento original)
- `skin` — Pele humana/personagem (metallic zero, normals suaves)
- `floor` — Chão/piso (height forte, AO profundo)
- `metal` — Superfícies metálicas (metallic boost, edges nítidos)
- `fabric` — Tecido/roupa (matte, sem metallic)
- `wood` — Madeira (detalhe de grão moderado)
- `stone` — Pedra/rocha (muito áspero, AO forte)

```bash
materialize textura.png -o ./out/ -v
materialize skin.png -p skin -o ./out/
materialize stone_floor.png --preset floor -v
materialize diffuse.png --format png --quiet
materialize skill install   # instala esta skill em .cursor/skills/ do projeto atual
```

## Integração com GameAssets / Texture2D

- **GLB 3D:** **Hunyuan3D-Paint 2.1** (`paint3d texture`) já exporta GLB PBR; não uses Materialize no mesh.
- **Imagem difusa:** com **`texture2d.materialize: true`** no `game.yaml`, o GameAssets chama `materialize <png> -o …` após `texture2d generate`.
- Variável de ambiente: **`MATERIALIZE_BIN`** se o executável não estiver no `PATH`.

## Layout do código (referência para contribuidores)

| Área | Caminho | Papel |
|------|---------|--------|
| Shaders | `src/shaders/*.wgsl` | Um compute shader por mapa; workgroup 8×8 |
| Pipeline | `src/pipeline.rs` | Ordem: height → normal → metallic → smoothness → edge → AO |
| GPU | `src/gpu.rs` | Pipelines 1 ou 2 inputs, bind groups |
| I/O | `src/io.rs` | `get_output_paths`, conversões, `save_image` |
| Presets | `src/preset.rs` | Enum `Preset`, struct `PresetParams` (GPU uniform), valores por tipo |
| CLI | `src/cli.rs` | Args clap; enums `OutputFormat`, `Preset` |
| Main | `src/main.rs` | Imagem → `pipeline.process()` → gravar os 6 mapas |

## Dependências entre mapas

- **Height:** a partir do diffuse.
- **Normal:** a partir do height.
- **Metallic:** a partir do diffuse.
- **Smoothness:** a partir do diffuse + metallic (pipeline a 2 entradas).
- **Edge:** a partir do normal.
- **AO:** a partir do height.

## Adicionar ou alterar um mapa

1. **Shader:** `src/shaders/<nome>.wgsl` com `@group(0) @binding(0)` entrada e `@binding(1)` storage output (ou 0,1,2 para 2 entradas). `@group(1) @binding(0)` uniform `Params` (preset). Workgroup 8×8. Guardar com `coords >= dims` → return.
2. **Pipeline:** em `pipeline.rs`, `include_str!`, criar pipeline, textura de saída, bind group, dispatch após dependências, readback para `PbrMaps`.
3. **I/O:** em `io.rs`, estender `OutputPaths`, `get_output_paths`, `*_to_image`, gravar em `main.rs`.
4. **Formatos:** height em R32Float onde aplicável; restantes conforme comentários em `io.rs` / `main.rs`.

## Testes

```bash
cargo build
cargo test
materialize /caminho/diffuse.png -o ./out/ -v
```

Testes de integração: `tests/integration_test.rs`. Unitários em `io::tests`.

## Documentação

- **Utilizador:** `README.md`, `docs/README.md`, `docs/features.md`, `docs/cli-api.md`
- **Técnico:** `docs/architecture.md`, `docs/algorithms.md`, `docs/shaders.md`
- **Planeamento:** `docs/roadmap.md`, `docs/plans/*.md`

Manter docs alinhados ao adicionar mapas ou opções CLI.

## Ferramentas relacionadas

| Ferramenta | Ligação |
|------------|---------|
| **Text3D / Paint3D** | PBR no GLB vem do Paint 2.1; ver `Text3D/docs/PBR_MATERIALIZE.md`. |
| **GameAssets** | Materialize só com **`texture2d.materialize`** (mapas a partir da difusa). |

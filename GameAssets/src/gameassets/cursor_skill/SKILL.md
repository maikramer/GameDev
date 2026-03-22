---
name: gameassets
description: Orquestra batches de prompts e assets 2D/3D com game.yaml, manifest CSV e presets. Use quando o utilizador falar em GameAssets, manifest, game.yaml, batch de imagens, presets locais, TEXT2D_BIN, TEXT3D_BIN, MATERIALIZE_BIN, path_layout flat, ou integração Text2D/Text3D/Materialize.
---

# GameAssets — batch de prompts e assets

## Quando usar

- Gerar **várias** imagens e/ou GLBs a partir de um **CSV** alinhado ao estilo do jogo.
- Trabalhar com **`game.yaml`** + **`manifest.csv`** + **`presets.yaml`** (e opcionalmente **`presets-local.yaml`**).
- Integrar **Text2D** (2D) e **Text3D** (3D) sem escrever comandos à mão para cada linha.

## O que é

CLI que combina **perfil** (`game.yaml`), **manifest** (`manifest.csv`) e **presets** de estilo, e chama `text2d` e, se pedires, `text3d` em subprocess. O batch constrói prompts com `prompt_builder` e executa o equivalente a `text2d generate … -o` e `text3d generate --from-image … -o` com opções do perfil.

## Pré-requisitos

| Componente | Notas |
|------------|--------|
| Python | 3.10+ |
| `text2d` | No `PATH` ou `TEXT2D_BIN` |
| `text3d` | Com `--with-3d`: no `PATH` ou `TEXT3D_BIN` |
| Materialize (opcional) | Se `text3d.materialize: true` no YAML: binário `materialize` no `PATH` ou `MATERIALIZE_BIN` (o Text3D invoca-o para PBR no GLB) |

## Pipeline mental

```text
game.yaml + manifest.csv + presets [+ presets-local.yaml]
        → text2d generate (por linha)
        → [se generate_3d e --with-3d] text3d generate --from-image …
              → [se materialize no perfil] Materialize CLI (via Text3D)
```

## Comandos principais

| Comando | Função |
|---------|--------|
| `gameassets init [--path DIR]` | Cria `game.yaml` e `manifest.csv` de exemplo |
| `gameassets prompts [--profile …] [--manifest …]` | Pré-visualiza prompts (sem GPU); `-o ficheiro.jsonl` grava JSONL |
| `gameassets batch [--profile …] [--manifest …]` | Gera imagens; `--with-3d` gera GLB quando `generate_3d=true` |
| `gameassets skill install` | Instala esta skill em `.cursor/skills/gameassets/` do projeto alvo |

**Flags úteis em `batch`:** `--dry-run` (ver comandos), `--fail-fast`, `--log run.jsonl` (registo JSONL por asset).

## Preset só no teu ficheiro (`presets-local.yaml`)

Se `style_preset` (ex.: `galaxy_orbital`) **não** existir em `data/presets.yaml` e estiver apenas no teu YAML local, **é obrigatório**:

```bash
gameassets batch --profile game.yaml --manifest manifest.csv --with-3d \
  --presets-local presets-local.yaml --log run.jsonl
```

Sem `--presets-local`, o comando falha com **preset desconhecido**.

## Perfil (`game.yaml`) — resumo

- **`style_preset`**, **`output_dir`**, **`path_layout`**: `split` (pastas `images/` e `meshes/`) ou **`flat`** (PNG e GLB na mesma árvore; usa `id` com barra, ex. `Props/caixa_01`).
- **`text2d`**: `low_vram`, `cpu`, `width`, `height` (resolução 2D).
- **`text3d`**: `preset` (`fast` \| `balanced` \| `hq`), `low_vram`, `texture` (omitido = **`true`**), ou **Hunyuan explícito** (`steps`, `octree_resolution`, `num_chunks` — nesse caso não se passa `--preset` ao CLI), `no_mesh_repair`, `mesh_smooth`, `mc_level`.
- **PBR (opcional):** `materialize`, `materialize_save_maps`, `materialize_maps_subdir`, `materialize_bin`, `materialize_no_invert` — ver `Text3D/docs/PBR_MATERIALIZE.md`.

**Atenção:** `text3d.low_vram: true` em GPU faz o Hunyuan “shape” cair para CPU e **costuma degradar a forma**; preferir reduzir resolução 2D ou fechar outras apps que consumam VRAM.

## Manifest (`manifest.csv`)

Colunas incluem **`id`**, **`idea`**, **`generate_3d`**, etc. (ver `manifest.py`). Com `path_layout: flat`, `id` pode ser `Categoria/nome` para espelhar pastas no Godot.

## Variáveis de ambiente

| Variável | Função |
|----------|--------|
| `TEXT2D_BIN` | Caminho ao executável `text2d` se não estiver no `PATH` |
| `TEXT3D_BIN` | Idem para `text3d` |
| `MATERIALIZE_BIN` | Idem para `materialize` quando usas PBR via perfil |

## Armadilhas frequentes

| Sintoma | O que verificar |
|---------|------------------|
| `Preset desconhecido` | Passar `--presets-local` com o YAML onde o preset está definido. |
| GLB não gerado | `batch` sem `--with-3d` **nunca** corre 3D; ou `generate_3d` falso na linha. |
| OOM no Text2D | Reduzir resolução em `text2d` no perfil; fechar **Godot** ou outros processos na GPU (`nvidia-smi`). |
| Qualidade 3D pior que no Text3D “isolado” | Mesmo `game.yaml` e mesmas flags; comparar preset/steps e VRAM livre. |

## Ferramentas relacionadas (monorepo GameDev)

| Ferramenta | Papel |
|------------|--------|
| **Text2D** | Geração da imagem base por prompt. |
| **Text3D** | Imagem → mesh + Paint + opcional Materialize. |
| **Materialize** | Mapas PBR a partir do diffuse (também acoplado ao Text3D com `--materialize`). |

## Referências no repositório

- `src/gameassets/cli.py` — CLI e epílogo de exemplos
- `src/gameassets/prompt_builder.py` — composição de prompts
- `src/gameassets/data/presets.yaml` — presets base
- `README.md` — documentação de utilizador

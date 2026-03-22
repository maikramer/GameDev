# GameAssets — desenho (2026-03-21)

## Objetivo

Pacote Python `GameAssets` na raiz do monorepo com CLI (`gameassets`) para:

- Perfil de jogo (`game.yaml`): título, género, tom, preset de estilo, negativos extra, pastas de saída, seed opcional, bloco opcional `text3d`.
- Manifest CSV (`id`, `idea`, opcional `kind`, `generate_3d`).
- Presets YAML embutidos (`src/gameassets/data/presets.yaml`) com merge opcional de `presets.local.yaml` via `--presets-local`.
- Comandos: `init`, `prompts` (sem GPU), `batch` (subprocess `text2d` / `text3d`).

## Fluxo 2D → 3D

1. `text2d generate PROMPT -o outputs/.../id.png` (e `--seed` se `seed_base` no perfil).
2. Se `--with-3d` e `generate_3d=true`: `text3d generate --from-image ... -o .../id.glb` com opções do bloco `text3d` do perfil.

Sem `--with-3d`, o 3D nunca corre (aviso se o CSV pede 3D).

## UX

- `TEXT2D_BIN` / `TEXT3D_BIN` para caminhos absolutos.
- `--dry-run`, `--fail-fast`, `--log` JSONL por asset.
- Erros explícitos se binários não forem encontrados.

## Testes

Unitários: `prompt_builder`, `manifest` (CSV).

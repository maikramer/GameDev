#!/usr/bin/env bash
# Gera props 3D clássicos de RPG (Text2D + Text3D + Paint3D) e faz handoff para public/assets.
# Uso: a partir desta pasta, com GPU; GameAssets/Text2D/Text3D/Paint3D no PATH.
#
# Variáveis opcionais:
#   SKIP_TEXT2D=1     — não regerar PNGs (requer images/*.png para cada id do manifest_classic).
#   SKIP_STONE_RETRY=1 — não voltar a correr só o poço se stone_well.glb faltar após o batch.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"

MAIN_MANIFEST="manifest.csv"
EXTRA_MANIFEST="manifest_classic.csv"
FULL_MANIFEST="manifest_full.csv"
STONE_ONLY="manifest_stone_well_only.csv"

if [[ ! -f "$MAIN_MANIFEST" ]]; then
  echo "Falta $MAIN_MANIFEST" >&2
  exit 1
fi
if [[ ! -f "$EXTRA_MANIFEST" ]]; then
  echo "Falta $EXTRA_MANIFEST" >&2
  exit 1
fi

{
  head -1 "$MAIN_MANIFEST"
  tail -n +2 "$MAIN_MANIFEST"
  tail -n +2 "$EXTRA_MANIFEST"
} > "$FULL_MANIFEST"

BATCH_FLAGS=(--profile game.yaml --manifest "$EXTRA_MANIFEST" --with-3d --skip-audio --skip-batch-lock)
if [[ "${SKIP_TEXT2D:-0}" == "1" ]]; then
  BATCH_FLAGS+=(--skip-text2d)
  echo "==> Batch (linhas de $EXTRA_MANIFEST) — 3D, sem 2D, sem áudio"
else
  echo "==> Batch (linhas de $EXTRA_MANIFEST) — Text2D + 3D, sem áudio"
fi
# Ajuda a evitar falhas intermitentes do Paint3D (cusolver) após longas sessões GPU.
export PYTORCH_CUDA_ALLOC_CONF="${PYTORCH_CUDA_ALLOC_CONF:-expandable_segments:True}"
gameassets batch "${BATCH_FLAGS[@]}"

MESHW="meshes/stone_well.glb"
if [[ ! -f "$MESHW" && "${SKIP_STONE_RETRY:-0}" != "1" && -f "$STONE_ONLY" ]]; then
  echo "==> Retentar só stone_well (manifest $STONE_ONLY)"
  gameassets batch --profile game.yaml --manifest "$STONE_ONLY" --with-3d --skip-audio --skip-text2d --skip-batch-lock
fi

echo "==> Handoff (manifest completo: hero + áudio + props clássicos)"
gameassets handoff \
  --profile game.yaml \
  --manifest "$FULL_MANIFEST" \
  --public-dir ../public \
  --with-textures

echo "OK: modelos em ../public/assets/models/ e gameassets_handoff.json actualizado."

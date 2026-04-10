#!/usr/bin/env bash
# Gera props 3D clássicos de RPG (Text3D + Paint3D) e faz handoff para public/assets.
# Uso: a partir desta pasta, com GPU e Text3D no PATH; monorepo com GameAssets instalado.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"

MAIN_MANIFEST="manifest.csv"
EXTRA_MANIFEST="manifest_classic.csv"
FULL_MANIFEST="manifest_full.csv"

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

echo "==> Batch (só linhas de $EXTRA_MANIFEST) — 3D, sem áudio"
gameassets batch \
  --profile game.yaml \
  --manifest "$EXTRA_MANIFEST" \
  --with-3d \
  --skip-audio

echo "==> Handoff (manifest completo: hero + áudio + props clássicos)"
gameassets handoff \
  --profile game.yaml \
  --manifest "$FULL_MANIFEST" \
  --public-dir ../public \
  --with-textures

echo "OK: modelos em ../public/assets/models/ e gameassets_handoff.json actualizado."

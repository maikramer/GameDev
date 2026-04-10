#!/usr/bin/env bash
# Regenera música + SFX extra do simple-rpg (Text2Sound, perfil music — Open 1.0).
# Uso: na raiz do monorepo, com Text2Sound/.venv activo e HF_TOKEN se necessário.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/public/assets/audio"
T2S_ROOT="$(cd "$ROOT/../../../Text2Sound" && pwd)"
# shellcheck source=/dev/null
source "$T2S_ROOT/.venv/bin/activate"
mkdir -p "$OUT"
text2sound generate "short playful cartoon jump whoosh single impact game sfx" --profile music -d 3 -o "$OUT/sfx_jump.wav" --seed 101
text2sound generate "soft positive UI confirm chime gentle bell game menu save success" --profile music -d 3 -o "$OUT/sfx_save.wav" --seed 102
text2sound generate "brief magical UI shimmer sparkle light fantasy game load restore" --profile music -d 3 -o "$OUT/sfx_load.wav" --seed 103
text2sound generate "Calm RPG exploration background music, orchestral strings and soft synth pads, bright fantasy adventure, peaceful green fields, loopable mood, light percussion, no heavy drums, suitable for a cute lowpoly game" --profile music -d 40 -s 80 -c 6 -o "$OUT/bgm_field.wav" --seed 2026

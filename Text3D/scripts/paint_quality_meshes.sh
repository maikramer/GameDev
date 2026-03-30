#!/usr/bin/env bash
# Textura nos GLBs *quality* com as refs em outputs/refs/ (mesmos prompts Text2D).
# Requer: Paint3D (paint3d no PATH ou PAINT3D_BIN); ver Paint3D/docs/PAINT_SETUP.md

set -euo pipefail
export PYTORCH_CUDA_ALLOC_CONF="${PYTORCH_CUDA_ALLOC_CONF:-expandable_segments:True}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAINT="${PAINT3D_BIN:-paint3d}"
M="${ROOT}/outputs/meshes"
R="${ROOT}/outputs/refs"

"$PAINT" texture "$M/robot_quality.glb"  -i "$R/robot_ref.png"  -o "$M/robot_quality_textured.glb"
"$PAINT" texture "$M/car_quality.glb"     -i "$R/car_ref.png"     -o "$M/car_quality_textured.glb"
"$PAINT" texture "$M/plant_quality.glb"   -i "$R/plant_ref.png"   -o "$M/plant_quality_textured.glb"
echo "Concluído: *_quality_textured.glb"

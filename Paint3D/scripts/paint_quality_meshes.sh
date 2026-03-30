#!/usr/bin/env bash
# Pinta os 3 GLBs *quality* usando as refs em outputs/refs/ (mesmos prompts Text2D).
# Requer: custom_rasterizer + CUDA_HOME (ver docs/PAINT_SETUP.md)

set -euo pipefail
export PYTORCH_CUDA_ALLOC_CONF="${PYTORCH_CUDA_ALLOC_CONF:-expandable_segments:True}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
P3D="${ROOT}/.venv/bin/paint3d"
M="${ROOT}/outputs/meshes"
R="${ROOT}/outputs/refs"

"$P3D" texture "$M/robot_quality.glb"  -i "$R/robot_ref.png"  -o "$M/robot_quality_textured.glb"
"$P3D" texture "$M/car_quality.glb"     -i "$R/car_ref.png"     -o "$M/car_quality_textured.glb"
"$P3D" texture "$M/plant_quality.glb"   -i "$R/plant_ref.png"   -o "$M/plant_quality_textured.glb"
echo "Concluído: *_quality_textured.glb"

#!/bin/bash
# Reprocess all decimated GLBs: Paint3D (all), Rigging3D + Animator3D (hero)
set -euo pipefail

BASE="/media/maikeu/b1e73891-ddde-49a0-9382-903accb68b49/GitClones/GameDev"
MESHES="$BASE/VibeGame/examples/simple-rpg/public/assets/meshes"
IMAGES="$BASE/VibeGame/examples/simple-rpg/public/assets/images"
PAINT3D="$BASE/Paint3D/.venv/bin/paint3d"
RIGGING3D="$BASE/Rigging3D/.venv/bin/rigging3d"
ANIMATOR3D="$BASE/Animator3D/.venv/bin/animator3d"

# Common paint3d flags
PFLAGS="--preserve-origin --allow-shared-gpu -v"

run() {
    local name="$1"
    local mesh="$2"
    local img="$3"
    local out="${4:-$mesh}"
    echo ""
    echo "============================================"
    echo "[PAINT] $name"
    echo "  mesh: $mesh"
    echo "  img:  $img"
    echo "  out:  $out"
    echo "============================================"
    "$PAINT3D" texture "$mesh" -i "$img" -o "$out" $PFLAGS
    echo "[DONE] $name ✓"
}

echo "=== Pipeline: Paint3D (all) + Rigging3D + Animator3D (hero) ==="
echo "Start: $(date)"

# -------------------------------------------------------------------
# Phase 1: Paint all static props
# -------------------------------------------------------------------
run "wooden_crate"   "$MESHES/wooden_crate.glb"   "$IMAGES/wooden_crate.png"
run "crystal_blue"   "$MESHES/crystal_blue.glb"   "$IMAGES/crystal_blue.png"
run "stone_pillar"   "$MESHES/stone_pillar.glb"   "$IMAGES/stone_pillar.png"
run "rock_boulder"   "$MESHES/rock_boulder.glb"    "$IMAGES/rock_boulder.png"
run "milestone_stone" "$MESHES/milestone_stone.glb" "$IMAGES/milestone_stone.png"
run "stone_well"     "$MESHES/stone_well.glb"      "$IMAGES/stone_well.png"
run "torch_stand"    "$MESHES/torch_stand.glb"     "$IMAGES/torch_stand.png"
run "treasure_chest" "$MESHES/treasure_chest.glb"  "$IMAGES/treasure_chest.png"
run "wooden_barrel"  "$MESHES/wooden_barrel.glb"   "$IMAGES/wooden_barrel.png"
run "bush_small"     "$MESHES/bush_small.glb"      "$IMAGES/bush_small.png"
run "hay_bale"       "$MESHES/hay_bale.glb"        "$IMAGES/hay_bale.png"

# -------------------------------------------------------------------
# Phase 2: Tree (base + LODs)
# -------------------------------------------------------------------
run "tree_lowpoly"   "$MESHES/tree_lowpoly.glb"    "$IMAGES/tree_lowpoly.png"

# lod0 = identical to base (same md5), just copy
echo "[COPY] tree_lowpoly → tree_lowpoly_lod0"
cp "$MESHES/tree_lowpoly.glb" "$MESHES/tree_lowpoly_lod0.glb"

# Paint LOD variants (different geometry, need separate bake)
run "tree_lowpoly_lod1" "$MESHES/tree_lowpoly_lod1.glb" "$IMAGES/tree_lowpoly.png"
run "tree_lowpoly_lod2" "$MESHES/tree_lowpoly_lod2.glb" "$IMAGES/tree_lowpoly.png"

# -------------------------------------------------------------------
# Phase 3: Hero — Paint → Rig → Animate
# -------------------------------------------------------------------
run "hero" "$MESHES/hero.glb" "$IMAGES/hero.png"

echo ""
echo "============================================"
echo "[RIG] hero → hero_rigged"
echo "============================================"
"$RIGGING3D" pipeline -i "$MESHES/hero.glb" -o "$MESHES/hero_rigged.glb"
echo "[DONE] hero_rigged ✓"

echo ""
echo "============================================"
echo "[ANIMATE] hero_rigged → hero_animated (humanoid)"
echo "============================================"
"$ANIMATOR3D" game-pack "$MESHES/hero_rigged.glb" "$MESHES/hero_animated.glb" --preset humanoid
echo "[DONE] hero_animated ✓"

# Also generate the Y-up variant if needed
echo ""
echo "============================================"
echo "[COPY] hero_animated → hero_animated_yup"
echo "============================================"
cp "$MESHES/hero_animated.glb" "$MESHES/hero_animated_yup.glb"

echo ""
echo "=== Pipeline complete! ==="
echo "End: $(date)"
echo ""
echo "GLBs in $MESHES:"
ls -lh "$MESHES/"*.glb

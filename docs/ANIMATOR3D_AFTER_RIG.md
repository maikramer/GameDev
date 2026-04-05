# Animator3D after Rigging3D (optional animation pass)

[Rigging3D](../Rigging3D/) produces a **rigged GLB** from the GameAssets batch (`--with-rig`). [Animator3D](../Animator3D/) runs **Blender (bpy)** to add clips or export animation. It is **not** invoked by `gameassets batch`; run it as a **manual step** on the rigged file, then copy the result into your web `public/assets/models/`.

## Prerequisites

- Rigged GLB (e.g. `hero_rigged.glb` from the batch, suffix from `rigging3d.output_suffix` in `game.yaml`).
- Animator3D installed: `./install.sh animator3d` from the repo root ([INSTALLING.md](INSTALLING.md)).

## Example: procedural wave idle clip

From a directory that contains the rigged GLB:

```bash
animator3d wave-idle hero_rigged.glb hero_animated.glb --frames 60
```

Inspect bones if needed:

```bash
animator3d inspect hero_rigged.glb --json-out
```

## Handoff to VibeGame

1. Copy `hero_animated.glb` to `public/assets/models/` (see [MONOREPO_GAME_PIPELINE.md](MONOREPO_GAME_PIPELINE.md)).
2. Load with `loadGltfToScene` or the declarative `<gltf-load url="/assets/models/hero_animated.glb"></gltf-load>` recipe (VibeGame).

## See also

- [Animator3D README](../Animator3D/README.md)
- [MONOREPO_GAME_PIPELINE.md](MONOREPO_GAME_PIPELINE.md)

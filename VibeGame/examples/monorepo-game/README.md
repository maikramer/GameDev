# Monorepo game bridge example

Demonstrates loading a **GLB** produced by the GameDev pipeline (Text3D, Paint3D, Rigging3D, etc.) into a VibeGame scene using **`loadGltfToScene`** from `vibegame`.

## Prerequisites

- Build the **vibegame** package once from the repo root: `cd VibeGame && bun install && bun run build`
- Or use the published package by changing `vibegame` in `package.json` to a version range instead of `file:../..`

## Run

```bash
cd VibeGame/examples/monorepo-game
bun install
bun run dev
```

## GLB handoff

1. Run `gameassets batch …` (or `text3d` / `paint3d`) and locate the output GLB.
2. Copy it to `public/assets/models/hero.glb` (or adjust the path in `src/main.ts`).
3. Reload the dev server — the mesh appears on top of the declarative ground plane.

Full layout and pipeline: [docs/MONOREPO_GAME_PIPELINE.md](../../../docs/MONOREPO_GAME_PIPELINE.md).

## API

- `loadGltfToScene(state, url)` — see [`vibegame` exports](https://github.com/maikramer/GameDev/tree/main/VibeGame/src/extras/gltf-bridge.ts) / `import { loadGltfToScene } from 'vibegame'`.

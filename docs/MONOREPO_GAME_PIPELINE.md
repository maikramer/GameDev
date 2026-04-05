# Monorepo Game Pipeline — from GameAssets to VibeGame

This document defines a **reference layout** and **handoff contract** between the asset-generation tools (centered on [GameAssets](../GameAssets/)) and a **browser runtime** ([VibeGame](../VibeGame/)). It complements [INSTALLING.md](INSTALLING.md).

## 1. Roles

| Layer | Tools | Output |
|--------|--------|--------|
| Orchestration | `gameassets` | Per-row images, GLBs, audio paths under a chosen `output_dir` |
| 2D / maps | Text2D, Texture2D, Materialize, Skymap2D | PNG, PBR map folders, equirectangular sky |
| 3D | Text3D, Paint3D, Part3D, Rigging3D | GLB (mesh + PBR; optional parts / rig) |
| Audio | Text2Sound | WAV/FLAC (see Text2Sound docs) |
| QA | GameDevLab | GLB inspection (optional) |
| Runtime | VibeGame | Interactive scene (ECS + Three.js) |

## 2. Recommended folder layout (handoff)

After batch generation, normalize assets into a **web project** tree (Vite `public/` or static host):

```text
my-game/
  public/
    assets/
      models/          # GLB from Text3D / Paint3D / Rigging3D (copy or symlink)
      textures/        # Optional: loose PNGs from Text2D / Texture2D
      audio/           # Text2Sound outputs
      sky/             # Skymap2D equirectangular EXR/PNG (if used as env map)
  src/
    main.ts
  index.html
```

**Naming convention (suggested):**

- `assets/models/<id>.glb` — one file per manifest row or logical prop.
- `assets/audio/<id>.<ext>` — matches CSV `id` or slug.
- `assets/sky/<name>.png` — skybox source; how you bind it in Three.js/VibeGame is up to your scene (VibeGame’s declarative layer does not yet ship a skybox recipe tied to this path — you may set scene background in code or extend the world XML later).

## 3. Web contract (minimum)

For the runtime to load content **without** a custom CMS:

| Asset type | URL pattern (dev) | Notes |
|------------|-------------------|--------|
| GLB | `/assets/models/<name>.glb` | Served as static files; load with `loadGltfToScene` from `vibegame` or Three.js `GLTFLoader` |
| Audio | `/assets/audio/<name>.wav` | Use Web Audio or `<audio>`; not wired by VibeGame core — integrate in your game code |
| Sky | `/assets/sky/<name>.png` | Use as `THREE.Texture`, `Scene.background`, or PMREM — manual bridge |

**Environment variables (`*_BIN`)** apply to **CLI batch** tools only, not to the browser. The browser only sees **HTTP URLs**.

## 4. Reference pipeline (minimal game)

1. **Install CLIs** (repo root): `./install.sh` for the tools you need (see [INSTALLING.md](INSTALLING.md)); include `gameassets`, `text2d`/`texture2d`, `text3d`, optional `paint3d`, `text2sound`, `vibegame`, etc.
2. **Author** `game.yaml` + `manifest.csv` + presets ([GameAssets README](../GameAssets/README.md)).
3. **Batch**: `gameassets batch --profile game.yaml --manifest manifest.csv --with-3d` (add `--with-rig`, `--with-parts`, audio columns as needed).
4. **Copy** generated GLBs/audio into `public/assets/...` of a Vite project (see [VibeGame/examples/monorepo-game](../VibeGame/examples/monorepo-game/)), or use **`gameassets handoff --public-dir path/to/public`** to copy/symlink from the profile `output_dir` and write `assets/gameassets_handoff.json` (URLs per row).
5. **Run** the web app: `bun dev` / `npm run dev`; GLB loads via `loadGltfToScene` after `run()`, or declaratively with `<gltf-load url="...">` in the world XML. **Skymap2D** equirect PNG/JPG: use `applyEquirectSkyEnvironment` from `vibegame` (PMREM + optional background).

**Animator3D** (bpy animation export) is **not** in `gameassets batch` — see [ANIMATOR3D_AFTER_RIG.md](ANIMATOR3D_AFTER_RIG.md), then copy the exported GLB into `public/assets/models/`.

## 5. Synergy limits (honest scope)

- **GameAssets** is the strongest link for **batch content**; it does not emit a VibeGame project by itself.
- **VibeGame** defaults to **declarative primitives** (XML recipes); **GLB** integration is via the **`loadGltfToScene`** helper and/or your own Three.js code (see example).
- **One-command “full game”** from prompt to shipped build is **out of scope** without additional automation (CI scripts, templates).

## 6. See also

- [VibeGame/examples/monorepo-game/README.md](../VibeGame/examples/monorepo-game/README.md) — runnable bridge example
- [GameAssets cursor skill / batch behavior](../GameAssets/src/gameassets/cursor_skill/SKILL.md)
- [ANIMATOR3D_AFTER_RIG.md](ANIMATOR3D_AFTER_RIG.md) — animation after rigging
- Root [README.md](../README.md) — project map

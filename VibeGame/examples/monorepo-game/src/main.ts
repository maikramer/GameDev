/**
 * Loads an optional GLB from ``public/assets/models/hero.glb`` after the runtime starts.
 * Copy a mesh from Text3D/Paint3D/GameAssets output into that path to see it in-scene.
 */
import { configure, loadGltfToScene, run } from 'vibegame';

async function bootstrap(): Promise<void> {
  configure({ canvas: '#game-canvas' });
  const runtime = await run();
  const state = runtime.getState();

  try {
    await loadGltfToScene(state, '/assets/models/hero.glb');
  } catch (err) {
    console.warn(
      '[monorepo-game] No GLB loaded (optional). Place ``hero.glb`` under ``public/assets/models/``.',
      err
    );
  }
}

void bootstrap();

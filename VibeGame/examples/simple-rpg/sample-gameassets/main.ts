import { configure, run, applyEquirectSkyEnvironment } from 'vibegame';

async function bootstrap(): Promise<void> {
  configure({ canvas: '#game-canvas' });
  const runtime = await run();
  const state = runtime.getState();
  try {
    await applyEquirectSkyEnvironment(state, '/assets/sky/sky.png');
  } catch {
    console.warn('[dream] Sky env map not loaded (optional).');
  }
}

void bootstrap();

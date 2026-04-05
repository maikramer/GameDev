import type { Adapter, Plugin } from '../../core';
import { GltfPending } from './components';
import { setGltfUrl } from './context';
import { gltfLoadRecipe } from './recipes';
import { GltfXmlLoadSystem } from './systems';

export const GltfXmlPlugin: Plugin = {
  recipes: [gltfLoadRecipe],
  systems: [GltfXmlLoadSystem],
  components: {
    gltfPending: GltfPending,
  },
  config: {
    adapters: {
      gltfPending: {
        url: ((entity, value, state) => {
          setGltfUrl(state, entity, value);
        }) as Adapter,
      },
    },
    defaults: {
      gltfPending: {
        loaded: 0,
      },
    },
  },
};

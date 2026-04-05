import type { Recipe } from '../../core';

export const gltfLoadRecipe: Recipe = {
  name: 'gltf-load',
  components: ['transform', 'gltfPending'],
};

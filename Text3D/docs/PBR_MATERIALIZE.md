# PBR completo no GLB — Hunyuan3D-Paint + Materialize

> **Nota:** A funcionalidade de Paint e Materialize PBR foi movida para o pacote **Paint3D**.
> Ver [Paint3D/docs/PBR_MATERIALIZE.md](../../Paint3D/docs/PBR_MATERIALIZE.md).

## Uso rápido

```bash
# Instalar Paint3D
pip install -e ../Paint3D

# Textura + PBR num comando
paint3d texture mesh.glb -i ref.png -o mesh_pbr.glb --materialize

# Fluxo completo em batch: GameAssets com text3d.texture / materialize no game.yaml

# Só PBR (mesh já texturizada)
paint3d materialize-pbr mesh_textured.glb -o mesh_pbr.glb
```

## API Python

```python
from paint3d import apply_hunyuan_paint, apply_materialize_pbr, load_mesh_trimesh, save_glb

mesh = load_mesh_trimesh("mesh.glb")
mesh_tex = apply_hunyuan_paint(mesh, "ref.png")
mesh_pbr = apply_materialize_pbr(mesh_tex)
save_glb(mesh_pbr, "out_pbr.glb")
```

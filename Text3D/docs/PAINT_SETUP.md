# Hunyuan3D-Paint — Setup

> **Nota:** A funcionalidade de Paint foi movida para o pacote **Paint3D**.
> Ver [Paint3D/docs/PAINT_SETUP.md](../../Paint3D/docs/PAINT_SETUP.md).

## Instalação rápida

```bash
cd Paint3D
pip install -e .
paint3d doctor
```

## Uso com Text3D

O **Text3D** gera só a geometria (`text3d generate`). Para textura, corre **depois** o Paint3D (ou usa **GameAssets** com `text3d.texture` no perfil).

```bash
pip install -e ../Paint3D
text3d generate "um robô" -o robo_shape.glb --save-reference-image
# Grava robo_shape_text2d.png junto ao GLB (imagem que condiciona o Hunyuan)
paint3d texture robo_shape.glb -i robo_shape_text2d.png -o robo.glb
```

Para texturizar meshes standalone:

```bash
paint3d texture mesh.glb -i ref.png -o mesh_tex.glb
```

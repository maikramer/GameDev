# PBR: GLB vs texturas

## GLB (Text3D + Paint3D)

Com **Hunyuan3D-Paint 2.1**, o comando `paint3d texture` produz um **GLB com material PBR** (base color e propriedades glTF adequadas ao pipeline 2.1). Não é necessário correr o **Materialize CLI** em cima do mesh 3D.

Fluxo típico no monorepo:

1. `text3d generate --from-image …` (só geometria) **ou** batch em fases no GameAssets.
2. `paint3d texture mesh.glb --image ref.png -o mesh_textured.glb`

## Texturas 2D (Materialize)

O binário **[Materialize](../Materialize)** continua útil para **gerar mapas PBR a partir de uma imagem difusa** (PNG/JPG), por exemplo no fluxo **Texture2D** com `texture2d.materialize: true` no `game.yaml` do GameAssets.

Ver [Materialize/README.md](../Materialize/README.md) e [GameAssets/README.md](../GameAssets/README.md).

## Legado

Versões anteriores encadeavam `paint3d materialize-pbr` ou flags `--materialize` no Paint3D; isso foi removido em favor do PBR nativo do Paint 2.1.

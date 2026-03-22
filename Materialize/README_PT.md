# Materialize CLI

*Versão em português. [English README](README.md).*

CLI em Rust que gera mapas PBR a partir de texturas difusas, usando compute shaders GPU (wgpu).

**Mapas gerados:** Height, Normal, Metallic, Smoothness, Edge, AO (Ambient Occlusion).

**Monorepo GameDev — Text3D:** o pacote Text3D pode invocar este CLI depois do **Hunyuan3D-Paint** para embutir normal, oclusão e metallic-roughness num **GLB**. Ver [`Text3D/docs/PBR_MATERIALIZE.md`](../Text3D/docs/PBR_MATERIALIZE.md) (`text3d generate ... --texture --materialize`).

## Instalação rápida

```bash
git clone https://github.com/seu-user/materialize-cli
cd materialize-cli
./install.sh
```

Requisitos: **Python 3** (instalador) e **Rust** (cargo) para compilar. O instalador coloca o binário em `~/.local/bin/materialize`.

- **Linux/macOS:** `./install.sh` | `./install.sh uninstall` | `./install.sh reinstall`
- **Windows:** `.\install.ps1` ou `install.bat`

## Uso

```bash
materialize texture.png -o ./out/ -v
# Gera: texture_height.png, texture_normal.png, texture_metallic.png,
#       texture_smoothness.png, texture_edge.png, texture_ao.png

materialize diffuse.png --format png --quiet   # sem listar arquivos no stdout

# Instalar a skill do Cursor no projeto atual (.cursor/skills/materialize-cli)
materialize skill install
```

## Documentação

Veja [docs/README.md](docs/README.md) para visão geral, instalação detalhada e links para arquitetura, algoritmos e roadmap.

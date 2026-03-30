# Materialize CLI

A **Rust CLI** that generates PBR (Physically Based Rendering) maps from diffuse textures using GPU compute shaders via [wgpu](https://wgpu.rs/). No GUI, no Unity — just one command and six maps.

Inspired by the original [Materialize](https://github.com/BoundingBoxSoftware/Materialize) (Unity/Windows). For a Portuguese version, see [README_PT.md](README_PT.md).

**Who is this for?** Game developers, 3D artists, and anyone who needs PBR maps from diffuse textures — in engines like Unity, Unreal, Godot, or in Blender, without running a GUI or the Windows-only original.

**GameDev monorepo — Text3D:** the **Text3D** package can call this CLI after **Hunyuan3D-Paint** to embed normal, occlusion, and metallic-roughness in a **GLB** (glTF 2.0). See [`Text3D/docs/PBR_MATERIALIZE.md`](../Text3D/docs/PBR_MATERIALIZE.md) (`text3d generate ... --texture --materialize`).

---

## Generated maps

From a single diffuse/albedo image, the tool outputs six maps:

| Map | Description |
|-----|-------------|
| **Height** | Surface elevation for parallax/displacement |
| **Normal** | Surface normals for lighting |
| **Metallic** | Metallic vs dielectric mask |
| **Smoothness** | Roughness/smoothness (base + metallic contribution) |
| **Edge** | Edge detection derived from normals |
| **AO** | Ambient occlusion (cavity-style from height) |

---

## Features

- **Minimal** — Command-line only; easy to script and automate
- **Fast** — GPU compute shaders (wgpu); no CPU-bound image loops
- **Cross-platform** — Linux, macOS, Windows (Vulkan, Metal, DirectX 12)
- **Flexible** — Output formats: PNG, JPG, TGA, EXR; configurable quality for JPEG

---

## Quick start

### Install (GameDev monorepo)

If you have the **GameDev** monorepo (this folder lives under `GameDev/Materialize`), use the unified installer from the **repository root**:

```bash
cd /path/to/GameDev
./install.sh materialize
```

See [docs/INSTALLING.md](../docs/INSTALLING.md). Requires **Rust** (`cargo`) and Python 3 for the installer wrapper.

### Install (standalone clone)

Requires **Python 3** (installer) and **Rust** (cargo) to build. The installer compiles and places the binary in `~/.local/bin/materialize` (ensure it’s on your `PATH`).

```bash
git clone https://github.com/maikramer/Materialize-CLI.git
cd Materialize-CLI
./install.sh
```

- **Linux/macOS:** `./install.sh` | `./install.sh uninstall` | `./install.sh reinstall`
- **Windows:** `.\install.ps1` or `install.bat`

### Run

```bash
materialize texture.png
# Writes to current directory:
#   texture_height.png, texture_normal.png, texture_metallic.png,
#   texture_smoothness.png, texture_edge.png, texture_ao.png

materialize texture.png -o ./out/ -v
materialize diffuse.png --format png --quiet
```

### Manual build (Cargo)

```bash
cargo build --release
cargo install --path .
```

---

## Usage

### Syntax

```text
materialize [OPTIONS] [INPUT] [COMMAND]
```

### Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--output` | `-o` | `./` | Output directory |
| `--format` | `-f` | `png` | Output format: `png`, `jpg`, `jpeg`, `tga`, `exr` |
| `--quality` | `-q` | `95` | JPEG quality (0–100) when using `-f jpg` |
| `--verbose` | `-v` | — | Print progress and timing |
| `--quiet` | — | — | Do not list generated files on success |
| `--help` | `-h` | — | Show help |
| `--version` | `-V` | — | Show version |

### Subcommands

- **`materialize skill install`** — Installs the Materialize CLI [Cursor skill](.cursor/skills/materialize-cli/) into the current project’s `.cursor/skills/materialize-cli/`.

### Output naming

For input `texture.png`, outputs are:

- `texture_height.png`
- `texture_normal.png`
- `texture_metallic.png`
- `texture_smoothness.png`
- `texture_edge.png`
- `texture_ao.png`

(Extension follows `--format`.)

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Generic error |
| `2` | Input file not found |
| `3` | Unsupported input format |
| `4` | GPU error (no adapter) |
| `5` | I/O error (permissions, disk full, etc.) |
| `6` | Image too large for GPU |

---

## Examples

```bash
# Default: current directory, PNG
materialize brick.png

# Custom output directory and verbose
materialize brick.png -o ./materials/brick/ -v

# EXR for HDR / precision
materialize texture.png -f exr -o ./out/

# Batch (parallel with xargs)
ls *.png | xargs -P 4 -I {} materialize {} -o ./output/

# Script-friendly: quiet, check exit code
materialize texture.png -o ./out/ --quiet
if [ $? -eq 0 ]; then echo "OK"; fi
```

---

## Getting help

- **Bugs & features** — [Open an issue](https://github.com/maikramer/Materialize-CLI/issues) (templates for bug reports and feature requests are available).
- **Questions** — [GitHub Discussions](https://github.com/maikramer/Materialize-CLI/discussions).
- **Contributing** — See [CONTRIBUTING.md](CONTRIBUTING.md). We follow a [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Requirements

- **Rust** 1.75+
- **GPU** with Vulkan (Linux), Metal (macOS), or DirectX 12 (Windows); up-to-date drivers

---

## Documentation

- [docs/README.md](docs/README.md) — Overview, installation details, and doc index
- [docs/cli-api.md](docs/cli-api.md) — Full CLI reference, env vars, shell completion
- [docs/architecture.md](docs/architecture.md) — System structure
- [docs/features.md](docs/features.md) — Capabilities
- [docs/algorithms.md](docs/algorithms.md) — Processing algorithms
- [docs/shaders.md](docs/shaders.md) — WGSL shaders
- [docs/roadmap.md](docs/roadmap.md) — Future plans

---

## License

[MIT](LICENSE). Based on the original Materialize by Bounding Box Software.

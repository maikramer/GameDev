# Contributing to Materialize CLI

Thank you for considering contributing. This document explains how to get started, the workflow we use, and what we care about.

## Code of conduct

By participating, you agree to uphold our [Code of Conduct](CODE_OF_CONDUCT.md).

## How to contribute

- **Bug reports** — Use [GitHub Issues](https://github.com/maikramer/Materialize-CLI/issues) and the bug report template. Include steps to reproduce, your OS, GPU, and Rust version when relevant.
- **Feature ideas** — Open an issue with the feature request template so we can discuss before you invest time.
- **Documentation** — Fixes and improvements to README, `docs/`, or in-code comments are always welcome.
- **Code** — Open a pull request against `master`. Keep changes focused and mention the related issue if any.

## Development setup

1. **Clone and build**

   ```bash
   git clone https://github.com/maikramer/Materialize-CLI.git
   cd Materialize-CLI
   cargo build --release
   ```

2. **Run tests**

   ```bash
   cargo test
   ```

3. **Run the CLI locally**

   ```bash
   cargo run --release -- texture.png -o ./out/ -v
   ```

You need Rust 1.75+ and a GPU with Vulkan (Linux), Metal (macOS), or DirectX 12 (Windows) support.

## Pull request process

1. Create a branch from `master` (e.g. `fix/issue-123` or `feat/add-option`).
2. Make your changes; keep commits logical and messages clear (e.g. conventional commits: `fix: …`, `docs: …`, `feat: …`).
3. Ensure `cargo test` passes.
4. Open a PR with a short description and, if applicable, “Fixes #123”.
5. Address review feedback. Maintainers will merge when things look good.

We don’t require a formal style guide; follow the existing code style in the project (formatting, naming, module layout). Running `cargo fmt` before committing is appreciated.

## Project structure (high level)

- `src/` — Rust source (CLI, GPU pipeline, shaders, I/O).
- `src/shaders/` — WGSL compute shaders.
- `docs/` — User and design documentation.
- `installer/` — Python installer script; `install.sh`, `install.ps1`, `install.bat` are entry points.

For deeper details, see [docs/architecture.md](docs/architecture.md).

## Questions?

Open a [GitHub Discussion](https://github.com/maikramer/Materialize-CLI/discussions) or an issue with the “question” label if you’re unsure where to start or how something works.

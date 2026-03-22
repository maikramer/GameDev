# Changelog

## [1.0.0] - 2026-03-15

### Added
- Height map generation from diffuse (multi-level box blur + contrast)
- Normal map generation from height (Sobel operator)
- Metallic map generation from diffuse (HSL-based detection)
- CLI interface with clap (input, output dir, format, quality, verbose)
- Support for PNG, JPG, TGA, EXR
- GPU processing via wgpu compute shaders
- Integration tests and unit tests

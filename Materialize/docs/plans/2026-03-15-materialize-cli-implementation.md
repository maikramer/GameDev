# Materialize CLI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implementar uma CLI em Rust que converte imagens difusas em mapas PBR (Height, Normal, Metallic) usando compute shaders wgpu.

**Architecture:** Pipeline de 3 estágios (Diffuse → Height → Normal → Metallic) executando compute shaders WGSL em GPU. CLI usa clap para argumentos, image crate para I/O, wgpu para GPU compute.

**Tech Stack:** Rust, wgpu 0.19, clap 4.5, image 0.24, pollster, anyhow

---

## Task 1: Project Setup

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `README.md` (básico)

**Step 1: Initialize Cargo project**

```bash
cargo init --name materialize-cli
```

**Step 2: Add dependencies to Cargo.toml**

```toml
[package]
name = "materialize-cli"
version = "1.0.0"
edition = "2021"
authors = ["Your Name"]
description = "Generate PBR maps from diffuse textures"
license = "MIT"

[dependencies]
wgpu = "0.19"
pollster = "0.3"
image = { version = "0.24", default-features = false, features = ["png", "jpeg", "tga", "exr"] }
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"

[dev-dependencies]
tempfile = "3.10"
```

**Step 3: Create .gitignore**

```
/target
**/*.rs.bk
Cargo.lock
*.png
*.jpg
*.exr
```

**Step 4: Commit**

```bash
git add Cargo.toml .gitignore
git commit -m "chore: initial project setup"
```

---

## Task 2: CLI Module (cli.rs)

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs` (adicionar módulo)

**Step 1: Write CLI definition**

```rust
// src/cli.rs
use clap::Parser;

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Png,
    Jpg,
    Tga,
    Exr,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "png" => Ok(OutputFormat::Png),
            "jpg" | "jpeg" => Ok(OutputFormat::Jpg),
            "tga" => Ok(OutputFormat::Tga),
            "exr" => Ok(OutputFormat::Exr),
            _ => Err(format!("Unsupported format: {}", s)),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "materialize")]
#[command(about = "Generate PBR maps from diffuse textures")]
#[command(version = "1.0.0")]
pub struct Cli {
    #[arg(help = "Input image path")]
    pub input: String,

    #[arg(short, long, help = "Output directory", default_value = ".")]
    pub output: String,

    #[arg(short, long, help = "Output format (png, jpg, tga, exr)", default_value = "png")]
    pub format: OutputFormat,

    #[arg(short, long, help = "JPEG quality (0-100)", default_value = "95")]
    pub quality: u8,

    #[arg(short, long, help = "Verbose output")]
    pub verbose: bool,
}
```

**Step 2: Update main.rs**

```rust
// src/main.rs
mod cli;

fn main() {
    let args = cli::Cli::parse();
    println!("Input: {}", args.input);
    println!("Output dir: {}", args.output);
}
```

**Step 3: Test CLI parsing**

```bash
cargo run -- --help
cargo run -- test.png -o ./output/ -f exr -v
```

Expected: Shows parsed arguments correctly

**Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: add CLI argument parsing"
```

---

## Task 3: I/O Module (io.rs)

**Files:**
- Create: `src/io.rs`
- Modify: `src/main.rs` (adicionar módulo)

**Step 1: Write io module**

```rust
// src/io.rs
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat, ImageOutputFormat};
use std::path::Path;

pub fn load_image(path: &str) -> Result<DynamicImage> {
    let path = Path::new(path);
    
    if !path.exists() {
        anyhow::bail!("Input file '{}' not found", path.display());
    }
    
    let img = image::open(path)
        .with_context(|| format!("Failed to load image: {}", path.display()))?;
    
    Ok(img)
}

pub fn save_image(
    image: &DynamicImage,
    path: &str,
    format: ImageFormat,
    quality: u8,
) -> Result<()> {
    let path = Path::new(path);
    
    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    
    let output_format = match format {
        ImageFormat::Jpeg => ImageOutputFormat::Jpeg(quality),
        _ => format.into(),
    };
    
    image.save_with_format(path, format)
        .with_context(|| format!("Failed to save image: {}", path.display()))?;
    
    Ok(())
}

pub fn get_output_paths(
    input_path: &str,
    output_dir: &str,
    format: &str,
) -> (String, String, String) {
    let input_name = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    let ext = match format {
        "jpg" | "jpeg" => "jpg",
        _ => format,
    };
    
    let height_path = format!("{}/{}_height.{}", output_dir, input_name, ext);
    let normal_path = format!("{}/{}_normal.{}", output_dir, input_name, ext);
    let metallic_path = format!("{}/{}_metallic.{}", output_dir, input_name, ext);
    
    (height_path, normal_path, metallic_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_image_not_found() {
        let result = load_image("nonexistent.png");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_get_output_paths() {
        let (h, n, m) = get_output_paths("textures/brick.png", "./output", "png");
        assert_eq!(h, "./output/brick_height.png");
        assert_eq!(n, "./output/brick_normal.png");
        assert_eq!(m, "./output/brick_metallic.png");
    }
}
```

**Step 2: Update main.rs**

```rust
// src/main.rs
mod cli;
mod io;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    
    // Load image
    let image = io::load_image(&args.input)?;
    
    if args.verbose {
        println!("Loaded: {} ({}x{})", args.input, image.width(), image.height());
    }
    
    // TODO: Process image
    
    // Get output paths
    let format_str = format!("{:?}", args.format).to_lowercase();
    let (height_path, normal_path, metallic_path) = 
        io::get_output_paths(&args.input, &args.output, &format_str);
    
    println!("Would generate:");
    println!("  - {}", height_path);
    println!("  - {}", normal_path);
    println!("  - {}", metallic_path);
    
    Ok(())
}
```

**Step 3: Run tests**

```bash
cargo test io::tests
```

Expected: Tests pass

**Step 4: Commit**

```bash
git add src/io.rs src/main.rs
git commit -m "feat: add image I/O module"
```

---

## Task 4: Shaders WGSL

**Files:**
- Create: `src/shaders/height.wgsl`
- Create: `src/shaders/normal.wgsl`
- Create: `src/shaders/metallic.wgsl`

**Step 1: Create shaders directory**

```bash
mkdir -p src/shaders
```

**Step 2: Write height.wgsl**

```wgsl
// src/shaders/height.wgsl

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<r32float, write>;

// Luminance weights (ITU-R BT.709)
const LUM_WEIGHTS: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

// Simple box blur for MVP (can be improved to Gaussian)
fn simple_blur(coords: vec2<i32>, dims: vec2<u32>, radius: i32) -> f32 {
    var sum = 0.0;
    var count = 0.0;
    
    for (var x = -radius; x <= radius; x++) {
        for (var y = -radius; y <= radius; y++) {
            let sample_coords = coords + vec2<i32>(x, y);
            if (sample_coords.x >= 0 && sample_coords.x < i32(dims.x) &&
                sample_coords.y >= 0 && sample_coords.y < i32(dims.y)) {
                let color = textureLoad(input_texture, sample_coords, 0).rgb;
                sum += dot(color, LUM_WEIGHTS);
                count += 1.0;
            }
        }
    }
    
    return sum / count;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    let coords = vec2<i32>(global_id.xy);
    
    // Early exit if out of bounds
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    // Multi-level blur (simplified for MVP)
    // Level 0: radius 1, weight 0.5
    // Level 1: radius 2, weight 0.3
    // Level 2: radius 4, weight 0.2
    
    let h0 = simple_blur(coords, dims, 1);
    let h1 = simple_blur(coords, dims, 2);
    let h2 = simple_blur(coords, dims, 4);
    
    let height = h0 * 0.5 + h1 * 0.3 + h2 * 0.2;
    
    // Apply contrast enhancement (sigmoid-like)
    let contrasted = (height - 0.5) * 1.5 + 0.5;
    let final_height = clampContrasted, 0.0, 1.0);
    
    textureStore(output_texture, coords, vec4<f32>(final_height, 0.0, 0.0, 1.0));
}
```

**Step 3: Write normal.wgsl**

```wgsl
// src/shaders/normal.wgsl

@group(0) @binding(0)
var height_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, write>;

// Sobel operator kernels
const SOBEL_X: array<f32, 9> = array(-1.0, 0.0, 1.0, -2.0, 0.0, 2.0, -1.0, 0.0, 1.0);
const SOBEL_Y: array<f32, 9> = array(-1.0, -2.0, -1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 1.0);

fn sample_height(coords: vec2<i32>, dims: vec2<u32>) -> f32 {
    let clamped = clamp(coords, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    return textureLoad(height_texture, clamped, 0).r;
}

fn sobel_gradient(center: vec2<i32>, dims: vec2<u32>) -> vec2<f32> {
    var gx = 0.0;
    var gy = 0.0;
    
    var idx = 0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let sample_coords = center + vec2<i32>(x, y);
            let h = sample_height(sample_coords, dims);
            gx += h * SOBEL_X[idx];
            gy += h * SOBEL_Y[idx];
            idx += 1;
        }
    }
    
    return vec2<f32>(gx, gy);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(height_texture);
    let coords = vec2<i32>(global_id.xy);
    
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    // Calculate gradient using Sobel
    let gradient = sobel_gradient(coords, dims);
    
    // Scale gradient for intensity
    let scale = 2.0;  // Adjust for desired normal strength
    let gx = gradient.x * scale;
    let gy = gradient.y * scale;
    
    // Construct normal vector (pointing up, against gradient)
    var normal = vec3<f32>(-gx, -gy, 1.0);
    normal = normalize(normal);
    
    // Encode to [0, 1] range for RGB8 storage
    // DirectX format: Y points down
    let encoded = normal * 0.5 + 0.5;
    
    textureStore(output_texture, coords, vec4<f32>(encoded, 1.0));
}
```

**Step 4: Write metallic.wgsl**

```wgsl
// src/shaders/metallic.wgsl

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<r8unorm, write>;

// RGB to HSL conversion
fn rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
    let max_val = max(max(rgb.r, rgb.g), rgb.b);
    let min_val = min(min(rgb.r, rgb.g), rgb.b);
    let delta = max_val - min_val;
    
    // Luminance
    let l = (max_val + min_val) * 0.5;
    
    // Saturation
    var s = 0.0;
    if (delta > 0.0) {
        s = delta / (1.0 - abs(2.0 * l - 1.0));
    }
    
    // Hue
    var h = 0.0;
    if (delta > 0.0) {
        if (max_val == rgb.r) {
            h = (rgb.g - rgb.b) / delta;
            if (rgb.g < rgb.b) {
                h += 6.0;
            }
        } else if (max_val == rgb.g) {
            h = (rgb.b - rgb.r) / delta + 2.0;
        } else {
            h = (rgb.r - rgb.g) / delta + 4.0;
        }
        h = h / 6.0;
    }
    
    return vec3<f32>(h, s, l);
}

// Smoothstep function
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Detect metallic based on HSL analysis
fn detect_metallic(rgb: vec3<f32>) -> f32 {
    let hsl = rgb_to_hsl(rgb);
    let h = hsl.x;  // Hue [0, 1]
    let s = hsl.y;  // Saturation [0, 1]
    let l = hsl.z;  // Luminance [0, 1]
    
    var metallic = 0.0;
    
    // Gray metals (iron, steel, aluminum, silver)
    // Low saturation, medium-high luminance
    if (s < 0.15 && l > 0.3 && l < 0.9) {
        let lum_factor = smooth_step(0.3, 0.8, l);
        let sat_factor = 1.0 - smooth_step(0.0, 0.15, s);
        metallic = max(metallic, lum_factor * sat_factor * 0.9);
    }
    
    // Gold (hue ~0.08-0.15 in normalized [0,1])
    if (h > 0.08 && h < 0.15 && s > 0.3 && l > 0.3) {
        let hue_center = 0.115;  // Gold hue center
        let hue_dist = abs(h - hue_center);
        let hue_factor = 1.0 - smooth_step(0.0, 0.035, hue_dist);
        let lum_factor = smooth_step(0.3, 0.7, l);
        let sat_factor = smooth_step(0.3, 0.8, s);
        metallic = max(metallic, hue_factor * lum_factor * sat_factor);
    }
    
    // Copper (hue ~0.02-0.08)
    if (h > 0.02 && h < 0.08 && s > 0.4 && l > 0.2) {
        let hue_center = 0.05;  // Copper hue center
        let hue_dist = abs(h - hue_center);
        let hue_factor = 1.0 - smooth_step(0.0, 0.03, hue_dist);
        let lum_factor = smooth_step(0.2, 0.6, l);
        let sat_factor = smooth_step(0.4, 0.9, s);
        metallic = max(metallic, hue_factor * lum_factor * sat_factor);
    }
    
    return clamp(metallic, 0.0, 1.0);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    let coords = vec2<i32>(global_id.xy);
    
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    let color = textureLoad(input_texture, coords, 0).rgb;
    let metallic = detect_metallic(color);
    
    textureStore(output_texture, coords, vec4<f32>(metallic, 0.0, 0.0, 0.0));
}
```

**Step 5: Commit**

```bash
git add src/shaders/
git commit -m "feat: add WGSL compute shaders for height, normal, and metallic"
```

---

## Task 5: GPU Module (gpu.rs) - Part 1: Setup

**Files:**
- Create: `src/gpu.rs` (estrutura inicial)
- Modify: `src/main.rs`

**Step 1: Create GPU module structure**

```rust
// src/gpu.rs
use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    pub async fn new() -> Result<Self> {
        // Initialize wgpu
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .context("No GPU adapter available. Check Vulkan/Metal/DX12 drivers")?;
        
        println!("Using adapter: {}", adapter.get_info().name);
        
        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .context("Failed to create GPU device")?;
        
        Ok(Self { device, queue })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpu_init() {
        let gpu = GpuContext::new().await;
        assert!(gpu.is_ok());
    }
}
```

**Step 2: Update main.rs**

```rust
// src/main.rs
mod cli;
mod gpu;
mod io;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    
    // Initialize GPU
    let gpu = gpu::GpuContext::new().await?;
    
    if args.verbose {
        println!("GPU initialized successfully");
    }
    
    // Load image
    let image = io::load_image(&args.input)?;
    
    if args.verbose {
        println!("Loaded: {} ({}x{})", args.input, image.width(), image.height());
    }
    
    // TODO: Process image
    
    // Get output paths
    let format_str = format!("{:?}", args.format).to_lowercase();
    let (height_path, normal_path, metallic_path) = 
        io::get_output_paths(&args.input, &args.output, &format_str);
    
    println!("Would generate:");
    println!("  - {}", height_path);
    println!("  - {}", normal_path);
    println!("  - {}", metallic_path);
    
    Ok(())
}
```

**Step 3: Add tokio to dependencies**

Update `Cargo.toml`:
```toml
[dependencies]
wgpu = "0.19"
tokio = { version = "1.0", features = ["full"] }  # Add this
pollster = "0.3"
image = { version = "0.24", default-features = false, features = ["png", "jpeg", "tga", "exr"] }
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
```

**Step 4: Test GPU initialization**

```bash
cargo run -- test.png -v
```

Expected: Shows "GPU initialized successfully" and "Using adapter: [GPU name]"

**Step 5: Commit**

```bash
git add src/gpu.rs src/main.rs Cargo.toml
it commit -m "feat: add GPU context initialization"
```

---

## Task 6: GPU Module (gpu.rs) - Part 2: Texture Management

**Files:**
- Modify: `src/gpu.rs`

**Step 1: Add texture creation methods**

```rust
// Add to src/gpu.rs after the new() function

impl GpuContext {
    // ... new() function ...
    
    /// Create a texture from a DynamicImage
    pub fn create_texture_from_image(
        &self,
        image: &image::DynamicImage,
    ) -> wgpu::Texture {
        let rgba = image.to_rgba8();
        let dimensions = image.dimensions();
        
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("input_texture"),
            view_formats: &[],
        });
        
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );
        
        texture
    }
    
    /// Create an empty texture for output
    pub fn create_output_texture(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        
        self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            label: Some("output_texture"),
            view_formats: &[],
        })
    }
}
```

**Step 2: Test texture creation**

```bash
cargo build
```

Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src/gpu.rs
it commit -m "feat: add texture creation methods"
```

---

## Task 7: GPU Module (gpu.rs) - Part 3: Compute Pipeline

**Files:**
- Modify: `src/gpu.rs`

**Step 1: Add compute pipeline creation**

```rust
// Add to src/gpu.rs

pub struct ComputePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuContext {
    // ... existing methods ...
    
    /// Create a compute pipeline from WGSL shader
    pub fn create_compute_pipeline(
        &self,
        shader_code: &str,
        entry_point: &str,
    ) -> Result<ComputePipeline> {
        // Create shader module
        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_code.into()),
        });
        
        // Create bind group layout (2 textures: input and output)
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout"),
            entries: &[
                // Input texture (sampled)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Output texture (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float, // Will be overridden per shader
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        
        // Create pipeline layout
        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        // Create compute pipeline
        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point,
        });
        
        Ok(ComputePipeline {
            pipeline,
            bind_group_layout,
        })
    }
}
```

**Step 2: Add dispatch method**

```rust
// Add to src/gpu.rs

impl GpuContext {
    // ... existing methods ...
    
    /// Create bind group for compute dispatch
    pub fn create_bind_group(
        &self,
        layout: &wgpu::BindGroupLayout,
        input_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
            ],
            label: Some("bind_group"),
        })
    }
    
    /// Dispatch compute shader
    pub fn dispatch_compute(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: &wgpu::BindGroup,
        workgroups_x: u32,
        workgroups_y: u32,
    ) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("compute_encoder"),
        });
        
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute_pass"),
                timestamp_writes: None,
            });
            
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
        
        self.queue.submit(Some(encoder.finish()));
    }
}
```

**Step 3: Test compilation**

```bash
cargo build
```

Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/gpu.rs
it commit -m "feat: add compute pipeline creation and dispatch"
```

---

## Task 8: GPU Module (gpu.rs) - Part 4: Texture Readback

**Files:**
- Modify: `src/gpu.rs`

**Step 1: Add texture readback method**

```rust
// Add to src/gpu.rs

impl GpuContext {
    // ... existing methods ...
    
    /// Read texture data back to CPU
    pub async fn read_texture(
        &self,
        texture: &wgpu::Texture,
    ) -> Result<Vec<u8>> {
        let size = texture.size();
        let format = texture.format();
        
        // Calculate buffer size
        let bytes_per_pixel = match format {
            wgpu::TextureFormat::R32Float => 4,
            wgpu::TextureFormat::R8Unorm => 1,
            wgpu::TextureFormat::Rgba8Unorm => 4,
            _ => anyhow::bail!("Unsupported texture format for readback"),
        };
        
        let unpadded_bytes_per_row = size.width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = padded_bytes_per_row * size.height;
        
        // Create buffer
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback_buffer"),
            size: buffer_size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        
        // Copy texture to buffer
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback_encoder"),
        });
        
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );
        
        self.queue.submit(Some(encoder.finish()));
        
        // Map buffer and read data
        let buffer_slice = buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
        
        self.device.poll(wgpu::Maintain::Wait);
        receiver.await??;
        
        let data = buffer_slice.get_mapped_range();
        
        // Remove padding
        let mut unpadded_data = Vec::with_capacity((unpadded_bytes_per_row * size.height) as usize);
        for row in 0..size.height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            unpadded_data.extend_from_slice(&data[start..end]);
        }
        
        drop(data);
        buffer.unmap();
        
        Ok(unpadded_data)
    }
}
```

**Step 2: Add futures to dependencies**

Update `Cargo.toml`:
```toml
[dependencies]
wgpu = "0.19"
tokio = { version = "1.0", features = ["full"] }
pollster = "0.3"
futures = "0.3"  # Add this
image = { version = "0.24", default-features = false, features = ["png", "jpeg", "tga", "exr"] }
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
```

**Step 3: Test compilation**

```bash
cargo build
```

Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/gpu.rs Cargo.toml
it commit -m "feat: add texture readback from GPU"
```

---

## Task 9: Pipeline Module (pipeline.rs)

**Files:**
- Create: `src/pipeline.rs`
- Modify: `src/main.rs`

**Step 1: Create pipeline module**

```rust
// src/pipeline.rs
use anyhow::Result;
use image::DynamicImage;

use crate::gpu::{GpuContext, ComputePipeline};

const HEIGHT_SHADER: &str = include_str!("shaders/height.wgsl");
const NORMAL_SHADER: &str = include_str!("shaders/normal.wgsl");
const METALLIC_SHADER: &str = include_str!("shaders/metallic.wgsl");

pub struct PbrMaps {
    pub height: Vec<f32>,
    pub normal: Vec<u8>,
    pub metallic: Vec<u8>,
}

pub struct Pipeline {
    gpu: GpuContext,
    height_pipeline: ComputePipeline,
    normal_pipeline: ComputePipeline,
    metallic_pipeline: ComputePipeline,
}

impl Pipeline {
    pub async fn new() -> Result<Self> {
        let gpu = GpuContext::new().await?;
        
        let height_pipeline = gpu.create_compute_pipeline(HEIGHT_SHADER, "main")?;
        let normal_pipeline = gpu.create_compute_pipeline(NORMAL_SHADER, "main")?;
        let metallic_pipeline = gpu.create_compute_pipeline(METALLIC_SHADER, "main")?;
        
        Ok(Self {
            gpu,
            height_pipeline,
            normal_pipeline,
            metallic_pipeline,
        })
    }
    
    pub async fn process(&self, image: &DynamicImage) -> Result<PbrMaps> {
        let width = image.width();
        let height = image.height();
        
        // Create input texture
        let diffuse_texture = self.gpu.create_texture_from_image(image);
        let diffuse_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        // 1. Generate height map
        let height_texture = self.gpu.create_output_texture(width, height, wgpu::TextureFormat::R32Float);
        let height_view = height_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let height_bind_group = self.gpu.create_bind_group(
            &self.height_pipeline.bind_group_layout,
            &diffuse_view,
            &height_view,
        );
        
        let workgroups_x = (width + 7) / 8;
        let workgroups_y = (height + 7) / 8;
        
        self.gpu.dispatch_compute(
            &self.height_pipeline.pipeline,
            &height_bind_group,
            workgroups_x,
            workgroups_y,
        );
        
        // 2. Generate normal map from height
        let normal_texture = self.gpu.create_output_texture(width, height, wgpu::TextureFormat::Rgba8Unorm);
        let normal_view = normal_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let normal_bind_group = self.gpu.create_bind_group(
            &self.normal_pipeline.bind_group_layout,
            &height_view,
            &normal_view,
        );
        
        self.gpu.dispatch_compute(
            &self.normal_pipeline.pipeline,
            &normal_bind_group,
            workgroups_x,
            workgroups_y,
        );
        
        // 3. Generate metallic map from diffuse
        let metallic_texture = self.gpu.create_output_texture(width, height, wgpu::TextureFormat::R8Unorm);
        let metallic_view = metallic_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let metallic_bind_group = self.gpu.create_bind_group(
            &self.metallic_pipeline.bind_group_layout,
            &diffuse_view,
            &metallic_view,
        );
        
        self.gpu.dispatch_compute(
            &self.metallic_pipeline.pipeline,
            &metallic_bind_group,
            workgroups_x,
            workgroups_y,
        );
        
        // Read back results
        let height_data = self.gpu.read_texture(&height_texture).await?;
        let normal_data = self.gpu.read_texture(&normal_texture).await?;
        let metallic_data = self.gpu.read_texture(&metallic_texture).await?;
        
        // Convert height from bytes to f32
        let height_f32: Vec<f32> = height_data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        
        Ok(PbrMaps {
            height: height_f32,
            normal: normal_data,
            metallic: metallic_data,
        })
    }
}
```

**Step 2: Update main.rs**

```rust
// src/main.rs
mod cli;
mod gpu;
mod io;
mod pipeline;

use clap::Parser;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Cli::parse();
    
    // Load image
    let image = io::load_image(&args.input)?;
    
    if args.verbose {
        println!("Loaded: {} ({}x{})", args.input, image.width(), image.height());
    }
    
    // Initialize pipeline
    let pipeline = pipeline::Pipeline::new().await?;
    
    if args.verbose {
        println!("Processing...");
    }
    
    // Process image
    let maps = pipeline.process(&image).await?;
    
    if args.verbose {
        println!("Height: {} pixels", maps.height.len());
        println!("Normal: {} bytes", maps.normal.len());
        println!("Metallic: {} bytes", maps.metallic.len());
    }
    
    // TODO: Convert and save maps
    
    // Get output paths
    let format_str = format!("{:?}", args.format).to_lowercase();
    let (height_path, normal_path, metallic_path) = 
        io::get_output_paths(&args.input, &args.output, &format_str);
    
    println!("Generated:");
    println!("  - {}", height_path);
    println!("  - {}", normal_path);
    println!("  - {}", metallic_path);
    
    Ok(())
}
```

**Step 3: Fix bind group layout format issue**

The bind group layout needs to be more flexible. Update `gpu.rs`:

```rust
// In gpu.rs, update create_compute_pipeline to accept format parameter

pub fn create_compute_pipeline(
    &self,
    shader_code: &str,
    entry_point: &str,
    output_format: wgpu::TextureFormat,
) -> Result<ComputePipeline> {
    // ... existing code ...
    
    // Update storage texture format to match output
    let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bind_group_layout"),
        entries: &[
            // Input texture (sampled)
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // Output texture (storage) - format is flexible
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: output_format,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    });
    
    // ... rest of code ...
}
```

Then update `pipeline.rs`:

```rust
let height_pipeline = gpu.create_compute_pipeline(HEIGHT_SHADER, "main", wgpu::TextureFormat::R32Float)?;
let normal_pipeline = gpu.create_compute_pipeline(NORMAL_SHADER, "main", wgpu::TextureFormat::Rgba8Unorm)?;
let metallic_pipeline = gpu.create_compute_pipeline(METALLIC_SHADER, "main", wgpu::TextureFormat::R8Unorm)?;
```

**Step 4: Test compilation**

```bash
cargo build
```

Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/pipeline.rs src/gpu.rs src/main.rs
it commit -m "feat: add pipeline orchestration module"
```

---

## Task 10: I/O - Converting and Saving Maps

**Files:**
- Modify: `src/io.rs`
- Modify: `src/main.rs`

**Step 1: Add conversion and save methods to io.rs**

```rust
// Add to src/io.rs

use image::{ImageBuffer, Luma, Rgba, Rgb};

/// Convert height map (f32) to grayscale image
pub fn height_to_image(width: u32, height: u32, data: &[f32]) -> DynamicImage {
    let mut img = ImageBuffer::new(width, height);
    
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let idx = (y * width + x) as usize;
        let value = (data[idx] * 255.0).clamp(0.0, 255.0) as u8;
        *pixel = Luma([value]);
    }
    
    DynamicImage::ImageLuma8(img)
}

/// Convert normal map (RGBA8) to RGB image
pub fn normal_to_image(width: u32, height: u32, data: &[u8]) -> DynamicImage {
    let mut img = ImageBuffer::new(width, height);
    
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let idx = ((y * width + x) * 4) as usize;
        let r = data[idx];
        let g = data[idx + 1];
        let b = data[idx + 2];
        *pixel = Rgb([r, g, b]);
    }
    
    DynamicImage::ImageRgb8(img)
}

/// Convert metallic map (R8) to grayscale image
pub fn metallic_to_image(width: u32, height: u32, data: &[u8]) -> DynamicImage {
    let mut img = ImageBuffer::new(width, height);
    
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let idx = (y * width + x) as usize;
        *pixel = Luma([data[idx]]);
    }
    
    DynamicImage::ImageLuma8(img)
}

/// Map OutputFormat to ImageFormat
pub fn output_format_to_image_format(format: &super::cli::OutputFormat) -> ImageFormat {
    match format {
        super::cli::OutputFormat::Png => ImageFormat::Png,
        super::cli::OutputFormat::Jpg => ImageFormat::Jpeg,
        super::cli::OutputFormat::Tga => ImageFormat::Tga,
        super::cli::OutputFormat::Exr => ImageFormat::OpenExr,
    }
}
```

**Step 2: Update main.rs to save outputs**

```rust
// Update main.rs main function

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Cli::parse();
    
    // Load image
    let image = io::load_image(&args.input)?;
    let (width, height) = (image.width(), image.height());
    
    if args.verbose {
        println!("Loaded: {} ({}x{})", args.input, width, height);
    }
    
    // Initialize pipeline
    let pipeline = pipeline::Pipeline::new().await?;
    
    if args.verbose {
        println!("Processing...");
    }
    
    // Process image
    let maps = pipeline.process(&image).await?;
    
    if args.verbose {
        println!("Processing complete");
    }
    
    // Convert to images
    let height_img = io::height_to_image(width, height, &maps.height);
    let normal_img = io::normal_to_image(width, height, &maps.normal);
    let metallic_img = io::metallic_to_image(width, height, &maps.metallic);
    
    // Get output paths
    let format_str = format!("{:?}", args.format).to_lowercase();
    let (height_path, normal_path, metallic_path) = 
        io::get_output_paths(&args.input, &args.output, &format_str);
    
    // Save images
    let image_format = io::output_format_to_image_format(&args.format);
    
    io::save_image(&height_img, &height_path, image_format, args.quality)?;
    io::save_image(&normal_img, &normal_path, image_format, args.quality)?;
    io::save_image(&metallic_img, &metallic_path, image_format, args.quality)?;
    
    println!("Generated:");
    println!("  - {}", height_path);
    println!("  - {}", normal_path);
    println!("  - {}", metallic_path);
    
    Ok(())
}
```

**Step 3: Test compilation**

```bash
cargo build
```

Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/io.rs src/main.rs
it commit -m "feat: add map conversion and saving"
```

---

## Task 11: Integration Testing

**Files:**
- Create: `tests/integration_test.rs`
- Create: `tests/fixtures/test.png`

**Step 1: Create test fixtures**

```bash
mkdir -p tests/fixtures

# Create a simple test image using ImageMagick (if available)
# Or use a placeholder - we'll test with a real file
```

**Step 2: Write integration tests**

```rust
// tests/integration_test.rs
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Generate PBR maps"));
    assert!(stdout.contains("--output"));
    assert!(stdout.contains("--format"));
}

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1.0.0"));
}

#[test]
fn test_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output");
    
    let output = Command::new("cargo")
        .args(["run", "--", "nonexistent.png", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("Failed to execute command");
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));
}
```

**Step 3: Run tests**

```bash
cargo test
```

Expected: Tests pass

**Step 4: Commit**

```bash
git add tests/
git commit -m "test: add integration tests"
```

---

## Task 12: Documentation and README

**Files:**
- Modify: `README.md`
- Create: `CHANGELOG.md`

**Step 1: Update README.md**

```markdown
# Materialize CLI

CLI minimalista em Rust que converte imagens em materiais PBR (Height, Normal, Metallic) usando compute shaders GPU via wgpu.

## Instalação

```bash
git clone https://github.com/seu-user/materialize-cli
cd materialize-cli
cargo build --release
cargo install --path .
```

## Uso

```bash
# Básico
materialize texture.png

# Com opções
materialize texture.png -o ./output/ -f exr -v
```

## Funcionalidades

- Gera Height map via multi-level blur
- Gera Normal map via Sobel operator
- Gera Metallic map via análise HSL
- Suporta PNG, JPG, TGA, EXR
- Processamento GPU rápido

## Documentação

Veja a pasta `docs/` para documentação completa:
- [Arquitetura](docs/architecture.md)
- [Algoritmos](docs/algorithms.md)
- [CLI API](docs/cli-api.md)

## Licença

MIT
```

**Step 2: Create CHANGELOG.md**

```markdown
# Changelog

## [1.0.0] - 2026-03-15

### Added
- Height map generation
- Normal map generation
- Metallic map generation
- CLI interface com clap
- Suporte a PNG, JPG, TGA, EXR
- GPU processing via wgpu
```

**Step 3: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: add README and CHANGELOG"
```

---

## Task 13: Final Review and Polish

**Files:**
- Modify: All source files for polish

**Step 1: Add better error messages to main.rs**

```rust
// Update main.rs error handling

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    // ... existing main() code ...
}
```

**Step 2: Run clippy**

```bash
cargo clippy -- -D warnings
```

Fix any warnings.

**Step 3: Run formatter**

```bash
cargo fmt
```

**Step 4: Final test**

```bash
cargo test
cargo build --release
```

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: final polish and error handling"
```

---

## Summary

### Completed Features

- [x] CLI argument parsing (clap)
- [x] Image I/O (image crate)
- [x] GPU context (wgpu)
- [x] Compute pipelines for 3 shaders
- [x] Height map generation
- [x] Normal map generation  
- [x] Metallic map generation
- [x] Pipeline orchestration
- [x] Map conversion and saving
- [x] Integration tests
- [x] Documentation

### File Structure

```
materialize-cli/
├── Cargo.toml
├── README.md
├── CHANGELOG.md
├── .gitignore
├── docs/
│   ├── README.md
│   ├── architecture.md
│   ├── algorithms.md
│   ├── cli-api.md
│   ├── features.md
│   ├── shaders.md
│   └── roadmap.md
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── io.rs
│   ├── gpu.rs
│   ├── pipeline.rs
│   └── shaders/
│       ├── height.wgsl
│       ├── normal.wgsl
│       └── metallic.wgsl
└── tests/
    └── integration_test.rs
```

### Next Steps

1. Test with real images
2. Tune shader parameters
3. Add more test coverage
4. Implement features do roadmap (v1.1+)

---

**Plan complete and saved to `docs/plans/2026-03-15-materialize-cli-implementation.md`.**

## Execution Options

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach would you prefer?

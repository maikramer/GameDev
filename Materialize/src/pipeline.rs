use anyhow::Result;
use image::DynamicImage;

use crate::gpu::{ComputePipeline, GpuContext};

const HEIGHT_SHADER: &str = include_str!("shaders/height.wgsl");
const NORMAL_SHADER: &str = include_str!("shaders/normal.wgsl");
const METALLIC_SHADER: &str = include_str!("shaders/metallic.wgsl");
const SMOOTHNESS_SHADER: &str = include_str!("shaders/smoothness.wgsl");
const EDGE_SHADER: &str = include_str!("shaders/edge.wgsl");
const AO_SHADER: &str = include_str!("shaders/ao.wgsl");

pub struct PbrMaps {
    pub height: Vec<f32>,
    pub normal: Vec<u8>,
    pub metallic: Vec<u8>,
    pub smoothness: Vec<u8>,
    pub edge: Vec<u8>,
    pub ao: Vec<u8>,
}

pub struct Pipeline {
    gpu: GpuContext,
    height_pipeline: ComputePipeline,
    normal_pipeline: ComputePipeline,
    metallic_pipeline: ComputePipeline,
    smoothness_pipeline: ComputePipeline,
    edge_pipeline: ComputePipeline,
    ao_pipeline: ComputePipeline,
}

impl Pipeline {
    pub async fn new() -> Result<Self> {
        let gpu = GpuContext::new().await?;

        let height_pipeline = gpu.create_compute_pipeline(
            HEIGHT_SHADER,
            "main",
            wgpu::TextureFormat::Rgba8Unorm, // input
            wgpu::TextureFormat::R32Float,   // output
        )?;
        let normal_pipeline = gpu.create_compute_pipeline(
            NORMAL_SHADER,
            "main",
            wgpu::TextureFormat::R32Float,   // input (height)
            wgpu::TextureFormat::Rgba8Unorm, // output
        )?;
        let metallic_pipeline = gpu.create_compute_pipeline(
            METALLIC_SHADER,
            "main",
            wgpu::TextureFormat::Rgba8Unorm, // input
            wgpu::TextureFormat::Rgba8Unorm, // output (R channel only used)
        )?;

        let smoothness_pipeline = gpu.create_compute_pipeline_2_inputs(
            SMOOTHNESS_SHADER,
            "main",
            wgpu::TextureFormat::Rgba8Unorm, // diffuse
            wgpu::TextureFormat::Rgba8Unorm, // metallic
            wgpu::TextureFormat::Rgba8Unorm, // output
        )?;

        let edge_pipeline = gpu.create_compute_pipeline(
            EDGE_SHADER,
            "main",
            wgpu::TextureFormat::Rgba8Unorm, // normal
            wgpu::TextureFormat::Rgba8Unorm, // output
        )?;

        let ao_pipeline = gpu.create_compute_pipeline(
            AO_SHADER,
            "main",
            wgpu::TextureFormat::R32Float,   // height
            wgpu::TextureFormat::Rgba8Unorm, // output
        )?;

        Ok(Self {
            gpu,
            height_pipeline,
            normal_pipeline,
            metallic_pipeline,
            smoothness_pipeline,
            edge_pipeline,
            ao_pipeline,
        })
    }

    pub async fn process(&self, image: &DynamicImage) -> Result<PbrMaps> {
        let width = image.width();
        let height = image.height();

        let diffuse_texture = self.gpu.create_texture_from_image(image);
        let diffuse_view = diffuse_texture.create_view(&Default::default());

        // 1. Generate height map
        let height_texture = self
            .gpu
            .create_output_texture(width, height, wgpu::TextureFormat::R32Float);
        let height_view = height_texture.create_view(&Default::default());

        let height_bind_group = self.gpu.create_bind_group(
            &self.height_pipeline.bind_group_layout,
            &diffuse_view,
            &height_view,
        );

        let workgroups_x = width.div_ceil(8);
        let workgroups_y = height.div_ceil(8);

        self.gpu.dispatch_compute(
            &self.height_pipeline.pipeline,
            &height_bind_group,
            workgroups_x,
            workgroups_y,
        );

        // 2. Generate normal map from height
        let normal_texture = self
            .gpu
            .create_output_texture(width, height, wgpu::TextureFormat::Rgba8Unorm);
        let normal_view = normal_texture.create_view(&Default::default());

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

        // 3. Generate metallic map from diffuse (Rgba8Unorm: only R channel used)
        let metallic_texture = self
            .gpu
            .create_output_texture(width, height, wgpu::TextureFormat::Rgba8Unorm);
        let metallic_view = metallic_texture.create_view(&Default::default());

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

        // 4. Smoothness from diffuse + metallic (2 inputs)
        let smoothness_texture = self
            .gpu
            .create_output_texture(width, height, wgpu::TextureFormat::Rgba8Unorm);
        let smoothness_view = smoothness_texture.create_view(&Default::default());

        let smoothness_bind_group = self.gpu.create_bind_group_2_inputs(
            &self.smoothness_pipeline.bind_group_layout,
            &diffuse_view,
            &metallic_view,
            &smoothness_view,
        );

        self.gpu.dispatch_compute(
            &self.smoothness_pipeline.pipeline,
            &smoothness_bind_group,
            workgroups_x,
            workgroups_y,
        );

        // 5. Edge from normal
        let edge_texture = self
            .gpu
            .create_output_texture(width, height, wgpu::TextureFormat::Rgba8Unorm);
        let edge_view = edge_texture.create_view(&Default::default());

        let edge_bind_group = self.gpu.create_bind_group(
            &self.edge_pipeline.bind_group_layout,
            &normal_view,
            &edge_view,
        );

        self.gpu.dispatch_compute(
            &self.edge_pipeline.pipeline,
            &edge_bind_group,
            workgroups_x,
            workgroups_y,
        );

        // 6. AO from height
        let ao_texture = self
            .gpu
            .create_output_texture(width, height, wgpu::TextureFormat::Rgba8Unorm);
        let ao_view = ao_texture.create_view(&Default::default());

        let ao_bind_group = self.gpu.create_bind_group(
            &self.ao_pipeline.bind_group_layout,
            &height_view,
            &ao_view,
        );

        self.gpu.dispatch_compute(
            &self.ao_pipeline.pipeline,
            &ao_bind_group,
            workgroups_x,
            workgroups_y,
        );

        // Read back results - we need to wait for GPU to finish before readback
        self.gpu.device.poll(wgpu::Maintain::Wait);

        let height_data = self.gpu.read_texture(&height_texture).await?;
        let normal_data = self.gpu.read_texture(&normal_texture).await?;
        let metallic_data = self.gpu.read_texture(&metallic_texture).await?;
        let smoothness_data = self.gpu.read_texture(&smoothness_texture).await?;
        let edge_data = self.gpu.read_texture(&edge_texture).await?;
        let ao_data = self.gpu.read_texture(&ao_texture).await?;

        let height_f32: Vec<f32> = height_data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        // Metallic/smoothness/edge/ao written as RGBA; we use R channel for grayscale outputs
        let metallic_r: Vec<u8> = metallic_data.chunks_exact(4).map(|c| c[0]).collect();
        let smoothness_r: Vec<u8> = smoothness_data.chunks_exact(4).map(|c| c[0]).collect();
        let edge_r: Vec<u8> = edge_data.chunks_exact(4).map(|c| c[0]).collect();
        let ao_r: Vec<u8> = ao_data.chunks_exact(4).map(|c| c[0]).collect();

        Ok(PbrMaps {
            height: height_f32,
            normal: normal_data,
            metallic: metallic_r,
            smoothness: smoothness_r,
            edge: edge_r,
            ao: ao_r,
        })
    }
}

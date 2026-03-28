struct Params {
    height_blur_radius_0: f32,
    height_blur_radius_1: f32,
    height_blur_radius_2: f32,
    height_contrast: f32,
    normal_strength: f32,
    metallic_scale: f32,
    smoothness_base: f32,
    smoothness_metallic_boost: f32,
    edge_contrast: f32,
    ao_depth_scale: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0)
var diffuse_texture: texture_2d<f32>;

@group(0) @binding(1)
var metallic_texture: texture_2d<f32>;

@group(0) @binding(2)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@group(1) @binding(0)
var<uniform> params: Params;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(diffuse_texture);
    let coords = vec2<i32>(global_id.xy);

    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }

    let metallic = textureLoad(metallic_texture, coords, 0).r;
    let smoothness = clamp(params.smoothness_base + params.smoothness_metallic_boost * metallic, 0.0, 1.0);

    textureStore(output_texture, coords, vec4<f32>(smoothness, smoothness, smoothness, 1.0));
}

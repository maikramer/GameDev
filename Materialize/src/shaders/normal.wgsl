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
var height_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@group(1) @binding(0)
var<uniform> params: Params;

fn sample_height(coords: vec2<i32>, dims: vec2<u32>) -> f32 {
    let clamped = clamp(coords, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    return textureLoad(height_texture, clamped, 0).r;
}

fn sobel_gradient(center: vec2<i32>, dims: vec2<u32>) -> vec2<f32> {
    let h_m1_m1 = sample_height(center + vec2<i32>(-1, -1), dims);
    let h_0_m1 = sample_height(center + vec2<i32>(0, -1), dims);
    let h_1_m1 = sample_height(center + vec2<i32>(1, -1), dims);
    let h_m1_0 = sample_height(center + vec2<i32>(-1, 0), dims);
    let h_0_0 = sample_height(center + vec2<i32>(0, 0), dims);
    let h_1_0 = sample_height(center + vec2<i32>(1, 0), dims);
    let h_m1_1 = sample_height(center + vec2<i32>(-1, 1), dims);
    let h_0_1 = sample_height(center + vec2<i32>(0, 1), dims);
    let h_1_1 = sample_height(center + vec2<i32>(1, 1), dims);

    let gx = -h_m1_m1 + h_1_m1 - 2.0 * h_m1_0 + 2.0 * h_1_0 - h_m1_1 + h_1_1;
    let gy = -h_m1_m1 - 2.0 * h_0_m1 - h_1_m1 + h_m1_1 + 2.0 * h_0_1 + h_1_1;

    return vec2<f32>(gx, gy);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(height_texture);
    let coords = vec2<i32>(global_id.xy);

    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }

    let gradient = sobel_gradient(coords, dims);

    let scale = params.normal_strength;
    let gx = gradient.x * scale;
    let gy = gradient.y * scale;

    var normal = vec3<f32>(-gx, -gy, 1.0);
    normal = normalize(normal);

    let encoded = normal * 0.5 + 0.5;

    textureStore(output_texture, coords, vec4<f32>(encoded, 1.0));
}

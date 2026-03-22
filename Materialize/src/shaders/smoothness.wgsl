// Smoothness from diffuse + metallic (simplified: base + metal contribution).
// Original Materialize also uses blur overlay; we use base + metal smoothness.

@group(0) @binding(0)
var diffuse_texture: texture_2d<f32>;

@group(0) @binding(1)
var metallic_texture: texture_2d<f32>;

@group(0) @binding(2)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(diffuse_texture);
    let coords = vec2<i32>(global_id.xy);

    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }

    let base_smoothness = 0.25;
    let metal_smoothness = 0.65;
    let metallic = textureLoad(metallic_texture, coords, 0).r;
    let smoothness = clamp(base_smoothness + metal_smoothness * metallic, 0.0, 1.0);

    textureStore(output_texture, coords, vec4<f32>(smoothness, smoothness, smoothness, 1.0));
}

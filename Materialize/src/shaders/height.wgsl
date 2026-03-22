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
    let h0 = simple_blur(coords, dims, 1);
    let h1 = simple_blur(coords, dims, 2);
    let h2 = simple_blur(coords, dims, 4);

    let height = h0 * 0.5 + h1 * 0.3 + h2 * 0.2;

    // Apply contrast enhancement (sigmoid-like)
    let contrasted = (height - 0.5) * 1.5 + 0.5;
    let final_height = clamp(contrasted, 0.0, 1.0);

    textureStore(output_texture, coords, vec4<f32>(final_height, 0.0, 0.0, 1.0));
}

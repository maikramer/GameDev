// Edge from normal map: gradient of normal X/Y, combined.
// Matches Blit_Edge_From_Normal fragEdge logic (diffX, diffY from neighbor samples).

@group(0) @binding(0)
var normal_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, write>;

fn sample_normal_rg(coords: vec2<i32>, dims: vec2<u32>) -> vec2<f32> {
    let clamped = clamp(coords, vec2<i32>(0), vec2<i32>(dims) - vec2<i32>(1));
    let n = textureLoad(normal_texture, clamped, 0);
    return vec2<f32>(n.r, n.g);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(normal_texture);
    let coords = vec2<i32>(global_id.xy);

    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }

    let contrast = 2.0;
    let n_x_plus = sample_normal_rg(coords + vec2<i32>(1, 0), dims);
    let n_x_minus = sample_normal_rg(coords + vec2<i32>(-1, 0), dims);
    let n_y_plus = sample_normal_rg(coords + vec2<i32>(0, 1), dims);
    let n_y_minus = sample_normal_rg(coords + vec2<i32>(0, -1), dims);

    var diff_x = (n_x_plus.r - n_x_minus.r) * contrast;
    var diff_y = (n_y_plus.g - n_y_minus.g) * contrast;
    diff_y = -diff_y;

    let diff = (diff_x + 0.5) * (diff_y + 0.5) * 2.0;
    let edge = clamp(diff, 0.0, 1.0);

    textureStore(output_texture, coords, vec4<f32>(edge, edge, edge, 1.0));
}

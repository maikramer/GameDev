// src/shaders/metallic.wgsl

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, write>;

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
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;

    var metallic = 0.0;

    // Gray metals (iron, steel, aluminum, silver)
    if (s < 0.15 && l > 0.3 && l < 0.9) {
        let lum_factor = smooth_step(0.3, 0.8, l);
        let sat_factor = 1.0 - smooth_step(0.0, 0.15, s);
        metallic = max(metallic, lum_factor * sat_factor * 0.9);
    }

    // Gold
    if (h > 0.08 && h < 0.15 && s > 0.3 && l > 0.3) {
        let hue_center = 0.115;
        let hue_dist = abs(h - hue_center);
        let hue_factor = 1.0 - smooth_step(0.0, 0.035, hue_dist);
        let lum_factor = smooth_step(0.3, 0.7, l);
        let sat_factor = smooth_step(0.3, 0.8, s);
        metallic = max(metallic, hue_factor * lum_factor * sat_factor);
    }

    // Copper
    if (h > 0.02 && h < 0.08 && s > 0.4 && l > 0.2) {
        let hue_center = 0.05;
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

    textureStore(output_texture, coords, vec4<f32>(metallic, 0.0, 0.0, 1.0));
}

// WGSL Signed-Distance Field Liquid Glass Material Shader

struct GlobalUniforms {
    viewport_size: vec2<f32>,
    time: f32,
    scale_factor: f32,
}

struct MaterialUniforms {
    rect: vec4<f32>,           // x, y, width, height in logical pixels
    corner_radii: vec4<f32>,   // top-left, top-right, bottom-right, bottom-left
    tint_color: vec4<f32>,     // rgba tint
    optical_params: vec4<f32>, // blur_radius, saturation, specular_rim, edge_width
}

@group(0) @binding(0)
var<uniform> global: GlobalUniforms;

@group(1) @binding(0)
var<uniform> material: MaterialUniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_pos: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Scale unit quad [-1, 1] to target screen rectangle
    let half_size = material.rect.zw * 0.5;
    let center = material.rect.xy + half_size;
    let local_pos = input.position * half_size;
    let screen_coord = center + local_pos;

    // Convert screen coordinates [0, viewport] to normalized device coordinates [-1, 1]
    let ndc_x = (screen_coord.x / global.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_coord.y / global.viewport_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.screen_pos = screen_coord;
    return out;
}

// Signed distance to rounded rectangle with per-corner radii
fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var rad = r.x;
    if (p.x > 0.0) {
        if (p.y > 0.0) {
            rad = r.z; // bottom-right
        } else {
            rad = r.y; // top-right
        }
    } else {
        if (p.y > 0.0) {
            rad = r.w; // bottom-left
        } else {
            rad = r.x; // top-left
        }
    }
    let q = abs(p) - b + vec2<f32>(rad, rad);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - rad;
}

// Smooth polynomial minimum for organic fluid union
fn sd_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let half_size = material.rect.zw * 0.5;
    let center = material.rect.xy + half_size;
    let p = in.screen_pos - center;

    let dist = sd_rounded_box(p, half_size, material.corner_radii);
    let aa = fwidth(dist);
    let alpha = 1.0 - smoothstep(-aa, 0.0, dist);

    if (alpha <= 0.0) {
        discard;
    }

    // Specular rim edge highlight
    let edge_width = material.optical_params.w;
    let rim_strength = material.optical_params.z;
    let rim_factor = smoothstep(-edge_width, 0.0, dist) * (1.0 - smoothstep(0.0, aa, dist));
    let specular = vec4<f32>(1.0, 1.0, 1.0, 1.0) * rim_factor * rim_strength * 0.4;

    // Output composite color
    var color = material.tint_color;
    color = mix(color, specular, rim_factor * 0.5);
    color.a = color.a * alpha;

    return color;
}

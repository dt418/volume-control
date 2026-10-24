// Separable 1D Gaussian Blur Shader (9-tap kernel)

struct BlurUniforms {
    direction: vec2<f32>, // (1.0, 0.0) for horizontal, (0.0, 1.0) for vertical
    radius: f32,
    texel_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: BlurUniforms;

@group(0) @binding(1)
var input_texture: texture_2d<f32>;

@group(0) @binding(2)
var texture_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Fullscreen triangle: 0: (-1, -1), 1: (3, -1), 2: (-1, 3)
    let x = f32(i32(in_vertex_index << 1u) & 2) * 2.0 - 1.0;
    let y = f32(i32(in_vertex_index & 2u) & 2) * 2.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 9-tap Gaussian weights (normalized sum = 1.0)
    let weights = array<f32, 5>(
        0.2270270270,
        0.1945945946,
        0.1216216216,
        0.0540540541,
        0.0162162162
    );

    let offset = uniforms.direction * uniforms.texel_size * uniforms.radius;
    var color = textureSample(input_texture, texture_sampler, in.uv) * weights[0];

    for (var i = 1; i < 5; i++) {
        let sample_offset = offset * f32(i);
        color += textureSample(input_texture, texture_sampler, in.uv + sample_offset) * weights[i];
        color += textureSample(input_texture, texture_sampler, in.uv - sample_offset) * weights[i];
    }

    return color;
}

// Vertex shader

[[block]] // 1.
struct CameraUniform {
    matrix: mat4x4<f32>;
};
[[group(0), binding(0)]] // 2.
var<uniform> camera: CameraUniform;

struct VertexInput {
    [[location(0)]] position: vec2<f32>;
    [[location(1)]] tex_coords: vec2<f32>;
    [[location(2)]] color: vec4<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] tex_coords: vec2<f32>;
    [[location(1)]] color: vec4<f32>;
};

[[stage(vertex)]]
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.color = model.color;
    out.clip_position = camera.matrix * vec4<f32>(model.position.xy, 0.0, 1.0);
    return out;
}

// Fragment shader

[[group(1), binding(0)]]
var t_diffuse: texture_2d<f32>;
[[group(1), binding(1)]]
var s_diffuse: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    var r = textureSample(t_diffuse, s_diffuse, in.tex_coords).r;
    return vec4<f32>(in.color.rgb, in.color.a * r);
}

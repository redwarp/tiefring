// Vertex shader
struct CameraUniform {
    matrix: mat4x4<f32>;
};
[[group(0), binding(0)]] // 2.
var<uniform> camera: CameraUniform;

struct VertexInput {
    [[location(0)]] position: vec2<f32>;
};

struct InstanceInput {
    [[location(1)]] tex_coords: vec4<f32>;
    [[location(2)]] position_matrix_0: vec4<f32>;
    [[location(3)]] position_matrix_1: vec4<f32>;
    [[location(4)]] position_matrix_2: vec4<f32>;
    [[location(5)]] position_matrix_3: vec4<f32>;
    [[location(6)]] color_matrix_0: vec4<f32>;
    [[location(7)]] color_matrix_1: vec4<f32>;
    [[location(8)]] color_matrix_2: vec4<f32>;
    [[location(9)]] color_matrix_3: vec4<f32>;
    [[location(10)]] color_adjust: vec4<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] tex_coords: vec2<f32>;
    [[location(1)]] color_matrix: mat4x4<f32>; // A mat4 takes 4 locations.
    [[location(5)]] color_adjust: vec4<f32>;
};

[[stage(vertex)]]
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let position_matrix = mat4x4<f32>(
        instance.position_matrix_0,
        instance.position_matrix_1,
        instance.position_matrix_2,
        instance.position_matrix_3,
    );
    let color_matrix = mat4x4<f32>(
        instance.color_matrix_0,
        instance.color_matrix_1,
        instance.color_matrix_2,
        instance.color_matrix_3,
    );

    var out: VertexOutput;
    out.clip_position = camera.matrix * position_matrix * vec4<f32>(model.position.xy, 0.0, 1.0);
    out.tex_coords = (vec2<f32>(
        model.position.x * instance.tex_coords.x + instance.tex_coords.y, 
        model.position.y * instance.tex_coords.z + instance.tex_coords.w
    ));
    
    out.color_matrix = color_matrix;
    out.color_adjust = instance.color_adjust;
    return out;
}

// Fragment shader
[[group(1), binding(0)]]
var t_diffuse: texture_2d<f32>;
[[group(1), binding(1)]]
var s_diffuse: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return in.color_matrix * textureSample(t_diffuse, s_diffuse, in.tex_coords) + in.color_adjust;
}
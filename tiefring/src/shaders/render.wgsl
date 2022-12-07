// Vertex shader
struct CameraUniform {
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> camera: mat4x4<f32>;

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct InstanceInput {
    @location(1) tex_coords: vec4<f32>,
    @location(2) position_matrix: vec4<f32>,
    @location(3) position_translate: vec2<f32>,
    @location(4) color_matrix_0: vec4<f32>,
    @location(5) color_matrix_1: vec4<f32>,
    @location(6) color_matrix_2: vec4<f32>,
    @location(7) color_matrix_3: vec4<f32>,
    @location(8) color_adjust: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color_matrix_0: vec4<f32>,
    @location(2) color_matrix_1: vec4<f32>,
    @location(3) color_matrix_2: vec4<f32>,
    @location(4) color_matrix_3: vec4<f32>,
    @location(5) color_adjust: vec4<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    // Reconstruct the position matrix from the affine 2D transform.
    let position_matrix = mat3x3<f32>(
        vec3<f32>(instance.position_matrix.xy, 0.0),
        vec3<f32>(instance.position_matrix.zw, 0.0),
        vec3<f32>(instance.position_translate, 1.0),
    );

    var out: VertexOutput;
    out.clip_position = camera * vec4<f32>(position_matrix * vec3<f32>(model.position, 1.0), 1.0);
    out.tex_coords = (vec2<f32>(
        model.position.x * instance.tex_coords.x + instance.tex_coords.y, 
        model.position.y * instance.tex_coords.z + instance.tex_coords.w
    ));
    
    out.color_matrix_0 = instance.color_matrix_0;
    out.color_matrix_1 = instance.color_matrix_1;
    out.color_matrix_2 = instance.color_matrix_2;
    out.color_matrix_3 = instance.color_matrix_3;
    out.color_adjust = instance.color_adjust;
    return out;
}

// Fragment shader
@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color_matrix = mat4x4<f32>(
        in.color_matrix_0,
        in.color_matrix_1,
        in.color_matrix_2,
        in.color_matrix_3,
    );

    return color_matrix * textureSample(t_diffuse, s_diffuse, in.tex_coords) + in.color_adjust;
}
use std::f32::consts::TAU;

use glam::{Mat4, Vec3};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, BufferUsages, Device, Queue, RenderPass, RenderPipeline, VertexBufferLayout,
};

use crate::{
    cache::{Resetable, ReusableBuffer},
    camera::Camera,
    sprite::{Texture, TextureContext},
    Color, DrawData, OperationBlock, Rect, RenderPosition,
};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub position: [f32; 2],
}

impl Vertex {
    pub(crate) fn description<'a>() -> VertexBufferLayout<'a> {
        use std::mem;
        VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ColorMatrix {
    matrix: [[f32; 4]; 4],
    adjust: [f32; 4],
}

impl ColorMatrix {
    pub fn from_color(color: Color) -> Self {
        let matrix = [
            [color.r, 0.0, 0.0, 0.0],
            [0.0, color.g, 0.0, 0.0],
            [0.0, 0.0, color.b, 0.0],
            [0.0, 0.0, 0.0, color.a],
        ];
        let adjust = [0.0, 0.0, 0.0, 0.0];

        Self { matrix, adjust }
    }

    pub fn for_text(color: Color) -> Self {
        let matrix = [
            [0.0, 0.0, 0.0, color.a],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ];
        let adjust = [color.r, color.g, color.b, 0.0];
        Self { matrix, adjust }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PositionMatrix {
    matrix: [[f32; 4]; 4],
}

impl PositionMatrix {
    pub fn from_mat4(mat4: &Mat4) -> Self {
        let matrix = mat4.to_cols_array_2d();

        Self { matrix }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    tex_coords: [f32; 4],
    position_matrix: PositionMatrix,
    color_matrix: ColorMatrix,
}

impl Instance {
    fn new(tex_coords: &Rect, position: &RenderPosition, color_matrix: &ColorMatrix) -> Self {
        let tex_coords = [
            tex_coords.right - tex_coords.left,
            tex_coords.left,
            tex_coords.bottom - tex_coords.top,
            tex_coords.top,
        ];

        Self {
            tex_coords,
            position_matrix: PositionMatrix::from_mat4(&position.matrix()),
            color_matrix: *color_matrix,
        }
    }

    fn description<'a>() -> VertexBufferLayout<'a> {
        use std::mem;
        VertexBufferLayout {
            array_stride: mem::size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 20]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 24]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 28]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 32]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 36]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub(crate) struct Renderer {
    render_pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl Renderer {
    pub(crate) fn new(device: &Device, texture_context: &TextureContext, camera: &Camera) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/render.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Texture Render Pipeline Layout"),
                bind_group_layouts: &[
                    &camera.camera_bind_group_layout,
                    &texture_context.texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Texture Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::description(), Instance::description()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let vertices = [
            Vertex {
                position: [0.0, 0.0],
            },
            Vertex {
                position: [0.0, 1.0],
            },
            Vertex {
                position: [1.0, 1.0],
            },
            Vertex {
                position: [1.0, 0.0],
            },
        ];
        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices[..]),
            usage: BufferUsages::VERTEX,
        });

        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];
        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&indices[..]),
            usage: BufferUsages::INDEX,
        });

        Self {
            render_pipeline,
            vertex_buffer,
            index_buffer,
        }
    }

    pub(crate) fn render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        instance_buffer: &'a ReusableBuffer,
        count: u32,
        texture: &'a Texture,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(1, &texture.texture_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice());
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..count);
    }
}

pub struct RenderOperation {
    pub(crate) tex_coords: Rect,
    pub(crate) position: RenderPosition,
    pub(crate) color_matrix: ColorMatrix,
}

impl RenderOperation {
    pub fn rotate(&mut self, angle: f32) -> &mut Self {
        let angle = angle.rem_euclid(TAU);
        let x = self.position.scale.x / 2.0;
        let y = self.position.scale.y / 2.0;
        let rotation_matrix = RenderOperation::centered_rotation_matrix(x, y, angle);

        self.position.transformation *= rotation_matrix;
        self
    }

    pub fn translate(&mut self, x: f32, y: f32) -> &mut Self {
        let translation_matrix = Mat4::from_translation(Vec3::new(x, y, 0.0));

        self.position.transformation *= translation_matrix;

        self
    }

    /// Calculate a rotation matrix centered at the x and y passed.
    /// Using this https://www.brainvoyager.com/bv/doc/UsersGuide/CoordsAndTransforms/SpatialTransformationMatrices.html
    /// for the base matrices, and using wolfgram alpha to reduce the operations of translate, rotate, translate back to one single matrix.
    fn centered_rotation_matrix(x: f32, y: f32, angle: f32) -> Mat4 {
        let cos = angle.cos();
        let sin = angle.sin();
        let cols = [
            [cos, sin, 0.0, 0.0],
            [-sin, cos, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                x * (-cos) + y * sin + x,
                -x * sin + y * (-cos) + y,
                0.0,
                1.0,
            ],
        ];
        Mat4::from_cols_array_2d(&cols)
    }
}

pub(crate) struct RenderPreper {
    instances: Vec<Instance>,
}

impl RenderPreper {
    pub fn new() -> Self {
        Self { instances: vec![] }
    }

    pub fn prepare(
        &mut self,
        buffer_cache: &mut crate::cache::BufferCache,
        device: &Device,
        queue: &Queue,
        operation_block: OperationBlock,
    ) -> Option<DrawData> {
        let count = operation_block.operations.len();
        if count == 0 {
            return None;
        }

        self.instances.reset_with_capacity(count);
        self.instances
            .extend(operation_block.operations.iter().map(|operation| {
                Instance::new(
                    &operation.tex_coords,
                    &operation.position,
                    &operation.color_matrix,
                )
            }));

        let instance_buffer = buffer_cache.get_buffer(
            device,
            queue,
            bytemuck::cast_slice(&self.instances[..]),
            BufferUsages::VERTEX,
        );

        Some(DrawData {
            instance_buffer,
            count: count as u32,
            texture: operation_block.texture,
        })
    }
}

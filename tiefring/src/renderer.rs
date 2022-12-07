use glam::{Affine2, Vec2};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, BufferUsages, Device, Queue, RenderPass, RenderPipeline, VertexBufferLayout,
};

use crate::{
    cache::TransformCache, camera::Camera, sprite::TextureContext, Color, DrawData, OperationBlock,
    Rect,
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
    pub const fn from_color(color: Color) -> Self {
        let matrix = [
            [color.r, 0.0, 0.0, 0.0],
            [0.0, color.g, 0.0, 0.0],
            [0.0, 0.0, color.b, 0.0],
            [0.0, 0.0, 0.0, color.a],
        ];
        let adjust = [0.0, 0.0, 0.0, 0.0];

        Self { matrix, adjust }
    }

    pub const fn for_text(color: Color) -> Self {
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
    matrix: [[f32; 2]; 2],
    // We only consider the first 2, the rest is padding because of the 16 byte alignment.
    translate: [f32; 4],
}

impl From<Affine2> for PositionMatrix {
    fn from(affine2: Affine2) -> Self {
        let translate = [affine2.translation.x, affine2.translation.y, 0.0, 0.0];
        Self {
            matrix: affine2.matrix2.to_cols_array_2d(),
            translate,
        }
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
    fn new(tex_coords: Rect, position: RenderPosition, color_matrix: ColorMatrix) -> Self {
        let tex_coords = [
            tex_coords.width,
            tex_coords.left,
            tex_coords.height,
            tex_coords.top,
        ];

        Self {
            tex_coords,
            position_matrix: position.into_affine2().into(),
            color_matrix,
        }
    }

    const fn description<'a>() -> VertexBufferLayout<'a> {
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
                    format: wgpu::VertexFormat::Float32x2,
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
        draw_data: &'a [DrawData],
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        for draw_data in draw_data.iter() {
            render_pass.set_bind_group(1, &draw_data.texture.texture_bind_group, &[]);
            render_pass.set_vertex_buffer(1, draw_data.instance_buffer.slice());
            render_pass.draw_indexed(0..6, 0, 0..draw_data.count);
        }
    }
}

struct RenderPosition {
    transformation: Affine2,
    scale: Vec2,
}

impl RenderPosition {
    fn translate(&mut self, x: f32, y: f32) -> &mut Self {
        let translation_matrix = Affine2::from_translation(Vec2::new(x, y));

        self.transformation = self.transformation * translation_matrix;

        self
    }

    fn rotate(&mut self, angle: f32) -> &mut Self {
        let angle = angle.rem_euclid(std::f32::consts::TAU);
        let x = self.scale.x / 2.0;
        let y = self.scale.y / 2.0;
        let rotation_matrix = Self::centered_rotation_affine(x, y, angle);

        self.transformation = self.transformation * rotation_matrix;
        self
    }

    fn centered_rotation_affine(x: f32, y: f32, angle: f32) -> Affine2 {
        Affine2::from_translation(Vec2::new(x, y))
            * Affine2::from_angle(angle)
            * Affine2::from_translation(Vec2::new(-x, -y))
    }

    fn into_affine2(self) -> Affine2 {
        let scale = Affine2::from_scale(self.scale);
        self.transformation * scale
    }
}

impl From<Rect> for RenderPosition {
    fn from(rect: Rect) -> Self {
        let transformation = Affine2::from_translation(Vec2::new(rect.left, rect.top));
        let scale = Vec2::new(rect.width, rect.height);
        Self {
            transformation,
            scale,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Transform {
    Rotate(f32),
    Translate { x: f32, y: f32 },
}

pub struct RenderOperation {
    pub(crate) tex_coords: Rect,
    pub(crate) rect: Rect,
    pub(crate) color_matrix: ColorMatrix,
    pub(crate) transforms: Vec<Transform>,
}

impl RenderOperation {
    pub fn rotate(&mut self, angle: f32) -> &mut Self {
        self.transforms.push(Transform::Rotate(angle));

        self
    }

    pub fn translate(&mut self, x: f32, y: f32) -> &mut Self {
        self.transforms.push(Transform::Translate { x, y });

        self
    }

    pub fn alpha(&mut self, alpha: f32) -> &mut Self {
        self.color_matrix.matrix[3][3] *= alpha;

        self
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
        transform_cache: &mut TransformCache,
    ) -> Option<DrawData> {
        let count = operation_block.operations.len();
        if count == 0 {
            return None;
        }

        self.instances.clear();
        self.instances
            .extend(operation_block.operations.into_iter().map(|operation| {
                let mut position: RenderPosition = operation.rect.into();

                for transform in &operation.transforms {
                    match transform {
                        Transform::Rotate(angle) => {
                            position.rotate(*angle);
                        }
                        Transform::Translate { x, y } => {
                            position.translate(*x, *y);
                        }
                    }
                }

                transform_cache.release(operation.transforms);
                Instance::new(operation.tex_coords, position, operation.color_matrix)
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

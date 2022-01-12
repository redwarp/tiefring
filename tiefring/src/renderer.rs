use std::rc::Rc;

use glam::Mat4;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroupLayout, Buffer, BufferUsages, RenderPass, RenderPipeline, Sampler, SamplerBindingType,
    VertexBufferLayout,
};

use crate::{
    cache::{Resetable, ReusableBuffer},
    camera::Camera,
    sprite::Texture,
    Color, DrawData, Rect, WgpuContext,
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
}

impl ColorMatrix {
    pub fn from_color(color: Color) -> Self {
        let matrix = [
            [color.r, 0.0, 0.0, 0.0],
            [0.0, color.g, 0.0, 0.0],
            [0.0, 0.0, color.b, 0.0],
            [0.0, 0.0, 0.0, color.a],
        ];

        Self { matrix }
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
    fn new(tex_coords: &Rect, position: &Mat4, color_matrix: &ColorMatrix) -> Self {
        let tex_coords = [
            tex_coords.right - tex_coords.left,
            tex_coords.left,
            tex_coords.bottom - tex_coords.top,
            tex_coords.top,
        ];

        Self {
            tex_coords,
            position_matrix: PositionMatrix::from_mat4(position),
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
            ],
        }
    }
}

pub(crate) struct Renderer {
    render_pipeline: RenderPipeline,
    pub sampler: Sampler,
    pub texture_bind_group_layout: BindGroupLayout,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl Renderer {
    pub(crate) fn new(context: &WgpuContext, camera: &Camera) -> Self {
        let shader = context
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/render.wgsl").into()),
            });

        let texture_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                    label: Some("texture_bind_group_layout"),
                });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Texture Render Pipeline Layout"),
                    bind_group_layouts: &[
                        &camera.camera_bind_group_layout,
                        &texture_bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Texture Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[Vertex::description(), Instance::description()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        // 3.
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[wgpu::ColorTargetState {
                            // 4.
                            format: context.config.format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        }],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                        polygon_mode: wgpu::PolygonMode::Fill,
                        // Requires Features::DEPTH_CLAMPING
                        unclipped_depth: false,
                        // Requires Features::CONSERVATIVE_RASTERIZATION
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

        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
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
        let vertex_buffer = context.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices[..]),
            usage: BufferUsages::VERTEX,
        });

        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];
        let index_buffer = context.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&indices[..]),
            usage: BufferUsages::INDEX,
        });

        Self {
            render_pipeline,
            sampler,
            texture_bind_group_layout,
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

pub(crate) struct RenderOperation {
    pub tex_coords: Rect,
    pub position: Mat4,
    pub color_matrix: ColorMatrix,
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
        context: &WgpuContext,
        texture: Rc<Texture>,
        operations: &[RenderOperation],
    ) -> Option<crate::DrawData> {
        let count = operations.len();
        self.instances.reset_with_capacity(count);
        self.instances.extend(operations.iter().map(|operation| {
            Instance::new(
                &operation.tex_coords,
                &operation.position,
                &operation.color_matrix,
            )
        }));

        let instance_buffer = buffer_cache.get_buffer(
            &context,
            bytemuck::cast_slice(&self.instances[..]),
            BufferUsages::VERTEX,
        );

        Some(DrawData::Render {
            instance_buffer,
            count: count as u32,
            texture,
        })
    }
}

use std::{cell::RefCell, rc::Rc};

use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Buffer, Color, PipelineLayout, RenderPass,
    RenderPipeline, Sampler, ShaderModule,
};

use crate::{
    camera::{self, Camera},
    sprite::{TextureId, TextureRepository},
    DrawRectOperation, DrawTextureOperation, WgpuContext,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl ColorVertex {
    fn description<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ColorVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextureVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl TextureVertex {
    fn description<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextureVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2, // NEW!
                },
            ],
        }
    }
}

fn color_to_float_array(color: Color) -> [f32; 4] {
    [
        color.r as f32,
        color.g as f32,
        color.b as f32,
        color.a as f32,
    ]
}

pub(crate) struct ColorRenderer {
    render_pipeline: RenderPipeline,
    vertex_buffer: Option<(Buffer, Buffer)>,
}

impl ColorRenderer {
    pub(crate) fn new(context: &WgpuContext, camera: &Camera) -> Self {
        let shader = context
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/color.wgsl").into()),
            });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Color Render Pipeline Layout"),
                    bind_group_layouts: &[&camera.camera_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Color Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",                 // 1.
                        buffers: &[ColorVertex::description()], // 2.
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
                        topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw, // 2.
                        cull_mode: Some(wgpu::Face::Back),
                        // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                        polygon_mode: wgpu::PolygonMode::Fill,
                        // Requires Features::DEPTH_CLAMPING
                        clamp_depth: false,
                        // Requires Features::CONSERVATIVE_RASTERIZATION
                        conservative: false,
                    },
                    depth_stencil: None, // 1.
                    multisample: wgpu::MultisampleState {
                        count: 1,                         // 2.
                        mask: !0,                         // 3.
                        alpha_to_coverage_enabled: false, // 4.
                    },
                });

        ColorRenderer {
            render_pipeline,
            vertex_buffer: None,
        }
    }

    pub(crate) fn render<'a>(
        &'a mut self,
        render_pass: &mut RenderPass<'a>,
        context: &WgpuContext,
        camera: &'a Camera,
        operations: &[DrawRectOperation],
    ) {
        let vertices: Vec<_> = operations
            .iter()
            .flat_map(|operation| {
                let rect = &operation.0;
                let color: [f32; 4] = color_to_float_array(operation.1);
                [
                    ColorVertex {
                        position: [rect.left as f32, rect.top as f32],
                        color: color,
                    },
                    ColorVertex {
                        position: [rect.left as f32, rect.bottom as f32],
                        color: color,
                    },
                    ColorVertex {
                        position: [rect.right as f32, rect.bottom as f32],
                        color: color,
                    },
                    ColorVertex {
                        position: [rect.right as f32, rect.top as f32],
                        color: color,
                    },
                ]
            })
            .collect();

        let indices: Vec<u16> = (0..operations.len())
            .flat_map(|index| {
                let step: u16 = index as u16 * 4;
                [step + 0, step + 1, step + 2, step + 2, step + 3, step + 0]
            })
            .collect();

        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices[..]),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&indices[..]),
                usage: wgpu::BufferUsages::INDEX,
            });

        self.vertex_buffer = Some((vertex_buffer, index_buffer));

        if let Some((vertex_buffer, index_buffer)) = &self.vertex_buffer {
            let count = indices.len() as u32;
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &camera.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..count, 0, 0..1);
        }
    }
}

pub struct TextureRenderer {
    shader: ShaderModule,
    sampler: Sampler,
    texture_bind_group_layout: BindGroupLayout,
    render_pipeline_layout: PipelineLayout,
    vertex_buffer: Vec<(Buffer, Buffer, RenderPipeline, BindGroup)>,
}

impl TextureRenderer {
    pub(crate) fn new(context: &WgpuContext, camera: &Camera) -> Self {
        let shader = context
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sprite.wgsl").into()),
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
                            ty: wgpu::BindingType::Sampler {
                                // This is only for TextureSampleType::Depth
                                comparison: false,
                                // This should be true if the sample_type of the texture is:
                                //     TextureSampleType::Float { filterable: true }
                                // Otherwise you'll get an error.
                                filtering: true,
                            },
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

        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        TextureRenderer {
            shader,
            sampler,
            texture_bind_group_layout,
            render_pipeline_layout,
            vertex_buffer: vec![],
        }
    }

    pub(crate) fn render<'a>(
        &'a mut self,
        render_pass: &mut RenderPass<'a>,
        context: &'a WgpuContext,
        texture_repository: &Rc<RefCell<TextureRepository>>,
        camera: &'a Camera,
        operations: &[DrawTextureOperation],
    ) {
        self.vertex_buffer.clear();
        for operation in operations {
            let vertices = [
                TextureVertex {
                    position: [operation.destination.left, operation.destination.top],
                    tex_coords: [operation.tex_coords.left, operation.tex_coords.top],
                },
                TextureVertex {
                    position: [operation.destination.left, operation.destination.bottom],
                    tex_coords: [operation.tex_coords.left, operation.tex_coords.bottom],
                },
                TextureVertex {
                    position: [operation.destination.right, operation.destination.bottom],
                    tex_coords: [operation.tex_coords.right, operation.tex_coords.bottom],
                },
                TextureVertex {
                    position: [operation.destination.right, operation.destination.top],
                    tex_coords: [operation.tex_coords.right, operation.tex_coords.top],
                },
            ];
            let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

            let vertex_buffer =
                context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Vertex Buffer"),
                        contents: bytemuck::cast_slice(&vertices[..]),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
            let index_buffer =
                context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Index Buffer"),
                        contents: bytemuck::cast_slice(&indices[..]),
                        usage: wgpu::BufferUsages::INDEX,
                    });

            let texture = texture_repository
                .borrow()
                .get_texture(&operation.texture_id);
            if let Some(texture) = texture {
                let texture_bind_group: BindGroup =
                    context
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &self.texture_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(
                                        &texture.texture_view,
                                    ),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                                },
                            ],
                            label: Some("diffuse_bind_group"),
                        });

                let render_pipeline =
                    context
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some("Texture Render Pipeline"),
                            layout: Some(&self.render_pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &self.shader,
                                entry_point: "vs_main",                   // 1.
                                buffers: &[TextureVertex::description()], // 2.
                            },
                            fragment: Some(wgpu::FragmentState {
                                // 3.
                                module: &self.shader,
                                entry_point: "fs_main",
                                targets: &[wgpu::ColorTargetState {
                                    // 4.
                                    format: context.config.format,
                                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                                    write_mask: wgpu::ColorWrites::ALL,
                                }],
                            }),
                            primitive: wgpu::PrimitiveState {
                                topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                                strip_index_format: None,
                                front_face: wgpu::FrontFace::Ccw, // 2.
                                cull_mode: Some(wgpu::Face::Back),
                                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                                polygon_mode: wgpu::PolygonMode::Fill,
                                // Requires Features::DEPTH_CLAMPING
                                clamp_depth: false,
                                // Requires Features::CONSERVATIVE_RASTERIZATION
                                conservative: false,
                            },
                            depth_stencil: None, // 1.
                            multisample: wgpu::MultisampleState {
                                count: 1,                         // 2.
                                mask: !0,                         // 3.
                                alpha_to_coverage_enabled: false, // 4.
                            },
                        });

                self.vertex_buffer.push((
                    vertex_buffer,
                    index_buffer,
                    render_pipeline,
                    texture_bind_group,
                ));
            }
        }

        for (vertex_buffer, index_buffer, render_pipeline, texture_bind_group) in
            &self.vertex_buffer
        {
            render_pass.set_pipeline(render_pipeline);
            render_pass.set_bind_group(0, &camera.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
    }
}

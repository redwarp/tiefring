use wgpu::{RenderPass, RenderPipeline};

use crate::{
    cache::{BufferCache, Resetable, ReusableBuffer},
    camera::Camera,
    Color, DrawData, DrawDataPreper, Rect, WgpuContext,
};

pub(crate) struct DrawRectOperation(pub Rect, pub Color);

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

pub(crate) struct ColorRenderer {
    render_pipeline: RenderPipeline,
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
                        unclipped_depth: false,
                        // Requires Features::CONSERVATIVE_RASTERIZATION
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,                         // 2.
                        mask: !0,                         // 3.
                        alpha_to_coverage_enabled: false, // 4.
                    },
                    multiview: None,
                });

        Self { render_pipeline }
    }

    pub(crate) fn render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        vertex_buffer: &'a ReusableBuffer,
        index_buffer: &'a ReusableBuffer,
        count: u32,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.buffer.slice(..vertex_buffer.current_size));
        render_pass.set_index_buffer(
            index_buffer.buffer.slice(..vertex_buffer.current_size),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..count, 0, 0..1);
    }
}

pub struct ColorDataPreper {
    vertices: Vec<ColorVertex>,
    indices: Vec<u16>,
}

impl ColorDataPreper {
    pub fn new() -> Self {
        Self {
            vertices: vec![],
            indices: vec![],
        }
    }
}

impl DrawDataPreper<DrawRectOperation, &WgpuContext> for ColorDataPreper {
    fn prepare(
        &mut self,
        buffer_cache: &mut BufferCache,
        context: &WgpuContext,
        operations: &[DrawRectOperation],
    ) -> Option<DrawData> {
        let vertices = &mut self.vertices;
        let capacity = operations.len() * 4;

        vertices.reset_with_capacity(capacity);

        vertices.extend(
            operations
                .iter()
                .flat_map(|&DrawRectOperation(rect, color)| {
                    let color: [f32; 4] = color.as_float_array();
                    [
                        ColorVertex {
                            position: [rect.left as f32, rect.top as f32],
                            color,
                        },
                        ColorVertex {
                            position: [rect.left as f32, rect.bottom as f32],
                            color,
                        },
                        ColorVertex {
                            position: [rect.right as f32, rect.bottom as f32],
                            color,
                        },
                        ColorVertex {
                            position: [rect.right as f32, rect.top as f32],
                            color,
                        },
                    ]
                }),
        );

        let indices = &mut self.indices;
        indices.reset_with_capacity(capacity);
        indices.extend((0..operations.len()).flat_map(|index| {
            let step: u16 = index as u16 * 4;
            [step, step + 1, step + 2, step + 2, step + 3, step]
        }));

        let vertex_buffer = buffer_cache.get_buffer(
            context,
            bytemuck::cast_slice(&vertices[..]),
            wgpu::BufferUsages::VERTEX,
        );

        let index_buffer = buffer_cache.get_buffer(
            context,
            bytemuck::cast_slice(&indices[..]),
            wgpu::BufferUsages::INDEX,
        );

        Some(DrawData::Color {
            vertex_buffer,
            index_buffer,
            count: indices.len() as u32,
        })
    }
}

use wgpu::{util::DeviceExt, Buffer, Color, RenderPass, RenderPipeline};

use crate::{DrawRectOperation, WgpuContext};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorVertex {
    position: [f32; 2],
    color: [f32; 4],
}

fn color_to_float_array(color: Color) -> [f32; 4] {
    [
        color.r as f32,
        color.g as f32,
        color.b as f32,
        color.a as f32,
    ]
}

// impl Into<[f32; 4]> for Color {
//     fn into(self) -> [f32; 4] {
//         [self.r as f32, self.g as f32, self.b as f32, self.a as f32]
//     }
// }

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
    vertex_buffer: Option<Buffer>,
}

impl ColorRenderer {
    pub(crate) fn new(context: &WgpuContext) -> Self {
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
                    bind_group_layouts: &[],
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
                        front_face: wgpu::FrontFace::Cw, // 2.
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
        operations: &[DrawRectOperation],
    ) {
        let vertices: Vec<_> = operations
            .iter()
            .flat_map(|operation| {
                let rect = &operation.0 * 0.2;
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
                        position: [rect.right as f32, rect.bottom as f32],
                        color: color,
                    },
                    ColorVertex {
                        position: [rect.right as f32, rect.top as f32],
                        color: color,
                    },
                    ColorVertex {
                        position: [rect.left as f32, rect.top as f32],
                        color: color,
                    },
                ]
            })
            .collect();

        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices[..]),
                usage: wgpu::BufferUsages::VERTEX,
            });

        self.vertex_buffer = Some(vertex_buffer);

        if let Some(vertex_buffer) = &self.vertex_buffer {
            let count = vertices.len() as u32;
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..count, 0..1)
        }
    }
}

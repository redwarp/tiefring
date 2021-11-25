use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

use itertools::Itertools;
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Buffer, PipelineLayout, RenderPass,
    RenderPipeline, Sampler, ShaderModule,
};

use crate::{
    camera::Camera, renderer, Canvas, DepthTexture, DrawTextureOperation, Rect, Size, WgpuContext,
};

pub struct Sprite {
    pub(crate) texture_id: TextureId,
    pub(crate) size: Size,
    pub(crate) tex_coords: Rect,
    texture_repository: Rc<RefCell<TextureRepository>>,
}

impl Drop for Sprite {
    fn drop(&mut self) {
        self.texture_repository
            .borrow_mut()
            .release_texture(&self.texture_id);
    }
}

impl Sprite {
    pub fn load_data<S>(canvas: &mut Canvas, rgba: &[u8], dimensions: S) -> Self
    where
        S: Into<Size> + Copy,
    {
        let texture = Texture::new(canvas, rgba, dimensions);
        let tex_coord = Rect {
            left: 0.0,
            top: 0.0,
            right: 1.0,
            bottom: 1.0,
        };

        let texture_id = {
            let mut repository = canvas.texture_repository.borrow_mut();
            repository.store_texture(texture)
        };

        Sprite {
            texture_id,
            size: dimensions.into(),
            tex_coords: tex_coord,
            texture_repository: canvas.texture_repository.clone(),
        }
    }

    pub fn load_image<P: AsRef<Path>>(canvas: &mut Canvas, path: P) -> Option<Self> {
        let image = image::open(path).ok()?;

        let rgba = image.as_rgba8()?;

        use image::GenericImageView;
        let dimensions = image.dimensions();

        Some(Sprite::load_data(canvas, rgba, dimensions))
    }
}

pub struct TileSet {
    pub(crate) texture_id: TextureId,
    pub(crate) dimensions: Size,
    pub(crate) tile_dimensions: Size,
    texture_repository: Rc<RefCell<TextureRepository>>,
}

impl TileSet {
    pub fn load_data<S, TS>(
        canvas: &mut Canvas,
        rgba: &[u8],
        dimensions: S,
        tile_dimensions: TS,
    ) -> Self
    where
        S: Into<Size> + Copy,
        TS: Into<Size> + Copy,
    {
        let texture = Texture::new(canvas, rgba, dimensions);

        let texture_id = {
            let mut repository = canvas.texture_repository.borrow_mut();
            repository.store_texture(texture)
        };

        TileSet {
            texture_id,
            dimensions: dimensions.into(),
            tile_dimensions: tile_dimensions.into(),
            texture_repository: canvas.texture_repository.clone(),
        }
    }

    pub fn load_image<P, S>(canvas: &mut Canvas, path: P, tile_dimensions: S) -> Option<Self>
    where
        P: AsRef<Path>,
        S: Into<Size> + Copy,
    {
        let image = image::open(path).ok()?;

        let rgba = image.as_rgba8()?;

        use image::GenericImageView;
        let dimensions = image.dimensions();

        Some(TileSet::load_data::<(u32, u32), S>(
            canvas,
            rgba,
            dimensions.into(),
            tile_dimensions,
        ))
    }

    pub fn sprite(x: u32, y: u32) {}
}

impl Drop for TileSet {
    fn drop(&mut self) {
        self.texture_repository
            .borrow_mut()
            .release_texture(&self.texture_id);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Hash, Clone, Copy)]
pub(crate) struct TextureId(u32);

pub(crate) struct Texture {
    pub size: Size,
    pub texture_bind_group: BindGroup,
    pub render_pipeline: RenderPipeline,
}

impl Texture {
    fn new<S: Into<Size>>(canvas: &Canvas, rgba: &[u8], dimensions: S) -> Self {
        let dimensions: Size = dimensions.into();
        let texture_size = wgpu::Extent3d {
            width: dimensions.width,
            height: dimensions.height,
            depth_or_array_layers: 1,
        };
        let wgpu_texture = canvas
            .wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                label: Some("texture"),
            });

        canvas.wgpu_context.queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::ImageCopyTexture {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            rgba,
            // The layout of the texture
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.width),
                rows_per_image: std::num::NonZeroU32::new(dimensions.height),
            },
            texture_size,
        );

        let texture_view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let texture_bind_group: BindGroup =
            canvas
                .wgpu_context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &canvas.texture_renderer.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(
                                &canvas.texture_renderer.sampler,
                            ),
                        },
                    ],
                    label: Some("diffuse_bind_group"),
                });

        let render_pipeline =
            canvas
                .wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Texture Render Pipeline"),
                    layout: Some(&canvas.texture_renderer.render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &canvas.texture_renderer.shader,
                        entry_point: "vs_main",                   // 1.
                        buffers: &[TextureVertex::description()], // 2.
                    },
                    fragment: Some(wgpu::FragmentState {
                        // 3.
                        module: &canvas.texture_renderer.shader,
                        entry_point: "fs_main",
                        targets: &[wgpu::ColorTargetState {
                            // 4.
                            format: canvas.wgpu_context.config.format,
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
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DepthTexture::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::GreaterEqual, // 1.
                        stencil: wgpu::StencilState::default(),             // 2.
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: 1,                         // 2.
                        mask: !0,                         // 3.
                        alpha_to_coverage_enabled: false, // 4.
                    },
                });

        let texture = Texture {
            size: dimensions,
            texture_bind_group,
            render_pipeline,
        };
        texture
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct TextureVertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl TextureVertex {
    pub(crate) fn description<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextureVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2, // NEW!
                },
            ],
        }
    }
}

pub(crate) struct TextureRepository {
    next_id: u32,
    textures: HashMap<TextureId, Rc<Texture>>,
    use_count: HashMap<TextureId, u32>,
}

impl TextureRepository {
    pub fn new() -> Self {
        TextureRepository {
            next_id: 0,
            textures: HashMap::new(),
            use_count: HashMap::new(),
        }
    }

    fn store_texture(&mut self, texture: Texture) -> TextureId {
        let texture_id = TextureId(self.next_id);
        self.next_id += 1;

        let texture = Rc::new(texture);
        self.textures.insert(texture_id, texture);
        self.use_count.insert(texture_id, 1);

        texture_id
    }

    fn get_texture(&self, texture_id: &TextureId) -> Option<Rc<Texture>> {
        self.textures.get(texture_id).map(|texture| texture.clone())
    }

    fn release_texture(&mut self, texture_id: &TextureId) {
        if let Some(count) = self.use_count.get_mut(texture_id) {
            let new_count = *count - 1;
            *count = new_count;
            if new_count == 0 {
                self.use_count.remove(texture_id);
                self.textures.remove(texture_id);
            }
        }
    }
}

pub(crate) struct TextureRenderer {
    shader: ShaderModule,
    sampler: Sampler,
    texture_bind_group_layout: BindGroupLayout,
    render_pipeline_layout: PipelineLayout,
    vertex_buffer: Vec<(Buffer, Rc<Texture>, Vec<u16>, Buffer)>,
}

impl TextureRenderer {
    pub(crate) fn new(context: &WgpuContext, camera: &Camera) -> Self {
        let shader = context
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/texture.wgsl").into()),
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
        operations: &Vec<DrawTextureOperation>,
    ) {
        self.vertex_buffer.clear();
        let sorted_op = operations.iter().into_group_map_by(|op| op.texture_id);
        for key in sorted_op.keys() {
            if let Some(operations) = sorted_op.get(key) {
                let vertices: Vec<_> = operations
                    .iter()
                    .flat_map(|operation| {
                        let depth = renderer::depth(operation.index);
                        [
                            TextureVertex {
                                position: [
                                    operation.destination.left,
                                    operation.destination.top,
                                    depth,
                                ],
                                tex_coords: [operation.tex_coords.left, operation.tex_coords.top],
                            },
                            TextureVertex {
                                position: [
                                    operation.destination.left,
                                    operation.destination.bottom,
                                    depth,
                                ],
                                tex_coords: [
                                    operation.tex_coords.left,
                                    operation.tex_coords.bottom,
                                ],
                            },
                            TextureVertex {
                                position: [
                                    operation.destination.right,
                                    operation.destination.bottom,
                                    depth,
                                ],
                                tex_coords: [
                                    operation.tex_coords.right,
                                    operation.tex_coords.bottom,
                                ],
                            },
                            TextureVertex {
                                position: [
                                    operation.destination.right,
                                    operation.destination.top,
                                    depth,
                                ],
                                tex_coords: [operation.tex_coords.right, operation.tex_coords.top],
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

                let texture = texture_repository.borrow().get_texture(key);
                if let Some(texture) = texture {
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
                    self.vertex_buffer
                        .push((vertex_buffer, texture, indices, index_buffer));
                }
            }
        }

        for (vertex_buffer, texture, indices, index_buffer) in &self.vertex_buffer {
            let indice_count = indices.len() as u32;
            render_pass.set_pipeline(&texture.render_pipeline);
            render_pass.set_bind_group(0, &camera.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &texture.texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..indice_count, 0, 0..1);
        }
    }
}

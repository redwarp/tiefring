use std::{cell::RefCell, collections::HashMap, rc::Rc};

use wgpu::{BindGroup, RenderPipeline};

use crate::{renderer::TextureVertex, Canvas, Rect, Size};

pub struct Sprite {
    pub(crate) texture_id: TextureId,
    pub(crate) rect: Rect,
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
    pub fn load(canvas: &mut Canvas) -> Self {
        let image = image::load_from_memory(include_bytes!("sprites/p1_jump.png")).unwrap();
        let rgba = image.as_rgba8().unwrap();

        use image::GenericImageView;
        let dimensions = image.dimensions();

        let size = Size {
            width: dimensions.0,
            height: dimensions.1,
        };

        let texture = Texture::new(canvas, rgba, dimensions);
        let rect = Rect {
            left: 0.0,
            top: 0.0,
            right: size.width as f32,
            bottom: size.height as f32,
        };
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
            rect,
            tex_coords: tex_coord,
            texture_repository: canvas.texture_repository.clone(),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub(crate) struct TextureId(u32);

pub(crate) struct Texture {
    pub size: Size,
    pub texture_bind_group: BindGroup,
    pub render_pipeline: RenderPipeline,
}

impl Texture {
    fn new(canvas: &Canvas, rgba: &[u8], dimensions: (u32, u32)) -> Self {
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
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
                bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
                rows_per_image: std::num::NonZeroU32::new(dimensions.1),
            },
            texture_size,
        );

        let texture_view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let size = Size {
            width: dimensions.0,
            height: dimensions.1,
        };

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
                    depth_stencil: None, // 1.
                    multisample: wgpu::MultisampleState {
                        count: 1,                         // 2.
                        mask: !0,                         // 3.
                        alpha_to_coverage_enabled: false, // 4.
                    },
                });

        let texture = Texture {
            size,
            texture_bind_group,
            render_pipeline,
        };
        texture
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

    pub(crate) fn get_texture(&self, texture_id: &TextureId) -> Option<Rc<Texture>> {
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

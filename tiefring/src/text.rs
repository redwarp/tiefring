use std::{cell::RefCell, collections::HashMap, rc::Rc};

use fontdue::Metrics;

use rect_packer::Packer;
use wgpu::{BindGroup, BindGroupLayout, RenderPass, RenderPipeline, Sampler};

use crate::{
    camera::Camera,
    sprite::Texture,
    sprite::{TextureId, TextureVertex, TEXTURE_INDEX},
    Canvas, OperationBlock, Rect, WgpuContext,
};

pub struct Font {
    font: fontdue::Font,
    font_cache: HashMap<u32, Rc<RefCell<FontForPx>>>,
}

static CACHE_WIDTH: u32 = 1024;

impl Font {
    pub fn load_font() -> Self {
        let font = include_bytes!("../../sample/fonts/Roboto-Regular.ttf") as &[u8];
        let font = fontdue::Font::from_bytes(font, fontdue::FontSettings::default()).unwrap();
        let font_cache = HashMap::new();

        Self { font, font_cache }
    }

    pub fn test<S: Into<String>>(&mut self, canvas: &Canvas, px: u32, text: S) {
        self.prepare_chars(px, text, &canvas.wgpu_context, &canvas.text_context);
    }

    fn get_font_for_px(
        &mut self,
        px: u32,
        wgpu_context: &WgpuContext,
        text_context: &TextContext,
    ) -> Rc<RefCell<FontForPx>> {
        self.font_cache
            .entry(px)
            .or_insert_with(|| Rc::new(RefCell::new(FontForPx::new(px))))
            .clone()
    }

    fn prepare_chars<S: Into<String>>(
        &mut self,
        px: u32,
        string: S,
        wgpu_context: &WgpuContext,
        text_context: &TextContext,
    ) {
        let string: String = string.into();
        let mut chars: Vec<_> = string.chars().collect();
        chars.sort();
        chars.dedup();

        let cache = self.font_cache.get(&px);

        let missing_chars = if let Some(cache) = cache {
            chars
                .into_iter()
                .filter(|character| !cache.borrow().contains(character))
                .collect()
        } else {
            chars
        };

        let mut cache = self
            .font_cache
            .entry(px)
            .or_insert_with(|| Rc::new(RefCell::new(FontForPx::new(px))))
            .borrow_mut();

        if missing_chars.len() > 0 {
            println!(
                "We are missing {} chars: {:?}",
                missing_chars.len(),
                missing_chars
            );
        }

        for missing_char in missing_chars {
            cache.create_character(missing_char, &self.font, wgpu_context, text_context);
        }
    }
}

struct Character {
    metrics: Metrics,
    tex_coords: Rect,
    character: char,
    rect: rect_packer::Rect,
}

pub(crate) struct FontForPx {
    px: u32,
    texture: Option<Rc<Texture>>,
    packer: Packer,
    characters: HashMap<char, Character>,
}

impl FontForPx {
    fn new(px: u32) -> Self {
        let texture = None;
        let packer = Packer::new(rect_packer::Config {
            width: CACHE_WIDTH as i32,
            height: CACHE_WIDTH as i32,
            border_padding: 0,
            rectangle_padding: 0,
        });
        let characters = HashMap::new();

        Self {
            px,
            texture,
            packer,
            characters,
        }
    }

    fn contains(&self, character: &char) -> bool {
        self.characters.contains_key(character)
    }

    fn create_character(
        &mut self,
        char: char,
        font: &fontdue::Font,
        wgpu_context: &WgpuContext,
        text_context: &TextContext,
    ) -> Option<&Character> {
        let texture = self.texture.get_or_insert_with(|| {
            Rc::new(FontForPx::font_texture(
                wgpu_context,
                &text_context.texture_bind_group_layout,
                &text_context.sampler,
            ))
        });

        let (metrics, bitmap) = font.rasterize(char, self.px as f32);
        let packed = self
            .packer
            .pack(metrics.width as i32, metrics.height as i32, false);

        println!(
            "Creating char {}. Pack = {:?}, bitmap bytes {}",
            char,
            packed,
            bitmap.len()
        );

        if let Some(packed) = packed {
            wgpu_context.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: packed.x as u32,
                        y: packed.y as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                // The actual pixel data
                &bitmap,
                // The layout of the texture
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: std::num::NonZeroU32::new(metrics.width as u32),
                    rows_per_image: std::num::NonZeroU32::new(metrics.height as u32),
                },
                wgpu::Extent3d {
                    width: metrics.width as u32,
                    height: metrics.height as u32,
                    depth_or_array_layers: 1,
                },
            );

            let tex_coords = Rect {
                left: packed.left() as f32 / 1024.0,
                top: packed.top() as f32 / 1024.0,
                right: packed.right() as f32 / 1024.0,
                bottom: packed.right() as f32 / 1024.0,
            };

            let character = Character {
                metrics,
                tex_coords,
                character: char,
                rect: packed,
            };

            self.characters.insert(char, character);
        }

        self.characters.get(&char)
    }

    fn font_texture(
        wgpu_context: &WgpuContext,
        texture_bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
    ) -> Texture {
        let id = TEXTURE_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let texture_size = wgpu::Extent3d {
            width: CACHE_WIDTH,
            height: CACHE_WIDTH,
            depth_or_array_layers: 1,
        };

        let wgpu_texture = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                label: Some("texture"),
            });

        let texture_view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let texture_bind_group: BindGroup =
            wgpu_context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                    label: Some("diffuse_bind_group"),
                });

        let texture = Texture {
            id: TextureId(id),
            texture: wgpu_texture,
            texture_bind_group,
        };
        texture
    }
}

pub(crate) struct TextContext {
    texture_bind_group_layout: BindGroupLayout,
    sampler: Sampler,
}

impl TextContext {
    pub fn new(context: &WgpuContext) -> Self {
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

        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        TextContext {
            texture_bind_group_layout,
            sampler,
        }
    }
}

struct TextRenderer {
    render_pipeline: RenderPipeline,
}

impl TextRenderer {
    pub(crate) fn new(context: &WgpuContext, text_context: &TextContext, camera: &Camera) -> Self {
        let shader = context
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/font.wgsl").into()),
            });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Texture Render Pipeline Layout"),
                    bind_group_layouts: &[
                        &camera.camera_bind_group_layout,
                        &text_context.texture_bind_group_layout,
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
                        entry_point: "vs_main",                   // 1.
                        buffers: &[TextureVertex::description()], // 2.
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
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,                         // 2.
                        mask: !0,                         // 3.
                        alpha_to_coverage_enabled: false, // 4.
                    },
                });

        TextRenderer { render_pipeline }
    }

    pub(crate) fn render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        context: &'a WgpuContext,
        camera: &'a Camera,
        operation_block: &'a mut OperationBlock,
    ) {
    }
}

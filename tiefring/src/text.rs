use std::{cell::RefCell, collections::HashMap, fmt::Debug, fs, path::Path, rc::Rc};

use fontdue::{LineMetrics, Metrics};
use rect_packer::Packer;
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Buffer, RenderPass, RenderPipeline, Sampler,
    SamplerBindingType,
};

use crate::{
    camera::Camera,
    sprite::Texture,
    sprite::{TextureId, TEXTURE_INDEX},
    Color, Position, Rect, WgpuContext,
};

pub(crate) struct DrawTextOperation {
    pub position: Position,
    pub font_for_px: Rc<RefCell<SizedFont>>,
    pub text: String,
    pub color: Color,
}

pub(crate) struct DrawTextOperations {
    pub operations: Vec<DrawTextOperation>,
    buffers: Option<(Rc<Texture>, Buffer, Buffer)>,
}

impl DrawTextOperations {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            buffers: None,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub(crate) struct FontId(pub(crate) usize, pub(crate) u32);

pub struct Font {
    pub(crate) font: Rc<fontdue::Font>,
    font_cache: HashMap<u32, Rc<RefCell<SizedFont>>>,
}

static CACHE_WIDTH: u32 = 1024;

impl Font {
    pub fn load_font<P: AsRef<Path>>(path: P) -> Option<Self> {
        let bytes = fs::read(path).ok()?;

        let font =
            Rc::new(fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?);
        let font_cache = HashMap::new();

        Some(Self { font, font_cache })
    }

    pub fn measure(&self, character: char, px: u32) -> (f32, f32) {
        let metrics = self.font.metrics(character, px as f32);
        (metrics.bounds.width, metrics.bounds.height)
    }

    pub fn ascent(&self, px: u32) -> f32 {
        let line_metrics = self.font.horizontal_line_metrics(px as f32).unwrap();
        line_metrics.ascent
    }

    pub(crate) fn get_font_for_px(&mut self, px: u32) -> Rc<RefCell<SizedFont>> {
        self.font_cache
            .entry(px)
            .or_insert_with(|| Rc::new(RefCell::new(SizedFont::new(px, self.font.clone()))))
            .clone()
    }
}

enum CharType {
    Regular,
    WhiteSpace,
    LineBreak,
}

impl CharType {
    fn new(char: char) -> Self {
        match char {
            ' ' => CharType::WhiteSpace,
            '\n' => CharType::LineBreak,
            _ => CharType::Regular,
        }
    }
}

struct CharacterReference {
    metrics: Metrics,
    tex_coords: Rect,
    rect: Option<rect_packer::Rect>,
    char_type: CharType,
}

pub(crate) struct SizedFont {
    px: u32,
    texture: Option<Rc<Texture>>,
    packer: Packer,
    font: Rc<fontdue::Font>,
    characters: HashMap<char, CharacterReference>,
    horizontal_line_metrics: LineMetrics,
}

impl SizedFont {
    fn new(px: u32, font: Rc<fontdue::Font>) -> Self {
        let texture = None;
        let packer = Packer::new(rect_packer::Config {
            width: CACHE_WIDTH as i32,
            height: CACHE_WIDTH as i32,
            border_padding: 0,
            rectangle_padding: 0,
        });
        let characters = HashMap::new();
        let horizontal_line_metrics = font
            .horizontal_line_metrics(px as f32)
            .expect("We only handle horizontal fonts for now");

        Self {
            px,
            texture,
            packer,
            font,
            characters,
            horizontal_line_metrics,
        }
    }

    fn get_or_create_texture(
        &mut self,
        wgpu_context: &WgpuContext,
        text_context: &TextContext,
    ) -> Rc<Texture> {
        self.texture
            .get_or_insert_with(|| {
                Rc::new(SizedFont::font_texture(
                    wgpu_context,
                    &text_context.texture_bind_group_layout,
                    &text_context.sampler,
                ))
            })
            .clone()
    }

    fn get_or_create_character(
        &mut self,
        char: char,
        wgpu_context: &WgpuContext,
        text_context: &TextContext,
    ) -> Option<&CharacterReference> {
        if self.contains(&char) {
            self.characters.get(&char)
        } else {
            self.create_character(char, wgpu_context, text_context)
        }
    }

    fn contains(&self, character: &char) -> bool {
        self.characters.contains_key(character)
    }

    fn create_character(
        &mut self,
        char: char,
        wgpu_context: &WgpuContext,
        text_context: &TextContext,
    ) -> Option<&CharacterReference> {
        let (metrics, bitmap) = self.font.rasterize(char, self.px as f32);

        if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
            // A character without dimension, probably white space.

            let char_type = CharType::new(char);
            let character = CharacterReference {
                metrics,
                tex_coords: Rect {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                },
                rect: None,
                char_type,
            };

            self.characters.insert(char, character);
            return self.characters.get(&char);
        }

        let packed = self
            .packer
            .pack(metrics.width as i32, metrics.height as i32, false);

        if let Some(packed) = packed {
            let texture = self.texture.get_or_insert_with(|| {
                Rc::new(SizedFont::font_texture(
                    wgpu_context,
                    &text_context.texture_bind_group_layout,
                    &text_context.sampler,
                ))
            });

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
                bottom: packed.bottom() as f32 / 1024.0,
            };
            let char_type = CharType::new(char);

            let character = CharacterReference {
                metrics,
                tex_coords,
                rect: Some(packed),
                char_type,
            };

            self.characters.insert(char, character);
            self.characters.get(&char)
        } else {
            None
        }
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
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                    ],
                    label: Some("diffuse_bind_group"),
                });

        Texture {
            id: TextureId(id),
            texture: wgpu_texture,
            texture_bind_group,
        }
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
                            ty: wgpu::BindingType::Sampler(SamplerBindingType::Filtering),
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
    color: [f32; 4],
}

impl TextVertex {
    fn description<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub(crate) struct TextRenderer {
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
                        entry_point: "vs_main",
                        buffers: &[TextVertex::description()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[wgpu::ColorTargetState {
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

        TextRenderer { render_pipeline }
    }

    pub(crate) fn render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        wgpu_context: &'a WgpuContext,
        text_context: &'a TextContext,
        camera: &'a Camera,
        draw_text_operations: &'a mut DrawTextOperations,
    ) {
        let char_count: usize = draw_text_operations
            .operations
            .iter()
            .map(|operation| operation.text.len())
            .sum();

        if char_count == 0 {
            return;
        }

        let mut vertices: Vec<TextVertex> = Vec::with_capacity(char_count * 4);

        let texture = draw_text_operations
            .operations
            .first()
            .expect("We have at last one operation, or char_count would be 0")
            .font_for_px
            .borrow_mut()
            .get_or_create_texture(wgpu_context, text_context);

        for operation in draw_text_operations.operations.iter() {
            let Position {
                x: mut left,
                y: mut top,
            } = operation.position;
            let mut font_for_px = operation.font_for_px.borrow_mut();
            let ascent = font_for_px.horizontal_line_metrics.ascent;
            let new_line_size = font_for_px.horizontal_line_metrics.new_line_size;
            for char in operation.text.chars() {
                let character =
                    match font_for_px.get_or_create_character(char, wgpu_context, text_context) {
                        Some(character) => character,
                        None => continue,
                    };

                match character.char_type {
                    CharType::WhiteSpace => {
                        left += character.metrics.advance_width.round();
                    }
                    CharType::LineBreak => {
                        left = operation.position.x;
                        top += new_line_size;
                    }
                    CharType::Regular => {
                        let rect = if let Some(rect) = character.rect {
                            rect
                        } else {
                            continue;
                        };
                        let char_left = left + character.metrics.xmin as f32;

                        let char_top = top + ascent
                            - character.metrics.height as f32
                            - character.metrics.ymin as f32;
                        let bottom = char_top + rect.height as f32;
                        let right = char_left + rect.width as f32;
                        let color: [f32; 4] = operation.color.as_float_array();

                        vertices.push(TextVertex {
                            position: [char_left, char_top],
                            tex_coords: [character.tex_coords.left, character.tex_coords.top],
                            color,
                        });
                        vertices.push(TextVertex {
                            position: [char_left, bottom],
                            tex_coords: [character.tex_coords.left, character.tex_coords.bottom],
                            color,
                        });
                        vertices.push(TextVertex {
                            position: [right, bottom],
                            tex_coords: [character.tex_coords.right, character.tex_coords.bottom],
                            color,
                        });
                        vertices.push(TextVertex {
                            position: [right, char_top],
                            tex_coords: [character.tex_coords.right, character.tex_coords.top],
                            color,
                        });

                        left += character.metrics.advance_width as f32;
                    }
                }
            }
        }

        let indices: Vec<u16> = (0..vertices.len() / 4)
            .flat_map(|index| {
                let step: u16 = index as u16 * 4;
                [step, step + 1, step + 2, step + 2, step + 3, step]
            })
            .collect();

        let vertex_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices[..]),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        let index_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(&indices[..]),
                    usage: wgpu::BufferUsages::INDEX,
                });

        let (texture, vertex_buffer, index_buffer) =
            draw_text_operations
                .buffers
                .insert((texture, vertex_buffer, index_buffer));

        let indice_count = indices.len() as u32;
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &camera.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &texture.texture_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..indice_count, 0, 0..1);
    }
}

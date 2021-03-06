use std::{cell::RefCell, collections::HashMap, fmt::Debug, fs, path::Path, rc::Rc};

use fontdue::layout::{CoordinateSystem, Layout, TextStyle};
use rect_packer::Packer;
use wgpu::{BindGroup, BindGroupLayout, Sampler};

use crate::{
    renderer::{ColorMatrix, RenderOperation},
    sprite::Texture,
    sprite::{TextureContext, TextureId, TEXTURE_INDEX},
    Color, DeviceAndQueue, Position, Rect,
};

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

struct CharacterReference {
    tex_coords: Rect,
}

pub(crate) struct SizedFont {
    px: u32,
    texture: Option<Rc<Texture>>,
    packer: Packer,
    font: Rc<fontdue::Font>,
    characters: HashMap<char, CharacterReference>,
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

        Self {
            px,
            texture,
            packer,
            font,
            characters,
        }
    }

    pub(crate) fn get_or_create_texture(
        &mut self,
        device_and_queue: &DeviceAndQueue,
        texture_context: &TextureContext,
    ) -> Rc<Texture> {
        self.texture
            .get_or_insert_with(|| {
                Rc::new(SizedFont::font_texture(
                    device_and_queue,
                    &texture_context.texture_bind_group_layout,
                    &texture_context.sampler,
                ))
            })
            .clone()
    }

    fn get_or_create_character(
        &mut self,
        char: char,
        device_and_queue: &DeviceAndQueue,
        texture_context: &TextureContext,
    ) -> Option<&CharacterReference> {
        if self.contains(&char) {
            self.characters.get(&char)
        } else {
            self.create_character(char, device_and_queue, texture_context)
        }
    }

    fn contains(&self, character: &char) -> bool {
        self.characters.contains_key(character)
    }

    fn create_character(
        &mut self,
        char: char,
        device_and_queue: &DeviceAndQueue,
        texture_context: &TextureContext,
    ) -> Option<&CharacterReference> {
        let (metrics, bitmap) = self.font.rasterize(char, self.px as f32);

        if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
            // A character without dimension, probably white space.
            let character = CharacterReference {
                tex_coords: Rect {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                },
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
                    device_and_queue,
                    &texture_context.texture_bind_group_layout,
                    &texture_context.sampler,
                ))
            });

            device_and_queue.queue.write_texture(
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

            let character = CharacterReference { tex_coords };

            self.characters.insert(char, character);
            self.characters.get(&char)
        } else {
            None
        }
    }

    fn font_texture(
        device_and_queue: &DeviceAndQueue,
        texture_bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
    ) -> Texture {
        let id = TEXTURE_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let texture_size = wgpu::Extent3d {
            width: CACHE_WIDTH,
            height: CACHE_WIDTH,
            depth_or_array_layers: 1,
        };

        let wgpu_texture = device_and_queue
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
            device_and_queue
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

pub(crate) struct TextConverter {
    layout: Layout,
}

impl TextConverter {
    pub fn new() -> Self {
        Self {
            layout: Layout::new(CoordinateSystem::PositiveYDown),
        }
    }
}

impl TextConverter {
    pub fn render_operation(
        &mut self,
        text: &str,
        color: Color,
        position: Position,
        font_for_px: &Rc<RefCell<SizedFont>>,
        device_and_queue: &DeviceAndQueue,
        texture_context: &TextureContext,
    ) -> Vec<RenderOperation> {
        let char_count: usize = text.len();

        if char_count == 0 {
            return vec![];
        }

        let size = font_for_px.borrow().px;
        let fonts = &[font_for_px.borrow().font.clone()];

        let Position { x, y } = position;
        self.layout.reset(&fontdue::layout::LayoutSettings {
            x,
            y,
            ..Default::default()
        });

        let color_matrix = ColorMatrix::for_text(color);

        self.layout
            .append(fonts, &TextStyle::new(text, size as f32, 0));
        let mut font_for_px = font_for_px.borrow_mut();

        let operations = self
            .layout
            .glyphs()
            .iter()
            .filter_map(|glyph| {
                let position =
                    Rect::from_xywh(glyph.x, glyph.y, glyph.width as f32, glyph.height as f32);

                font_for_px
                    .get_or_create_character(glyph.parent, device_and_queue, texture_context)
                    .map(|character| RenderOperation {
                        tex_coords: character.tex_coords,
                        position: position.into(),
                        color_matrix,
                    })
            })
            .collect();

        operations
    }
}

use std::{ops::Index, path::Path, rc::Rc, sync::atomic::AtomicUsize};

use wgpu::{BindGroup, BindGroupLayout, Device, Queue, Sampler, SamplerBindingType};

use crate::{Error, Rect, SizeInPx};

#[derive(Clone)]
pub struct Sprite {
    pub dimensions: SizeInPx,
    pub(crate) tex_coords: Rect,
    pub(crate) texture: Rc<Texture>,
}

impl Sprite {
    pub(crate) fn load_image<P: AsRef<Path>>(
        device: &Device,
        queue: &Queue,
        texture_bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
        path: P,
    ) -> Result<Self, Error> {
        let image = image::open(&path).map_err(|_e| Error::LoadingFailed(path.as_ref().into()))?;

        let rgba = image.to_rgba8();

        use image::GenericImageView;
        let dimensions = image.dimensions();

        Ok(Sprite::load_data(
            device,
            queue,
            texture_bind_group_layout,
            sampler,
            &rgba,
            dimensions,
        ))
    }

    pub(crate) fn load_data<S>(
        device: &Device,
        queue: &Queue,
        texture_bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
        rgba: &[u8],
        dimensions: S,
    ) -> Self
    where
        S: Into<SizeInPx> + Copy,
    {
        let texture = Rc::new(Texture::new(
            device,
            queue,
            texture_bind_group_layout,
            sampler,
            rgba,
            dimensions.into(),
        ));
        let tex_coord = Rect {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        };

        Sprite {
            dimensions: dimensions.into(),
            tex_coords: tex_coord,
            texture,
        }
    }
}

pub struct TileSet {
    pub(crate) dimensions: SizeInPx,
    pub(crate) tile_dimensions: SizeInPx,
    sprites: Vec<Sprite>,
}

impl TileSet {
    pub fn load_image<P, S>(
        device: &Device,
        queue: &Queue,
        texture_bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
        path: P,
        tile_dimensions: S,
    ) -> Result<Self, Error>
    where
        P: AsRef<Path>,
        S: Into<SizeInPx> + Copy,
    {
        let image = image::open(&path).map_err(|_e| Error::LoadingFailed(path.as_ref().into()))?;

        let rgba = image.to_rgba8();

        use image::GenericImageView;
        let dimensions = image.dimensions();

        Ok(TileSet::load_data::<(u32, u32), S>(
            device,
            queue,
            texture_bind_group_layout,
            sampler,
            &rgba,
            dimensions,
            tile_dimensions,
        ))
    }

    pub fn tile_count(&self) -> (u32, u32) {
        (
            self.dimensions.width / self.tile_dimensions.width,
            self.dimensions.height / self.tile_dimensions.height,
        )
    }

    pub fn sprite(&self, x: u32, y: u32) -> Option<&Sprite> {
        let (width, height) = self.tile_count();
        if x > width || y > height {
            return None;
        }

        let index = (y * width + x) as usize;
        self.sprites.get(index)
    }

    pub fn sprite_with_index(&self, index: usize) -> Option<&Sprite> {
        self.sprites.get(index)
    }

    fn load_data<S, TS>(
        device: &Device,
        queue: &Queue,
        texture_bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
        rgba: &[u8],
        dimensions: S,
        tile_dimensions: TS,
    ) -> Self
    where
        S: Into<SizeInPx> + Copy,
        TS: Into<SizeInPx> + Copy,
    {
        let texture = Rc::new(Texture::new(
            device,
            queue,
            texture_bind_group_layout,
            sampler,
            rgba,
            dimensions.into(),
        ));
        let dimensions = dimensions.into();
        let tile_dimensions = tile_dimensions.into();

        let x_count = dimensions.width / tile_dimensions.width;
        let y_count = dimensions.height / tile_dimensions.height;

        let mut sprites = Vec::with_capacity((x_count * y_count) as usize);
        for y in 0..y_count {
            for x in 0..x_count {
                let tex_coords = Rect {
                    left: (x * tile_dimensions.width) as f32 / dimensions.width as f32,
                    top: (y * tile_dimensions.height) as f32 / dimensions.height as f32,
                    width: tile_dimensions.width as f32 / dimensions.width as f32,
                    height: tile_dimensions.height as f32 / dimensions.height as f32,
                };

                let sprite = Sprite {
                    dimensions: tile_dimensions,
                    tex_coords,
                    texture: texture.clone(),
                };
                sprites.push(sprite);
            }
        }

        TileSet {
            dimensions,
            tile_dimensions,
            sprites,
        }
    }
}

impl Index<usize> for TileSet {
    type Output = Sprite;

    fn index(&self, index: usize) -> &Self::Output {
        &self.sprites[index]
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub(crate) struct TextureId(pub(crate) usize);

#[derive(Debug)]
pub(crate) struct Texture {
    pub id: TextureId,
    pub texture: wgpu::Texture,
    pub texture_bind_group: BindGroup,
}

pub(crate) static TEXTURE_INDEX: AtomicUsize = AtomicUsize::new(0);

impl Texture {
    pub fn new(
        device: &Device,
        queue: &Queue,
        texture_bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
        rgba: &[u8],
        dimensions: SizeInPx,
    ) -> Self {
        let id = TEXTURE_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let texture_size = wgpu::Extent3d {
            width: dimensions.width,
            height: dimensions.height,
            depth_or_array_layers: 1,
        };
        let wgpu_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("texture"),
        });

        queue.write_texture(
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

        let texture_bind_group: BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
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

pub(crate) struct TextureContext {
    pub texture_bind_group_layout: BindGroupLayout,
    pub sampler: Sampler,
    pub white_texture: Rc<Texture>,
}

impl TextureContext {
    pub fn new(device: &Device, queue: &Queue) -> Self {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let white_texture = Rc::new(Texture::new(
            device,
            queue,
            &texture_bind_group_layout,
            &sampler,
            &[255, 255, 255, 255],
            SizeInPx::new(1, 1),
        ));

        Self {
            texture_bind_group_layout,
            sampler,
            white_texture,
        }
    }
}

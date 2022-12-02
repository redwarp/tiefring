use std::path::Path;

use wgpu::{Device, Queue};

use crate::{
    sprite::{Sprite, TextureContext, TileSet},
    text::Font,
    Error, SizeInPx,
};

pub struct Resources<'a> {
    device: &'a Device,
    queue: &'a Queue,
    texture_context: &'a TextureContext,
}

impl<'a> Resources<'a> {
    pub(crate) fn new(
        device: &'a Device,
        queue: &'a Queue,
        texture_context: &'a TextureContext,
    ) -> Self {
        Self {
            device,
            queue,
            texture_context,
        }
    }

    pub fn load_sprite<P: AsRef<Path>>(&self, path: P) -> Result<Sprite, Error> {
        Sprite::load_image(
            self.device,
            self.queue,
            &self.texture_context.texture_bind_group_layout,
            &self.texture_context.sampler,
            path,
        )
    }

    pub fn load_tileset<P, S>(&self, path: P, tile_dimensions: S) -> Result<TileSet, Error>
    where
        P: AsRef<Path>,
        S: Into<SizeInPx> + Copy,
    {
        TileSet::load_image(
            self.device,
            self.queue,
            &self.texture_context.texture_bind_group_layout,
            &self.texture_context.sampler,
            path,
            tile_dimensions,
        )
    }

    pub fn load_font<P: AsRef<Path>>(&self, path: P) -> Result<Font, Error> {
        Font::load_font(path)
    }

    #[cfg(feature = "svg")]
    pub fn load_svg<P: AsRef<Path>>(&self, path: P) -> Result<Sprite, Error> {
        let resources_dir = std::fs::canonicalize(&path)
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let opt = resvg::usvg::Options {
            resources_dir,
            ..Default::default()
        };

        let svg_data = std::fs::read(&path)?;

        let rtree = resvg::usvg::Tree::from_data(&svg_data, &opt.to_ref())
            .map_err(|_e| Error::LoadingFailed(path.as_ref().to_path_buf()))?;
        let pixmap_size = rtree.size.to_screen_size();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())
            .ok_or_else(|| Error::LoadingFailed(path.as_ref().to_path_buf()))?;

        resvg::render(
            &rtree,
            resvg::usvg::FitTo::Original,
            resvg::tiny_skia::Transform::default(),
            pixmap.as_mut(),
        )
        .ok_or_else(|| Error::LoadingFailed(path.as_ref().to_path_buf()))?;

        Ok(Sprite::load_data(
            self.device,
            self.queue,
            &self.texture_context.texture_bind_group_layout,
            &self.texture_context.sampler,
            pixmap.data(),
            pixmap_size.dimensions(),
        ))
    }
}

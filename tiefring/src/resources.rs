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
}

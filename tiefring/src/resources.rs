use std::path::Path;

use wgpu::{Device, Queue};

use crate::{
    sprite::{Sprite, TextureContext},
    Error,
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
}

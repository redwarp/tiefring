use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use cache::TransformCache;
use futures::AsyncBufferView;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use renderer::Transform;
use resources::Resources;
use thiserror::Error;
use wgpu::{BufferAsyncError, CommandEncoder, Device, Queue, RenderPass};

use crate::{
    cache::{BufferCache, ReusableBuffer},
    camera::{Camera, CameraSettings},
    renderer::{ColorMatrix, RenderOperation, RenderPreper, Renderer},
    sprite::{Sprite, Texture, TextureContext},
    text::{Font, TextConverter},
};

mod cache;
mod camera;
mod futures;
mod renderer;
pub mod resources;
pub mod sprite;
pub mod text;

const DEFAULT_COLOR_MATRIX: ColorMatrix = ColorMatrix::from_color(Color::rgb(1.0, 1.0, 1.0));
const OPERATION_CAPACITY: usize = 2048;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't initialize wgpu")]
    InitializationFailed,

    #[error("Rendering failed")]
    RenderingFailed(wgpu::SurfaceError),

    #[error("Loading failed")]
    LoadingFailed(PathBuf),

    #[error("Loading failed")]
    IOError(std::io::Error),

    #[error("Couldn't take screenshot")]
    ScreenshotFailed,
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::IOError(error)
    }
}

pub struct GraphicsRenderer {
    draw_datas: Vec<DrawData>,
    renderer: Renderer,
    buffer_cache: BufferCache,
    transform_cache: TransformCache,
    camera: Camera,
    size: SizeInPx,
    texture_context: TextureContext,
    text_converter: TextConverter,
    render_preper: RenderPreper,
}

impl GraphicsRenderer {
    pub fn new(device: &Device, queue: &Queue, width: u32, height: u32, scale: f32) -> Self {
        let draw_datas = vec![];
        let camera = Camera::new(
            device,
            CameraSettings {
                scale,
                translation: Position::new(0.0, 0.0),
                width,
                height,
            },
        );

        let texture_context = TextureContext::new(device, queue);

        let renderer = Renderer::new(device, &texture_context, &camera);
        let buffer_cache = BufferCache::new();
        let transform_cache = TransformCache::new();
        let size = SizeInPx { width, height };

        let text_converter = TextConverter::new();
        let render_preper = RenderPreper::new();

        Self {
            draw_datas,
            renderer,
            buffer_cache,
            transform_cache,
            camera,
            size,
            texture_context,
            text_converter,
            render_preper,
        }
    }

    pub fn prepare<F>(&mut self, device: &Device, queue: &Queue, prepare_function: F)
    where
        F: FnOnce(&mut Graphics),
    {
        self.reset();
        if self.camera.dirty {
            self.camera.recalculate(queue);
        }

        let mut graphics = Graphics::new(
            self.size,
            device,
            queue,
            &self.texture_context,
            &mut self.draw_datas,
            &mut self.buffer_cache,
            &mut self.transform_cache,
            &mut self.text_converter,
            &mut self.render_preper,
        );

        prepare_function(&mut graphics);
        graphics.prepare_current_block();

        self.cleanup();
    }

    pub fn render<'rpass>(&'rpass mut self, render_pass: &mut RenderPass<'rpass>) {
        render_pass.set_bind_group(0, &self.camera.camera_bind_group, &[]);
        self.renderer.render(render_pass, &self.draw_datas);
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.size = SizeInPx { width, height };
        self.camera.set_size(width, height)
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.camera.set_scale(scale);
    }

    pub fn set_translation(&mut self, translation: Position) {
        self.camera.set_translation(translation);
    }

    pub fn resources<'a>(&'a self, device: &'a Device, queue: &'a Queue) -> Resources<'a> {
        Resources::new(device, queue, &self.texture_context)
    }

    fn reset(&mut self) {
        for draw_data in self.draw_datas.drain(..) {
            self.buffer_cache.release_buffer(draw_data.instance_buffer);
        }
    }

    fn cleanup(&mut self) {
        // We cleanup buffers that were not reused previously.
        self.buffer_cache.clear();
        self.transform_cache.free_unused();
    }
}

pub struct Canvas {
    wgpu_context: WgpuContext,
    graphics_renderer: GraphicsRenderer,
    canvas_settings: CanvasSettings,
}

impl Canvas {
    pub async fn new<W>(
        window: &W,
        width: u32,
        height: u32,
        canvas_settings: CanvasSettings,
    ) -> Result<Canvas, Error>
    where
        W: HasRawWindowHandle + HasRawDisplayHandle,
    {
        let wgpu_context = WgpuContext::new(window, width, height).await?;
        let graphics_renderer = GraphicsRenderer::new(
            &wgpu_context.device,
            &wgpu_context.queue,
            width,
            height,
            canvas_settings.scale,
        );

        Ok(Self {
            wgpu_context,
            graphics_renderer,
            canvas_settings,
        })
    }

    pub fn draw<F>(&mut self, draw_function: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Graphics),
    {
        self.graphics_renderer.prepare(
            &self.wgpu_context.device,
            &self.wgpu_context.queue,
            draw_function,
        );

        let surface_texture = self
            .wgpu_context
            .surface
            .get_current_texture()
            .map_err(Error::RenderingFailed)?;
        let view = self
            .wgpu_context
            .buffer_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder: CommandEncoder =
            self.wgpu_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.canvas_settings.background_color.into()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.graphics_renderer.render(&mut render_pass);
        }

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.wgpu_context.buffer_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &surface_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.wgpu_context.size.width,
                height: self.wgpu_context.size.height,
                depth_or_array_layers: 1,
            },
        );

        self.wgpu_context.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.wgpu_context.resize(width, height);
        self.graphics_renderer.set_size(width, height);
    }

    pub fn size(&self) -> SizeInPx {
        self.graphics_renderer.size
    }

    pub fn scale(&self) -> f32 {
        self.canvas_settings.scale
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.canvas_settings.scale = scale;
        self.graphics_renderer.set_scale(scale);
    }

    pub fn translation(&self) -> Position {
        self.graphics_renderer.camera.camera_settings.translation
    }

    pub fn set_translation(&mut self, translation: Position) {
        self.graphics_renderer.set_translation(translation)
    }

    pub async fn screenshot<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let SizeInPx { width, height } = self.wgpu_context.size;
        let pixels = texture_to_cpu(
            &self.wgpu_context.device,
            &self.wgpu_context.queue,
            width,
            height,
            &self.wgpu_context.buffer_texture,
        )
        .await
        .map_err(|_| Error::ScreenshotFailed)?;

        use image::{ImageBuffer, Rgba};
        let mut buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, pixels).unwrap();

        for px in buffer.pixels_mut() {
            let cmp = px.0;
            *px = Rgba([cmp[2], cmp[1], cmp[0], cmp[3]]);
        }

        buffer.save(path).map_err(|_| Error::ScreenshotFailed)?;

        Ok(())
    }

    pub fn resources(&self) -> Resources {
        Resources::new(
            &self.wgpu_context.device,
            &self.wgpu_context.queue,
            &self.graphics_renderer.texture_context,
        )
    }
}

pub struct CanvasSettings {
    pub scale: f32,
    pub background_color: Color,
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            background_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }
}

struct OperationBlock {
    operations: Vec<RenderOperation>,
    texture: Rc<Texture>,
}

impl OperationBlock {
    fn with_texture(texture: Rc<Texture>) -> Self {
        OperationBlock {
            operations: Vec::with_capacity(OPERATION_CAPACITY),
            texture,
        }
    }

    fn push_render_operation(&mut self, render_operation: RenderOperation) -> &mut RenderOperation {
        self.operations.push(render_operation);
        self.operations.last_mut().expect("Just pushed an item")
    }
}

struct DrawData {
    instance_buffer: ReusableBuffer,
    count: u32,
    texture: Rc<Texture>,
}

pub struct Graphics<'a> {
    device: &'a Device,
    queue: &'a Queue,
    size: SizeInPx,
    transforms: Vec<Transform>,
    current_operation_block: Option<OperationBlock>,
    draw_datas: &'a mut Vec<DrawData>,
    render_preper: &'a mut RenderPreper,
    buffer_cache: &'a mut BufferCache,
    transform_cache: &'a mut TransformCache,
    texture_context: &'a TextureContext,
    text_converter: &'a mut TextConverter,
}

impl<'a> Graphics<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        size: SizeInPx,
        device: &'a Device,
        queue: &'a Queue,
        texture_context: &'a TextureContext,
        draw_datas: &'a mut Vec<DrawData>,
        buffer_cache: &'a mut BufferCache,
        transform_cache: &'a mut TransformCache,
        text_converter: &'a mut TextConverter,
        render_preper: &'a mut RenderPreper,
    ) -> Self {
        Graphics {
            current_operation_block: None,
            draw_datas,
            size,
            transforms: vec![],
            texture_context,
            device,
            queue,
            text_converter,
            render_preper,
            buffer_cache,
            transform_cache,
        }
    }

    pub fn draw_rect<R: Into<Rect>>(&mut self, rect: R, color: Color) -> &mut RenderOperation {
        let tex_coords = Rect::new(0.0, 0.0, 1.0, 1.0);

        let rect: Rect = rect.into();
        let mut transforms = self.transform_cache.get();
        transforms.extend(&self.transforms);
        let color_matrix = ColorMatrix::from_color(color);

        let operation = RenderOperation {
            rect,
            color_matrix,
            tex_coords,
            transforms,
        };

        self.get_operation_block(&self.texture_context.white_texture)
            .push_render_operation(operation)
    }

    pub fn draw_sprite<P: Into<Position>>(
        &mut self,
        sprite: &Sprite,
        position: P,
    ) -> &mut RenderOperation {
        self.draw_sprite_in_rect(sprite, (position.into(), sprite.dimensions))
    }

    pub fn draw_sprite_in_rect<R: Into<Rect>>(
        &mut self,
        sprite: &Sprite,
        rect: R,
    ) -> &mut RenderOperation {
        let tex_coords = sprite.tex_coords;

        let rect: Rect = rect.into();
        let mut transforms = self.transform_cache.get();
        transforms.extend(&self.transforms);
        let color_matrix = DEFAULT_COLOR_MATRIX;
        let operation = RenderOperation {
            rect,
            color_matrix,
            tex_coords,
            transforms,
        };
        self.get_operation_block(&sprite.texture)
            .push_render_operation(operation)
    }

    pub fn draw_text<T, P>(&mut self, font: &mut Font, text: T, px: u32, position: P, color: Color)
    where
        T: AsRef<str>,
        P: Into<Position>,
    {
        let position = position.into();

        let mut transforms = self.transform_cache.get();
        transforms.extend(&self.transforms);
        let font_for_px = font.get_font_for_px(px);
        let mut operations = self.text_converter.render_operation(
            text.as_ref(),
            color,
            position,
            &font_for_px,
            transforms,
            self.device,
            self.queue,
            self.texture_context,
            self.transform_cache,
        );

        let texture = font_for_px
            .borrow_mut()
            .get_or_create_texture(self.device, self.texture_context);
        self.get_operation_block(&texture)
            .operations
            .append(&mut operations);
    }

    pub fn with_translation<F>(&mut self, translation: Position, function: F)
    where
        F: FnOnce(&mut Self),
    {
        self.transforms.push(Transform::Translate {
            x: translation.left,
            y: translation.top,
        });
        function(self);
        self.transforms.pop();
    }

    pub fn size(&self) -> SizeInPx {
        self.size
    }

    fn get_operation_block(&mut self, texture: &Rc<Texture>) -> &mut OperationBlock {
        let need_new = !matches!(&self.current_operation_block, Some(operation_block) if operation_block.texture.id == texture.id && operation_block.operations.len() < OPERATION_CAPACITY);
        if need_new {
            self.prepare_current_block();

            self.current_operation_block
                .insert(OperationBlock::with_texture(texture.clone()))
        } else {
            self.current_operation_block.as_mut().unwrap()
        }
    }

    fn prepare_current_block(&mut self) {
        if let Some(draw_data) = self
            .current_operation_block
            .take()
            .and_then(|operation_block| {
                self.render_preper.prepare(
                    self.buffer_cache,
                    self.device,
                    self.queue,
                    operation_block,
                    self.transform_cache,
                )
            })
        {
            self.draw_datas.push(draw_data);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            left: x,
            top: y,
            width,
            height,
        }
    }

    pub const fn square(x: f32, y: f32, width: f32) -> Self {
        Self {
            left: x,
            top: y,
            width,
            height: width,
        }
    }

    pub fn translated(&self, x: f32, y: f32) -> Self {
        Self {
            left: self.left + x,
            top: self.top + y,
            width: self.width,
            height: self.height,
        }
    }
}

impl From<[i32; 4]> for Rect {
    fn from(coordinates: [i32; 4]) -> Self {
        Rect {
            left: coordinates[0] as f32,
            top: coordinates[1] as f32,
            width: coordinates[2] as f32,
            height: coordinates[3] as f32,
        }
    }
}

impl From<[f32; 4]> for Rect {
    fn from(coordinates: [f32; 4]) -> Self {
        Rect {
            left: coordinates[0],
            top: coordinates[1],
            width: coordinates[2],
            height: coordinates[3],
        }
    }
}

impl From<(Position, SizeInPx)> for Rect {
    fn from((position, size): (Position, SizeInPx)) -> Self {
        Rect::new(
            position.left,
            position.top,
            size.width as f32,
            size.height as f32,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub left: f32,
    pub top: f32,
}

impl Position {
    pub fn new(left: f32, top: f32) -> Self {
        Self { left, top }
    }

    pub fn translated(&self, x: f32, y: f32) -> Self {
        Self {
            left: self.left + x,
            top: self.top + y,
        }
    }
}

impl From<(f32, f32)> for Position {
    fn from((left, top): (f32, f32)) -> Self {
        Self { left, top }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SizeInPx {
    pub width: u32,
    pub height: u32,
}

impl SizeInPx {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl From<(u32, u32)> for SizeInPx {
    fn from(size: (u32, u32)) -> Self {
        Self {
            width: size.0,
            height: size.1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<Color> for wgpu::Color {
    fn from(color: Color) -> Self {
        wgpu::Color {
            r: color.r as f64,
            g: color.g as f64,
            b: color.b as f64,
            a: color.a as f64,
        }
    }
}

impl Color {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug)]
pub(crate) struct WgpuContext {
    surface: wgpu::Surface,
    device: Device,
    queue: Queue,
    config: wgpu::SurfaceConfiguration,
    size: SizeInPx,
    buffer_texture: wgpu::Texture,
}

impl WgpuContext {
    async fn new<W>(window: &W, width: u32, height: u32) -> Result<WgpuContext, Error>
    where
        W: HasRawWindowHandle + HasRawDisplayHandle,
    {
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(Error::InitializationFailed)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .map_err(|_| Error::InitializationFailed)?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        surface.configure(&device, &config);

        let size = SizeInPx { width, height };

        let buffer_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: None,
        });

        Ok(WgpuContext {
            surface,
            config,
            device,
            queue,
            size,
            buffer_texture,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = SizeInPx { width, height };
        self.config.width = width;
        self.config.height = height;

        self.surface.configure(&self.device, &self.config);
        self.buffer_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: self.size.width,
                height: self.size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: None,
        });
    }
}

async fn texture_to_cpu(
    device: &Device,
    queue: &Queue,
    width: u32,
    height: u32,
    texture: &wgpu::Texture,
) -> Result<Vec<u8>, BufferAsyncError> {
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let padded_bytes_per_row = padded_bytes_per_row(width);
    let unpadded_bytes_per_row = width as usize * 4;

    let output_buffer_size =
        padded_bytes_per_row as u64 * height as u64 * std::mem::size_of::<u8>() as u64;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            aspect: wgpu::TextureAspect::All,
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(padded_bytes_per_row as u32),
                rows_per_image: std::num::NonZeroU32::new(height),
            },
        },
        texture_size,
    );
    queue.submit(Some(encoder.finish()));

    let padded_data = AsyncBufferView::new(output_buffer.slice(..), device).await?;

    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    for (padded, pixels) in padded_data
        .chunks_exact(padded_bytes_per_row)
        .zip(pixels.chunks_exact_mut((width * 4) as usize))
    {
        pixels.copy_from_slice(bytemuck::cast_slice(&padded[..unpadded_bytes_per_row]));
    }

    Ok(pixels)
}

fn padded_bytes_per_row(width: u32) -> usize {
    let bytes_per_row = width as usize * 4;
    let padding = (256 - bytes_per_row % 256) % 256;
    bytes_per_row + padding
}

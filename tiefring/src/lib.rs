use std::{path::Path, rc::Rc};

use cache::{BufferCache, ReusableBuffer};
use camera::{Camera, CameraSettings};

use glam::{Mat4, Vec3};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use renderer::{ColorMatrix, RenderOperation, RenderPreper, Renderer};
use sprite::{Sprite, Texture, TextureContext};
use text::{Font, TextConverter};
use thiserror::Error;
use wgpu::{CommandEncoder, Device, Queue, RenderPass};

mod cache;
mod camera;
mod renderer;
pub mod sprite;
pub mod text;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't initialize wgpu")]
    InitializationFailed,

    #[error("Rendering failed")]
    RenderingFailed(wgpu::SurfaceError),
}

pub struct TiefringRenderer {
    draw_datas: Vec<DrawData>,
    renderer: Renderer,
    buffer_cache: BufferCache,
    camera: Camera,
    size: SizeInPx,
    white_texture: Rc<Texture>,
    texture_context: TextureContext,
    pub(crate) canvas_settings: CanvasSettings,
}

impl TiefringRenderer {
    fn new(
        device: &Device,
        queue: &Queue,
        width: u32,
        height: u32,
        canvas_settings: CanvasSettings,
    ) -> Self {
        let draw_datas = vec![];
        let camera = Camera::new(
            device,
            CameraSettings {
                scale: canvas_settings.scale,
                translation: Position::new(0.0, 0.0),
                width,
                height,
            },
        );

        let texture_context = Rc::new(TextureContext::new(device));

        let renderer = Renderer::new(device, &texture_context, &camera);
        let buffer_cache = BufferCache::new();
        let size = SizeInPx { width, height };
        let white_texture = Rc::new(Texture::new(
            device,
            queue,
            &texture_context.texture_bind_group_layout,
            &texture_context.sampler,
            &[255, 255, 255, 255],
            SizeInPx::new(1, 1),
        ));

        let texture_context = TextureContext::new(device);

        Self {
            draw_datas,
            renderer,
            buffer_cache,
            camera,
            size,
            white_texture,
            texture_context,
            canvas_settings,
        }
    }

    pub fn prepare<F>(&mut self, device: &Device, queue: &Queue, prepare_function: F)
    where
        F: FnOnce(&mut Graphics),
    {
        self.reset();

        let mut graphics = Graphics::new(
            self.size,
            self.white_texture.clone(),
            device,
            queue,
            &self.texture_context,
        );

        prepare_function(&mut graphics);
        graphics.prepare_current_block();

        self.draw_datas = graphics.draw_datas;
    }

    pub fn render<'rpass>(&'rpass mut self, render_pass: &mut RenderPass<'rpass>) {
        render_pass.set_bind_group(0, &self.camera.camera_bind_group, &[]);
        for DrawData {
            instance_buffer,
            count,
            texture,
        } in self.draw_datas.iter()
        {
            self.renderer
                .render(render_pass, instance_buffer, *count, texture);
        }
    }

    pub fn set_size(&mut self, queue: &Queue, width: u32, height: u32) {
        self.size = SizeInPx { width, height };
        self.camera.set_size(queue, width, height)
    }

    fn reset(&mut self) {
        // We cleanup buffers that were not reused previously.
        self.buffer_cache.clear();
        for draw_data in self.draw_datas.drain(..) {
            self.buffer_cache.release_buffer(draw_data.instance_buffer);
        }
    }
}

pub struct Canvas {
    wgpu_context: WgpuContext,
    tiefring_renderer: TiefringRenderer,
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
        let tiefring_renderer = TiefringRenderer::new(
            &wgpu_context.device_and_queue.device,
            &wgpu_context.device_and_queue.queue,
            width,
            height,
            canvas_settings,
        );

        Ok(Self {
            wgpu_context,
            tiefring_renderer,
        })
    }

    pub fn draw<F>(&mut self, draw_function: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Graphics),
    {
        let mut encoder: CommandEncoder = self
            .wgpu_context
            .device_and_queue
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.tiefring_renderer.prepare(
            &self.wgpu_context.device_and_queue.device,
            &self.wgpu_context.device_and_queue.queue,
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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(
                            self.tiefring_renderer
                                .canvas_settings
                                .background_color
                                .into(),
                        ),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.tiefring_renderer.render(&mut render_pass);
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

        self.wgpu_context
            .device_and_queue
            .queue
            .submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    pub fn redraw_last(&mut self) -> Result<(), Error> {
        let mut encoder: CommandEncoder = self
            .wgpu_context
            .device_and_queue
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Redraw Encoder"),
            });

        let surface_texture = self
            .wgpu_context
            .surface
            .get_current_texture()
            .map_err(Error::RenderingFailed)?;

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

        self.wgpu_context
            .device_and_queue
            .queue
            .submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.tiefring_renderer
            .set_size(&self.wgpu_context.device_and_queue.queue, width, height);
        self.wgpu_context.resize(width, height);
    }

    pub fn size(&self) -> SizeInPx {
        self.wgpu_context.size
    }

    pub fn scale(&self) -> f32 {
        self.tiefring_renderer.canvas_settings.scale
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.tiefring_renderer.canvas_settings.scale = scale;

        self.tiefring_renderer
            .camera
            .set_scale(&self.wgpu_context.device_and_queue.queue, scale);
    }

    pub fn translation(&self) -> Position {
        self.tiefring_renderer.camera.camera_settings.translation
    }

    pub fn set_translation(&mut self, translation: Position) {
        self.tiefring_renderer
            .camera
            .set_translation(&self.wgpu_context.device_and_queue.queue, translation)
    }

    pub async fn screenshot<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let mut encoder: CommandEncoder = self
            .wgpu_context
            .device_and_queue
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Screenshot Encoder"),
            });

        let output_buffer_size = self.wgpu_context.size.width as u64
            * self.wgpu_context.size.height as u64
            * std::mem::size_of::<u32>() as u64;
        let output_buffer_desc = wgpu::BufferDescriptor {
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            label: None,
            mapped_at_creation: false,
        };
        let output_buffer = self
            .wgpu_context
            .device_and_queue
            .device
            .create_buffer(&output_buffer_desc);

        let texture = &self.wgpu_context.buffer_texture;

        let SizeInPx { width, height } = self.wgpu_context.size;

        let copy_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

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
                    bytes_per_row: std::num::NonZeroU32::new(4 * width),
                    rows_per_image: std::num::NonZeroU32::new(height),
                },
            },
            copy_size,
        );
        self.wgpu_context
            .device_and_queue
            .queue
            .submit(Some(encoder.finish()));

        {
            let buffer_slice = output_buffer.slice(..);
            buffer_slice.map_async(wgpu::MapMode::Read, |_v| {});
            self.wgpu_context
                .device_and_queue
                .device
                .poll(wgpu::Maintain::Wait);

            let data = buffer_slice.get_mapped_range();

            use image::{ImageBuffer, Rgba};
            let mut buffer =
                ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec()).unwrap();

            for px in buffer.pixels_mut() {
                let cmp = px.0;
                *px = Rgba([cmp[2], cmp[1], cmp[0], cmp[3]]);
            }

            buffer.save(path).unwrap();
        }
        output_buffer.unmap();

        Ok(())
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
            operations: vec![],
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
    translation: Option<Position>,
    white_texture: Rc<Texture>,
    current_operation_block: Option<OperationBlock>,
    draw_datas: Vec<DrawData>,
    render_preper: RenderPreper,
    buffer_cache: BufferCache,
    texture_context: &'a TextureContext,
    text_converter: TextConverter,
}

impl<'a> Graphics<'a> {
    fn new(
        size: SizeInPx,
        white_texture: Rc<Texture>,
        device: &'a Device,
        queue: &'a Queue,
        texture_context: &'a TextureContext,
    ) -> Self {
        Graphics {
            current_operation_block: None,
            draw_datas: vec![],
            size,
            translation: None,
            white_texture,
            texture_context,
            device,
            queue,
            text_converter: TextConverter::new(),
            render_preper: RenderPreper::new(),
            buffer_cache: BufferCache::new(),
        }
    }

    pub fn draw_rect<R: Into<Rect>>(&mut self, rect: R, color: Color) -> &mut RenderOperation {
        let tex_coords = Rect::new(0.0, 0.0, 1.0, 1.0);

        let rect: Rect = if let Some(translation) = self.translation {
            rect.into().translated(translation.x, translation.y)
        } else {
            rect.into()
        };
        let position: RenderPosition = rect.into();
        let color_matrix = ColorMatrix::from_color(color);
        let operation = RenderOperation {
            position,
            color_matrix,
            tex_coords,
        };

        self.get_operation_block(&self.white_texture.clone())
            .push_render_operation(operation)
    }

    pub fn draw_sprite(&mut self, sprite: &Sprite, position: Position) -> &mut RenderOperation {
        let destination = Rect {
            left: position.x,
            top: position.y,
            right: position.x + sprite.dimensions.width as f32,
            bottom: position.y + sprite.dimensions.height as f32,
        };
        self.draw_sprite_in_rect(sprite, destination)
    }

    pub fn draw_sprite_in_rect<R: Into<Rect>>(
        &mut self,
        sprite: &Sprite,
        rect: R,
    ) -> &mut RenderOperation {
        let tex_coords = sprite.tex_coords;
        let rect: Rect = if let Some(translation) = self.translation {
            rect.into().translated(translation.x, translation.y)
        } else {
            rect.into()
        };

        let position: RenderPosition = rect.into();
        let color_matrix = ColorMatrix::from_color(Color::rgb(1.0, 1.0, 1.0));
        let operation = RenderOperation {
            position,
            color_matrix,
            tex_coords,
        };
        self.get_operation_block(&sprite.texture)
            .push_render_operation(operation)
    }

    pub fn draw_text<T>(
        &mut self,
        font: &mut Font,
        text: T,
        px: u32,
        position: Position,
        color: Color,
    ) where
        T: AsRef<str>,
    {
        let position = if let Some(translation) = self.translation {
            position.translated(translation.x, translation.y)
        } else {
            position
        };
        let font_for_px = font.get_font_for_px(px);
        let mut operations = self.text_converter.render_operation(
            text.as_ref(),
            color,
            position,
            &font_for_px,
            &self.device,
            &self.queue,
            &self.texture_context,
        );

        let texture = font_for_px
            .borrow_mut()
            .get_or_create_texture(&self.device, &self.texture_context);
        self.get_operation_block(&texture)
            .operations
            .append(&mut operations);
    }

    pub fn with_translation<F>(&mut self, translation: Position, function: F)
    where
        F: FnOnce(&mut Self),
    {
        self.translation = Some(translation);
        function(self);
        self.translation = None;
    }

    pub fn size(&self) -> SizeInPx {
        self.size
    }

    fn get_operation_block(&mut self, texture: &Rc<Texture>) -> &mut OperationBlock {
        let need_new = !matches!(&self.current_operation_block, Some(operation_block) if operation_block.texture.id == texture.id);
        if need_new {
            self.prepare_current_block();

            self.current_operation_block = Some(OperationBlock::with_texture(texture.clone()));
        }

        self.current_operation_block.as_mut().unwrap()
    }

    fn prepare_current_block(&mut self) {
        if let Some(draw_data) = self
            .current_operation_block
            .take()
            .and_then(|operation_block| {
                self.render_preper.prepare(
                    &mut self.buffer_cache,
                    &self.device,
                    &self.queue,
                    operation_block,
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
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    pub fn square(x: f32, y: f32, width: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + width,
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn translated(&self, x: f32, y: f32) -> Self {
        Self {
            left: self.left + x,
            top: self.top + y,
            right: self.right + x,
            bottom: self.bottom + y,
        }
    }
}

impl From<[i32; 4]> for Rect {
    fn from(coordinates: [i32; 4]) -> Self {
        Rect {
            left: coordinates[0] as f32,
            top: coordinates[1] as f32,
            right: coordinates[2] as f32,
            bottom: coordinates[3] as f32,
        }
    }
}

impl std::ops::Mul<f32> for &Rect {
    type Output = Rect;

    fn mul(self, rhs: f32) -> Self::Output {
        Rect {
            left: self.left * rhs,
            top: self.top * rhs,
            right: self.right * rhs,
            bottom: self.bottom * rhs,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    pub fn new(left: f32, top: f32) -> Self {
        Self { x: left, y: top }
    }

    pub fn translated(&self, x: f32, y: f32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SizeInPx {
    pub width: u32,
    pub height: u32,
}

impl SizeInPx {
    pub fn new(width: u32, height: u32) -> Self {
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
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug)]
pub(crate) struct DeviceAndQueue {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Debug)]
pub(crate) struct WgpuContext {
    surface: wgpu::Surface,
    pub device_and_queue: Rc<DeviceAndQueue>,
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
        let device_and_queue = Rc::new(DeviceAndQueue { device, queue });

        Ok(WgpuContext {
            surface,
            config,
            device_and_queue,
            size,
            buffer_texture,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = SizeInPx { width, height };
        self.config.width = width;
        self.config.height = height;
        self.surface
            .configure(&self.device_and_queue.device, &self.config);
        self.buffer_texture =
            self.device_and_queue
                .device
                .create_texture(&wgpu::TextureDescriptor {
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
    }
}

pub(crate) struct RenderPosition {
    transformation: Mat4,
    scale: Position,
}

impl RenderPosition {
    pub fn matrix(&self) -> Mat4 {
        let scale = Mat4::from_scale(Vec3::new(self.scale.x, self.scale.y, 1.0));
        self.transformation * scale
    }
}

impl From<Rect> for RenderPosition {
    fn from(rect: Rect) -> Self {
        let transformation = Mat4::from_translation(Vec3::new(rect.left, rect.top, 0.0));
        let scale = Position::new(rect.width(), rect.height());
        Self {
            transformation,
            scale,
        }
    }
}

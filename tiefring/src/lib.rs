use std::{path::Path, rc::Rc};

use cache::{BufferCache, ReusableBuffer};
use camera::{Camera, CameraSettings};

use glam::{Mat4, Vec3};
use raw_window_handle::HasRawWindowHandle;
use renderer::{ColorMatrix, RenderOperation, RenderPreper, Renderer};
use sprite::{Sprite, Texture, TextureContext};
use text::{Font, TextConverter};
use thiserror::Error;
use wgpu::{CommandEncoder, RenderPass};

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

pub struct Canvas {
    wgpu_context: WgpuContext,
    graphics: Graphics,
    camera: Camera,
    pub(crate) canvas_settings: CanvasSettings,
    texture_context: Rc<TextureContext>,
    renderer: Renderer,
}

impl Canvas {
    pub async fn new<W>(
        window: &W,
        width: u32,
        height: u32,
        canvas_settings: CanvasSettings,
    ) -> Result<Canvas, Error>
    where
        W: HasRawWindowHandle,
    {
        let wgpu_context = WgpuContext::new(window, width, height).await?;
        let camera = Camera::new(
            &wgpu_context.device_and_queue,
            CameraSettings {
                scale: canvas_settings.scale,
                translation: Position::new(0.0, 0.0),
                width,
                height,
            },
        );
        let texture_context = Rc::new(TextureContext::new(&wgpu_context.device_and_queue));
        let renderer = Renderer::new(&wgpu_context.device_and_queue, &texture_context, &camera);

        let white_texture = Rc::new(Texture::new(
            &wgpu_context.device_and_queue,
            &texture_context.texture_bind_group_layout,
            &texture_context.sampler,
            &[255, 255, 255, 255],
            SizeInPx::new(1, 1),
        ));

        let graphics = Graphics::new(
            width,
            height,
            white_texture,
            wgpu_context.device_and_queue.clone(),
            texture_context.clone(),
        );

        Ok(Canvas {
            wgpu_context,
            graphics,
            camera,
            canvas_settings,
            texture_context,
            renderer,
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

        draw_function(&mut self.graphics);
        self.graphics.prepare_current_block();

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
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.canvas_settings.background_color.into()),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            self.render_draw_calls(&mut render_pass);
        }

        self.cleanup_draw_calls();

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
        self.graphics.size = SizeInPx { width, height };
        self.wgpu_context.resize(width, height);
        self.camera
            .set_size(&self.wgpu_context.device_and_queue.queue, width, height);
    }

    pub fn size(&self) -> SizeInPx {
        self.wgpu_context.size
    }

    pub fn scale(&self) -> f32 {
        self.canvas_settings.scale
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.canvas_settings.scale = scale;

        self.camera
            .set_scale(&self.wgpu_context.device_and_queue.queue, scale);
    }

    pub fn translation(&self) -> Position {
        self.camera.camera_settings.translation
    }

    pub fn set_translation(&mut self, translation: Position) {
        self.camera
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
            let mapping = buffer_slice.map_async(wgpu::MapMode::Read);
            self.wgpu_context
                .device_and_queue
                .device
                .poll(wgpu::Maintain::Wait);
            mapping.await.unwrap();

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

    fn cleanup_draw_calls(&mut self) {
        self.graphics.reset();
    }

    fn render_draw_calls<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_bind_group(0, &self.camera.camera_bind_group, &[]);

        for DrawData {
            instance_buffer,
            count,
            texture,
        } in &self.graphics.draw_datas
        {
            self.renderer
                .render(render_pass, instance_buffer, *count, texture);
        }
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

pub struct Graphics {
    current_operation_block: Option<OperationBlock>,
    draw_datas: Vec<DrawData>,
    size: SizeInPx,
    translation: Option<Position>,
    white_texture: Rc<Texture>,
    texture_context: Rc<TextureContext>,
    device_and_queue: Rc<DeviceAndQueue>,
    text_converter: TextConverter,
    render_preper: RenderPreper,
    buffer_cache: BufferCache,
}

impl Graphics {
    fn new(
        width: u32,
        height: u32,
        white_texture: Rc<Texture>,
        device_and_queue: Rc<DeviceAndQueue>,
        texture_context: Rc<TextureContext>,
    ) -> Self {
        Graphics {
            current_operation_block: None,
            draw_datas: vec![],
            size: SizeInPx { width, height },
            translation: None,
            white_texture,
            texture_context,
            device_and_queue,
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
        let font_for_px = font.get_font_for_px(px);
        let mut operations = self.text_converter.render_operation(
            text.as_ref(),
            color,
            position,
            &font_for_px,
            &self.device_and_queue,
            &self.texture_context,
        );

        let texture = font_for_px
            .borrow_mut()
            .get_or_create_texture(&self.device_and_queue, &self.texture_context);
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

    fn reset(&mut self) {
        // We cleanup buffers that were not reused previously.
        self.buffer_cache.clear();
        self.current_operation_block = None;
        for draw_data in self.draw_datas.drain(..) {
            self.buffer_cache.release_buffer(draw_data.instance_buffer);
        }
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
        if let Some(draw_data) =
            self.current_operation_block
                .take()
                .into_iter()
                .find_map(|operation_block| {
                    self.render_preper.prepare(
                        &mut self.buffer_cache,
                        &self.device_and_queue,
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
        W: HasRawWindowHandle,
    {
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
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

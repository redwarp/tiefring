use std::{cell::RefCell, path::Path, rc::Rc};

use camera::Camera;
use raw_window_handle::HasRawWindowHandle;
use shape::ColorRenderer;
use sprite::{Sprite, Texture, TextureId, TextureRenderer};
use text::{Font, FontForPx, FontId, TextContext, TextRenderer};
use thiserror::Error;

use wgpu::{Buffer, CommandEncoder, RenderPass, SurfaceError};

mod camera;
mod shape;
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
    color_renderer: ColorRenderer,
    texture_renderer: TextureRenderer,
    camera: Camera,
    pub(crate) canvas_settings: CanvasSettings,
    text_context: TextContext,
    text_renderer: TextRenderer,
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
        let graphics = Graphics::new();
        let camera = Camera::new(&wgpu_context, width, height, &canvas_settings.canvas_zero);
        let color_renderer = ColorRenderer::new(&wgpu_context, &camera);
        let texture_renderer = TextureRenderer::new(&wgpu_context, &camera);
        let text_context = TextContext::new(&wgpu_context);
        let text_renderer = TextRenderer::new(&wgpu_context, &text_context, &camera);
        Ok(Canvas {
            wgpu_context,
            graphics,
            color_renderer,
            texture_renderer,
            camera,
            canvas_settings,
            text_context,
            text_renderer,
        })
    }

    pub fn draw<'a, F>(&'a mut self, draw_function: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Graphics),
    {
        self.graphics.reset();
        let mut encoder: CommandEncoder =
            self.wgpu_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        draw_function(&mut self.graphics);
        if let Some(operation_block) = self.graphics.current_operation_block.take() {
            self.graphics.operation_blocks.push(operation_block);
        }

        let surface_texture = self
            .wgpu_context
            .surface
            .get_current_texture()
            .map_err(|error: SurfaceError| Error::RenderingFailed(error))?;
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

            self.handle_draw_operations(&mut render_pass);
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

    pub fn resize(&mut self, width: u32, height: u32) {
        self.wgpu_context.resize(width, height);
        self.camera.resize(
            &self.wgpu_context,
            width,
            height,
            &self.canvas_settings.canvas_zero,
        );
    }

    pub async fn screenshot<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let mut encoder: CommandEncoder =
            self.wgpu_context
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
        let output_buffer = self.wgpu_context.device.create_buffer(&output_buffer_desc);

        let texture = &self.wgpu_context.buffer_texture;

        let Size { width, height } = self.wgpu_context.size;

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
        self.wgpu_context.queue.submit(Some(encoder.finish()));

        {
            let buffer_slice = output_buffer.slice(..);
            let mapping = buffer_slice.map_async(wgpu::MapMode::Read);
            self.wgpu_context.device.poll(wgpu::Maintain::Wait);
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

    fn handle_draw_operations<'a>(&'a mut self, render_pass: &mut RenderPass<'a>) {
        for operation_block in self.graphics.operation_blocks.iter_mut() {
            match operation_block.operation_type {
                OperationType::DrawRect => self.color_renderer.render(
                    render_pass,
                    &self.wgpu_context,
                    &self.camera,
                    &mut operation_block.draw_rect_operations,
                ),
                OperationType::DrawTexture(_) => self.texture_renderer.render(
                    render_pass,
                    &self.wgpu_context,
                    &self.camera,
                    &mut operation_block.draw_texture_operations,
                ),
                OperationType::DrawText(_) => self.text_renderer.render(
                    render_pass,
                    &self.wgpu_context,
                    &self.text_context,
                    &self.camera,
                    &mut operation_block.draw_text_operations,
                ),
            }
        }
    }
}

pub struct CanvasSettings {
    pub background_color: Color,
    pub canvas_zero: CanvasZero,
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self {
            background_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            canvas_zero: CanvasZero::TopLeft,
        }
    }
}

pub enum CanvasZero {
    TopLeft,
    Centered,
}

pub struct Graphics {
    current_operation_block: Option<OperationBlock>,
    operation_blocks: Vec<OperationBlock>,
}

struct OperationBlock {
    operation_type: OperationType,
    draw_rect_operations: DrawRectOperations,
    draw_texture_operations: DrawTextureOperations,
    draw_text_operations: DrawTextOperations,
}

impl OperationBlock {
    fn new(operation_type: OperationType) -> Self {
        OperationBlock {
            operation_type,
            draw_rect_operations: DrawRectOperations::new(),
            draw_texture_operations: DrawTextureOperations::new(),
            draw_text_operations: DrawTextOperations::new(),
        }
    }

    fn push_draw_text_operation(&mut self, draw_text_operation: DrawTextOperation) {
        self.draw_text_operations
            .operations
            .push(draw_text_operation);
    }

    fn push_draw_rect_operation(&mut self, draw_rect_operation: DrawRectOperation) {
        self.draw_rect_operations
            .operations
            .push(draw_rect_operation);
    }

    fn push_draw_texture_operation(&mut self, draw_texture_operation: DrawTextureOperation) {
        self.draw_texture_operations
            .operations
            .push(draw_texture_operation);
    }
}

struct DrawTextOperations {
    operations: Vec<DrawTextOperation>,
    buffers: Option<(Rc<Texture>, Buffer, Buffer)>,
}

impl DrawTextOperations {
    fn new() -> Self {
        Self {
            operations: Vec::new(),
            buffers: None,
        }
    }
}

struct DrawTextureOperations {
    operations: Vec<DrawTextureOperation>,
    buffers: Option<(Rc<Texture>, Buffer, Buffer, Vec<u16>)>,
}

impl DrawTextureOperations {
    fn new() -> Self {
        Self {
            operations: Vec::new(),
            buffers: None,
        }
    }
}

impl Graphics {
    fn new() -> Self {
        Graphics {
            current_operation_block: None,
            operation_blocks: Vec::new(),
        }
    }

    pub fn draw_rect<R: Into<Rect>>(&mut self, rect: R, color: Color) {
        self.get_operation_block(OperationType::DrawRect)
            .push_draw_rect_operation(DrawRectOperation(rect.into(), color));
    }

    pub fn draw_sprite(&mut self, sprite: &Sprite, position: Position) {
        let destination = Rect {
            left: position.left,
            top: position.top,
            right: position.left + sprite.dimensions.width as f32,
            bottom: position.top + sprite.dimensions.height as f32,
        };
        self.draw_sprite_in_rect(sprite, destination);
    }

    pub fn draw_sprite_in_rect<R: Into<Rect>>(&mut self, sprite: &Sprite, rect: R) {
        let tex_coords = sprite.tex_coords;
        let destination = rect.into();
        let texture = sprite.texture.clone();
        self.get_operation_block(OperationType::DrawTexture(sprite.texture.id))
            .push_draw_texture_operation(DrawTextureOperation {
                tex_coords,
                destination,
                texture,
            });
    }

    pub fn draw_text<T: Into<String>>(
        &mut self,
        font: &mut Font,
        text: T,
        px: u32,
        position: Position,
        color: Color,
    ) {
        let text: String = text.into();
        let font_for_px = font.get_font_for_px(px);
        self.get_operation_block(OperationType::DrawText(FontId(font.font.file_hash(), px)))
            .push_draw_text_operation(DrawTextOperation {
                font_for_px,
                position,
                text,
                color,
            });
    }

    fn reset(&mut self) {
        self.current_operation_block = None;
        self.operation_blocks.clear();
    }

    fn get_operation_block(&mut self, current_operation: OperationType) -> &mut OperationBlock {
        let need_new = match &self.current_operation_block {
            Some(operation_block) if operation_block.operation_type == current_operation => false,
            _ => true,
        };

        if need_new {
            if let Some(operation_block) = self.current_operation_block.take() {
                self.operation_blocks.push(operation_block);
            }

            let operation_block = OperationBlock::new(current_operation);
            self.current_operation_block = Some(operation_block);
        }

        self.current_operation_block.as_mut().unwrap()
    }
}

#[derive(PartialEq)]
enum OperationType {
    DrawRect,
    DrawTexture(TextureId),
    DrawText(FontId),
}

pub(crate) struct DrawRectOperation(Rect, Color);

struct DrawRectOperations {
    operations: Vec<DrawRectOperation>,
    buffers: Option<(Buffer, Buffer)>,
}

impl DrawRectOperations {
    fn new() -> Self {
        Self {
            operations: Vec::new(),
            buffers: None,
        }
    }
}

pub(crate) struct DrawTextureOperation {
    pub tex_coords: Rect,
    pub destination: Rect,
    pub texture: Rc<Texture>,
}

pub(crate) struct DrawTextOperation {
    pub position: Position,
    pub font_for_px: Rc<RefCell<FontForPx>>,
    pub text: String,
    pub color: Color,
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

impl From<(Position, Size)> for Rect {
    fn from((position, size): (Position, Size)) -> Self {
        Rect {
            left: position.left,
            top: position.top,
            right: position.left + size.width as f32,
            bottom: position.top + size.height as f32,
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

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl From<(u32, u32)> for Size {
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

impl Into<wgpu::Color> for Color {
    fn into(self) -> wgpu::Color {
        wgpu::Color {
            r: self.r as f64,
            g: self.g as f64,
            b: self.b as f64,
            a: self.a as f64,
        }
    }
}

impl Color {
    fn as_float_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

struct WgpuContext {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: Size,
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

        let size = Size { width, height };

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
            device,
            config,
            queue,
            size,
            buffer_texture,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = Size { width, height };
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.buffer_texture = self.device.create_texture(&wgpu::TextureDescriptor {
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

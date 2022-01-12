use std::{path::Path, rc::Rc};

use cache::{BufferCache, ReusableBuffer};
use camera::{Camera, CameraSettings};

use glam::{Mat4, Vec3};
use raw_window_handle::HasRawWindowHandle;
use renderer::{ColorMatrix, RenderOperation, RenderPreper, Renderer};
use sprite::{Sprite, Texture, TextureId};
use text::{DrawTextOperation, Font, FontId, TextContext, TextDataPreper, TextRenderer};
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
    text_context: TextContext,
    text_renderer: TextRenderer,
    renderer: Renderer,
    draw_data: Vec<DrawData>,
    buffer_cache: BufferCache,
    draw_data_prepers: DrawDataPrepers,
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
            &wgpu_context,
            CameraSettings {
                scale: canvas_settings.scale,
                translation: Position::new(0.0, 0.0),
                width,
                height,
            },
        );
        let text_context = TextContext::new(&wgpu_context);
        let text_renderer = TextRenderer::new(&wgpu_context, &text_context, &camera);
        let renderer = Renderer::new(&wgpu_context, &camera);
        let draw_data = vec![];
        let buffer_cache = BufferCache::new();

        let white_texture = Rc::new(Texture::new(
            &wgpu_context,
            &renderer.texture_bind_group_layout,
            &renderer.sampler,
            &[255, 255, 255, 255],
            SizeInPx::new(1, 1),
        ));

        let graphics = Graphics::new(width, height, white_texture);
        let draw_data_prepers = DrawDataPrepers::new();

        Ok(Canvas {
            wgpu_context,
            graphics,
            camera,
            canvas_settings,
            text_context,
            text_renderer,
            renderer,
            draw_data,
            buffer_cache,
            draw_data_prepers,
        })
    }

    pub fn draw<F>(&mut self, draw_function: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Graphics),
    {
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

            self.prepare_draw_calls();
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

        self.wgpu_context.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    pub fn redraw_last(&mut self) -> Result<(), Error> {
        let mut encoder: CommandEncoder =
            self.wgpu_context
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

        self.wgpu_context.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.graphics.size = SizeInPx { width, height };
        self.wgpu_context.resize(width, height);
        self.camera.set_size(&self.wgpu_context, width, height);
    }

    pub fn size(&self) -> SizeInPx {
        self.wgpu_context.size
    }

    pub fn scale(&self) -> f32 {
        self.canvas_settings.scale
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.canvas_settings.scale = scale;

        self.camera.set_scale(&self.wgpu_context, scale);
    }

    pub fn translation(&self) -> Position {
        self.camera.camera_settings.translation
    }

    pub fn set_translation(&mut self, translation: Position) {
        self.camera.set_translation(&self.wgpu_context, translation)
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

    fn cleanup_draw_calls(&mut self) {
        for draw_data in self.draw_data.drain(..) {
            match draw_data {
                DrawData::Text {
                    vertex_buffer,
                    index_buffer,
                    ..
                } => {
                    self.buffer_cache.release_buffer(vertex_buffer);
                    self.buffer_cache.release_buffer(index_buffer);
                }
                DrawData::Render {
                    instance_buffer, ..
                } => self.buffer_cache.release_buffer(instance_buffer),
            }
        }

        self.graphics.reset();
    }

    fn prepare_draw_calls(&mut self) {
        let draw_data = &mut self.draw_data;
        draw_data.clear();

        draw_data.extend(
            self.graphics
                .operation_blocks
                .drain(..)
                .filter_map(|operation_block| {
                    self.draw_data_prepers.prepare(
                        &self.wgpu_context,
                        &self.text_context,
                        operation_block,
                    )
                }),
        );

        // Remove buffers that were not reused.
        self.buffer_cache.clear();
    }

    fn render_draw_calls<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_bind_group(0, &self.camera.camera_bind_group, &[]);

        for draw_data in &self.draw_data {
            match draw_data {
                DrawData::Text {
                    vertex_buffer,
                    index_buffer,
                    count,
                    texture,
                } => self.text_renderer.render(
                    render_pass,
                    vertex_buffer,
                    index_buffer,
                    *count,
                    texture,
                ),
                DrawData::Render {
                    instance_buffer,
                    count,
                    texture,
                } => self
                    .renderer
                    .render(render_pass, instance_buffer, *count, texture),
            }
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

pub struct Graphics {
    current_operation_block: Option<OperationBlock>,
    operation_blocks: Vec<OperationBlock>,
    size: SizeInPx,
    translation: Option<Position>,
    white_texture: Rc<Texture>,
}

struct OperationBlock {
    operation_type: DrawOperationType,
    draw_text_operations: Vec<DrawTextOperation>,
    render_operations: Vec<RenderOperation>,
    texture: Option<Rc<Texture>>,
}

impl OperationBlock {
    fn new(operation_type: DrawOperationType) -> Self {
        OperationBlock {
            operation_type,
            draw_text_operations: vec![],
            render_operations: vec![],
            texture: None,
        }
    }

    fn with_texture(texture: Rc<Texture>) -> Self {
        OperationBlock {
            operation_type: DrawOperationType::ExpRect(texture.id),
            draw_text_operations: vec![],
            render_operations: vec![],
            texture: Some(texture),
        }
    }

    fn push_draw_text_operation(&mut self, draw_text_operation: DrawTextOperation) {
        self.draw_text_operations.push(draw_text_operation);
    }

    fn push_render_operation(&mut self, render_operation: RenderOperation) {
        self.render_operations.push(render_operation);
    }
}

enum DrawData {
    Text {
        vertex_buffer: ReusableBuffer,
        index_buffer: ReusableBuffer,
        count: u32,
        texture: Rc<Texture>,
    },
    Render {
        instance_buffer: ReusableBuffer,
        count: u32,
        texture: Rc<Texture>,
    },
}

impl Graphics {
    fn new(width: u32, height: u32, white_texture: Rc<Texture>) -> Self {
        Graphics {
            current_operation_block: None,
            operation_blocks: Vec::new(),
            size: SizeInPx { width, height },
            translation: None,
            white_texture,
        }
    }

    pub fn draw_rect<R: Into<Rect>>(&mut self, rect: R, color: Color) {
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
        self.get_operation_block_2(&self.white_texture.clone())
            .push_render_operation(operation);
    }

    pub fn draw_sprite(&mut self, sprite: &Sprite, position: Position) {
        let destination = Rect {
            left: position.x,
            top: position.y,
            right: position.x + sprite.dimensions.width as f32,
            bottom: position.y + sprite.dimensions.height as f32,
        };
        self.draw_sprite_in_rect(sprite, destination);
    }

    pub fn draw_sprite_in_rect<R: Into<Rect>>(&mut self, sprite: &Sprite, rect: R) {
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
        self.get_operation_block_2(&sprite.texture)
            .push_render_operation(operation);
    }

    pub fn draw_text<T>(
        &mut self,
        font: &mut Font,
        text: T,
        px: u32,
        position: Position,
        color: Color,
    ) where
        T: Into<String>,
    {
        let position = if let Some(translation) = self.translation {
            position.translated(translation.x, translation.y)
        } else {
            position
        };
        let text: String = text.into();
        let font_for_px = font.get_font_for_px(px);
        self.get_operation_block(DrawOperationType::Text(FontId(font.font.file_hash(), px)))
            .push_draw_text_operation(DrawTextOperation {
                font_for_px,
                position,
                text,
                color,
            });
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
        self.current_operation_block = None;
        self.operation_blocks.clear();
    }

    fn get_operation_block(&mut self, current_operation: DrawOperationType) -> &mut OperationBlock {
        let need_new = !matches!(&self.current_operation_block, Some(operation_block) if operation_block.operation_type == current_operation);
        if need_new {
            if let Some(operation_block) = self.current_operation_block.take() {
                self.operation_blocks.push(operation_block);
            }

            let operation_block = OperationBlock::new(current_operation);
            self.current_operation_block = Some(operation_block);
        }

        self.current_operation_block.as_mut().unwrap()
    }

    fn get_operation_block_2(&mut self, texture: &Rc<Texture>) -> &mut OperationBlock {
        let current_operation = DrawOperationType::ExpRect(texture.id);
        let need_new = !matches!(&self.current_operation_block, Some(operation_block) if operation_block.operation_type == current_operation);
        if need_new {
            if let Some(operation_block) = self.current_operation_block.take() {
                self.operation_blocks.push(operation_block);
            }

            self.current_operation_block = Some(OperationBlock::with_texture(texture.clone()));
        }

        self.current_operation_block.as_mut().unwrap()
    }
}

#[derive(PartialEq, Debug)]
enum DrawOperationType {
    Text(FontId),
    ExpRect(TextureId),
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

    fn as_float_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[derive(Debug)]
pub(crate) struct WgpuContext {
    surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
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
        self.size = SizeInPx { width, height };
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

trait DrawDataPreper<O, C> {
    fn prepare(
        &mut self,
        buffer_cache: &mut BufferCache,
        context: C,
        operations: &[O],
    ) -> Option<DrawData>;
}

struct DrawDataPrepers {
    text_data_preper: TextDataPreper,
    render_preper: RenderPreper,
    buffer_cache: BufferCache,
}

impl DrawDataPrepers {
    fn new() -> Self {
        Self {
            text_data_preper: TextDataPreper::new(),
            render_preper: RenderPreper::new(),
            buffer_cache: BufferCache::new(),
        }
    }

    fn prepare(
        &mut self,
        wgpu_context: &WgpuContext,
        text_context: &TextContext,
        operation_block: OperationBlock,
    ) -> Option<DrawData> {
        match operation_block.operation_type {
            DrawOperationType::Text(_) => self.text_data_preper.prepare(
                &mut self.buffer_cache,
                (wgpu_context, text_context),
                &operation_block.draw_text_operations,
            ),
            DrawOperationType::ExpRect(_) => self.render_preper.prepare(
                &mut self.buffer_cache,
                wgpu_context,
                operation_block.texture.expect("Should be set").clone(),
                &operation_block.render_operations,
            ),
        }
    }
}

pub(crate) struct RenderPosition(Mat4);

impl From<Rect> for RenderPosition {
    fn from(rect: Rect) -> Self {
        let position = Mat4::from_translation(Vec3::new(rect.left, rect.top, 1.0))
            * Mat4::from_scale(Vec3::new(rect.width(), rect.height(), 1.0));
        Self(position)
    }
}

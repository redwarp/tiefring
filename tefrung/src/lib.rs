use std::{cell::RefCell, path::Path, rc::Rc};

use camera::Camera;
use raw_window_handle::HasRawWindowHandle;
use renderer::ColorRenderer;
use sprite::{Sprite, Texture, TextureId, TextureRenderer};
use thiserror::Error;

pub use wgpu::Color;
use wgpu::{CommandEncoder, RenderPass, Sampler, SurfaceError, TextureView};

mod camera;
mod renderer;
pub mod sprite;

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
        Ok(Canvas {
            wgpu_context,
            graphics,
            color_renderer,
            texture_renderer,
            camera,
            canvas_settings,
        })
    }

    pub fn draw<'a, F>(&'a mut self, draw_function: F) -> Result<(), Error>
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

        let surface_texture = self
            .wgpu_context
            .surface
            .get_current_texture()
            .map_err(|error: SurfaceError| Error::RenderingFailed(error))?;
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let depth_view = self.wgpu_context.depth_texture.view.clone();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.canvas_settings.background_color),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(-100.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            self.handle_draw_operations(&mut render_pass);
        }

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

    fn handle_draw_operations<'a>(&'a mut self, render_pass: &mut RenderPass<'a>) {
        self.color_renderer.render(
            render_pass,
            &self.wgpu_context,
            &self.camera,
            &self.graphics.draw_rect_operations,
        );
        self.texture_renderer.render(
            render_pass,
            &self.wgpu_context,
            &self.camera,
            &mut self.graphics.draw_texture_operations,
        );

        self.graphics.reset();
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
    index: u16,
    previous_operation: Option<OperationType>,
    draw_rect_operations: Vec<DrawRectOperation>,
    draw_texture_operations: Vec<DrawTextureOperation>,
}

impl Graphics {
    fn new() -> Self {
        Graphics {
            index: 0,
            previous_operation: None,
            draw_rect_operations: vec![],
            draw_texture_operations: vec![],
        }
    }

    pub fn draw_rect<R: Into<Rect>>(&mut self, rect: R, color: Color) {
        let index = self.next_index(OperationType::DrawRect);
        self.draw_rect_operations
            .push(DrawRectOperation(index, rect.into(), color));
    }

    pub fn draw_sprite(&mut self, sprite: &Sprite, position: Position) {
        let destination = Rect {
            left: position.left,
            top: position.top,
            right: position.left + sprite.size.width as f32,
            bottom: position.top + sprite.size.height as f32,
        };
        self.draw_sprite_in_rect(sprite, destination);
    }

    pub fn draw_sprite_in_rect<R: Into<Rect>>(&mut self, sprite: &Sprite, rect: R) {
        let index = self.next_index(OperationType::DrawTexture(sprite.texture.id));
        let tex_coords = sprite.tex_coords;
        let destination = rect.into();
        let texture = sprite.texture.clone();
        self.draw_texture_operations.push(DrawTextureOperation {
            index,
            tex_coords,
            destination,
            texture,
        });
    }

    pub fn reset(&mut self) {
        self.index = 0;
        self.draw_rect_operations.clear();
        self.draw_texture_operations.clear();
    }

    fn next_index(&mut self, current_operation: OperationType) -> u16 {
        if let Some(previous_operation) = &self.previous_operation {
            if previous_operation != &current_operation {
                self.index += 1;
            }
        }
        self.previous_operation = Some(current_operation);
        self.index
    }
}

#[derive(PartialEq)]
enum OperationType {
    DrawRect,
    DrawTexture(TextureId),
}

pub(crate) struct DrawRectOperation(u16, Rect, Color);

pub(crate) struct DrawTextureOperation {
    pub index: u16,
    pub tex_coords: Rect,
    pub destination: Rect,
    pub texture: Rc<Texture>,
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

pub struct Position {
    pub left: f32,
    pub top: f32,
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

#[derive(Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl From<(u32, u32)> for Size {
    fn from(size: (u32, u32)) -> Self {
        Self {
            width: size.0,
            height: size.1,
        }
    }
}

struct WgpuContext {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: Size,
    depth_texture: DepthTexture,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface
                .get_preferred_format(&adapter)
                .ok_or(Error::InitializationFailed)?,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let size = Size { width, height };

        let depth_texture = DepthTexture::create_depth_texture(&device, &config, "depth_texture");

        Ok(WgpuContext {
            surface,
            device,
            config,
            queue,
            size,
            depth_texture,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = Size { width, height };
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_texture =
            DepthTexture::create_depth_texture(&self.device, &self.config, "depth_texture");
    }
}

struct DepthTexture {
    texture: wgpu::Texture,
    view: Rc<TextureView>,
    sampler: Sampler,
}

impl DepthTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            // 2.
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT // 3.
                | wgpu::TextureUsages::TEXTURE_BINDING,
        };
        let texture = device.create_texture(&desc);

        let view = Rc::new(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            // 4.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual), // 5.
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }
}

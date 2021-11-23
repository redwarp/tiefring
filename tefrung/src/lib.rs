use std::{cell::RefCell, path::Path, rc::Rc};

use camera::Camera;
use raw_window_handle::HasRawWindowHandle;
use renderer::ColorRenderer;
use sprite::{Sprite, TextureId, TextureRenderer, TextureRepository};
use thiserror::Error;

pub use wgpu::Color;
use wgpu::{CommandEncoder, RenderPass, SurfaceError};

mod camera;
mod renderer;
mod sprite;

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
    texture_repository: Rc<RefCell<TextureRepository>>,
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
        let texture_repository = Rc::new(RefCell::new(TextureRepository::new()));
        Ok(Canvas {
            wgpu_context,
            graphics,
            color_renderer,
            texture_renderer,
            camera,
            texture_repository,
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

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        self.handle_draw_operations(&mut render_pass);

        drop(render_pass);
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

    pub fn load_sprite<P: AsRef<Path>>(&mut self, path: P) -> Option<Sprite> {
        Sprite::load_image(self, path)
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
            &self.texture_repository,
            &self.camera,
            &self.graphics.draw_texture_operations,
        );

        self.graphics.draw_rect_operations.clear();
        self.graphics.draw_texture_operations.clear();
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
    draw_rect_operations: Vec<DrawRectOperation>,
    draw_texture_operations: Vec<DrawTextureOperation>,
}

impl Graphics {
    fn new() -> Self {
        Graphics {
            draw_rect_operations: vec![],
            draw_texture_operations: vec![],
        }
    }

    pub fn draw_rect<R: Into<Rect>>(&mut self, rect: R, color: Color) {
        self.draw_rect_operations
            .push(DrawRectOperation(rect.into(), color));
    }

    pub fn draw_sprite(&mut self, position: Position, sprite: &Sprite) {
        let tex_coords = sprite.tex_coords;
        let destination = Rect {
            left: position.left,
            top: position.top,
            right: position.left + sprite.rect.width(),
            bottom: position.top + sprite.rect.height(),
        };
        let texture_id = sprite.texture_id;
        self.draw_texture_operations.push(DrawTextureOperation {
            tex_coords,
            destination,
            texture_id,
        });
    }
}

pub(crate) struct DrawRectOperation(Rect, Color);

pub(crate) struct DrawTextureOperation {
    pub tex_coords: Rect,
    pub destination: Rect,
    pub texture_id: TextureId,
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
pub(crate) struct Size {
    pub width: u32,
    pub height: u32,
}

struct WgpuContext {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: Size,
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

        Ok(WgpuContext {
            surface,
            device,
            config,
            queue,
            size,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = Size { width, height };
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

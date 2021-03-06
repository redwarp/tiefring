use glam::{Mat4, Vec3};
use wgpu::{util::DeviceExt, BindGroup, BindGroupLayout, Buffer};

use crate::{DeviceAndQueue, Position};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    matrix: [f32; 16],
}

impl CameraUniform {}

#[derive(Debug, Clone, Copy)]
pub struct CameraSettings {
    pub(crate) scale: f32,
    pub(crate) translation: Position,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug)]
pub struct Camera {
    pub(crate) camera_settings: CameraSettings,
    pub(crate) camera_buffer: Buffer,
    pub(crate) camera_bind_group_layout: BindGroupLayout,
    pub(crate) camera_bind_group: BindGroup,
}

impl Camera {
    pub(crate) fn new(device_and_queue: &DeviceAndQueue, camera_settings: CameraSettings) -> Self {
        let camera_uniform = CameraUniform {
            matrix: Camera::matrix(&camera_settings),
        };

        let camera_buffer =
            device_and_queue
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Projection matrix buffer"),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let camera_bind_group_layout =
            device_and_queue
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("camera_bind_group_layout"),
                });
        let camera_bind_group =
            device_and_queue
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &camera_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    }],
                    label: Some("camera_bind_group"),
                });

        Camera {
            camera_settings,
            camera_buffer,
            camera_bind_group_layout,
            camera_bind_group,
        }
    }

    pub(crate) fn set_scale(&mut self, queue: &wgpu::Queue, scale: f32) {
        self.camera_settings.scale = scale;
        self.recalculate(queue);
    }

    pub(crate) fn set_size(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.camera_settings.width = width;
        self.camera_settings.height = height;
        self.recalculate(queue);
    }

    pub(crate) fn set_translation(&mut self, queue: &wgpu::Queue, translation: Position) {
        self.camera_settings.translation = translation;
        self.recalculate(queue);
    }

    fn recalculate(&mut self, queue: &wgpu::Queue) {
        let camera_uniform = CameraUniform {
            matrix: Camera::matrix(&self.camera_settings),
        };

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );
    }

    fn matrix(camera_settings: &CameraSettings) -> [f32; 16] {
        (Camera::projection_matrix(camera_settings.width, camera_settings.height)
            * Camera::view_matrix(camera_settings.scale, camera_settings.translation))
        .to_cols_array()
    }

    fn projection_matrix(width: u32, height: u32) -> Mat4 {
        Mat4::orthographic_rh(0.0, width as f32, height as f32, 0.0, -100.0, 100.0)
    }

    fn view_matrix(scale: f32, translate: Position) -> Mat4 {
        Mat4::from_scale(Vec3::new(scale, scale, 1.0))
            * Mat4::from_translation(Vec3::new(translate.x, translate.y, 0.0))
    }
}

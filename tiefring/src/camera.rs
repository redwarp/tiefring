use glam::{Mat4, Vec3};
use wgpu::{util::DeviceExt, BindGroup, BindGroupLayout, Buffer};

use crate::WgpuContext;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    matrix: [f32; 16],
}

impl CameraUniform {}

pub struct Camera {
    pub(crate) camera_buffer: Buffer,
    pub(crate) camera_bind_group_layout: BindGroupLayout,
    pub(crate) camera_bind_group: BindGroup,
}

impl Camera {
    pub(crate) fn new(wgpu_context: &WgpuContext, width: u32, height: u32, scale: f32) -> Self {
        let camera_uniform = CameraUniform {
            matrix: (Camera::projection_matrix(width, height) * Camera::view_matrix(scale))
                .to_cols_array(),
        };

        let camera_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Projection matrix buffer"),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let camera_bind_group_layout =
            wgpu_context
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
        let camera_bind_group = wgpu_context
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
            camera_buffer,
            camera_bind_group_layout,
            camera_bind_group,
        }
    }

    pub(crate) fn resize(
        &mut self,
        wgpu_context: &WgpuContext,
        width: u32,
        height: u32,
        scale: f32,
    ) {
        let camera_uniform = CameraUniform {
            matrix: (Camera::projection_matrix(width, height) * Camera::view_matrix(scale))
                .to_cols_array(),
        };

        wgpu_context.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );
    }

    fn projection_matrix(width: u32, height: u32) -> Mat4 {
        Mat4::orthographic_rh(0.0, width as f32, height as f32, 0.0, -100.0, 100.0)
    }

    fn view_matrix(scale: f32) -> Mat4 {
        Mat4::from_scale(Vec3::new(scale, scale, 1.0))
    }
}

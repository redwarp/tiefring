use cgmath::Matrix4;
use wgpu::{util::DeviceExt, BindGroup, BindGroupLayout, Buffer};

use crate::{CanvasZero, WgpuContext};

// We need this for Rust to store our data correctly for the shaders
#[repr(C)]
// This is so we can store this in a buffer
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    // We can't use cgmath with bytemuck directly so we'll have
    // to convert the Matrix4 into a 4x4 f32 array
    projection: [[f32; 4]; 4],
}

impl CameraUniform {}

#[rustfmt::skip]
const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

pub struct Camera {
    pub(crate) camera_buffer: Buffer,
    pub(crate) camera_bind_group_layout: BindGroupLayout,
    pub(crate) camera_bind_group: BindGroup,
}

impl Camera {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        width: u32,
        height: u32,
        canvas_zero: &CanvasZero,
    ) -> Self {
        let camera_uniform = CameraUniform {
            projection: Camera::projection_matrix(width, height, canvas_zero).into(),
        };

        let camera_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera buffer"),
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
        canvas_zero: &CanvasZero,
    ) {
        let camera_uniform = CameraUniform {
            projection: Camera::projection_matrix(width, height, canvas_zero).into(),
        };

        let updated_camera_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera buffer"),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_SRC,
                });

        let mut encoder =
            wgpu_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Update Camera buffer"),
                });
        encoder.copy_buffer_to_buffer(
            &updated_camera_buffer,
            0,
            &self.camera_buffer,
            0,
            std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress,
        );
        wgpu_context.queue.submit(Some(encoder.finish()));
    }

    fn projection_matrix(width: u32, height: u32, canvas_zero: &CanvasZero) -> Matrix4<f32> {
        let projection = match canvas_zero {
            CanvasZero::Centered => cgmath::ortho(
                (-(width as f32 / 2.0)).floor(),
                (width as f32 / 2.0).ceil(),
                (height as f32 / 2.0).ceil(),
                (-(height as f32 / 2.0)).floor(),
                0.0,
                1.0,
            ),
            CanvasZero::TopLeft => {
                cgmath::ortho(0.0, width as f32, height as f32, 0.0, -100.0, 100.0)
            }
        };

        OPENGL_TO_WGPU_MATRIX * projection
    }
}

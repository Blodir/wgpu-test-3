use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt as _;

use crate::host::world::prepare::camera::PreparedCamera;

pub struct CameraBuffers {
    pub view_proj: wgpu::Buffer,
    pub position: wgpu::Buffer,
    pub inverse_view_proj: wgpu::Buffer,
    pub prev_inverse_view_proj: wgpu::Buffer,
    pub view_rotation: wgpu::Buffer,
    pub unjittered_view_proj: wgpu::Buffer,
    pub prev_unjittered_view_proj: wgpu::Buffer,
    pub jitter: wgpu::Buffer,
}

impl CameraBuffers {
    fn padded_view_rotation(right: Vec3, up: Vec3, look: Vec3) -> [[f32; 4]; 3] {
        [
            [right.x, right.y, right.z, 0.0],
            [up.x, up.y, up.z, 0.0],
            [look.x, look.y, look.z, 0.0],
        ]
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let position = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Position Buffer"),
            contents: bytemuck::cast_slice(&Vec3::ZERO.to_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let inverse_view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Inverse View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let prev_inverse_view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Previous Inverse View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let view_rotation = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera View Rotation Buffer"),
            contents: bytemuck::cast_slice(&Self::padded_view_rotation(Vec3::X, Vec3::Y, -Vec3::Z)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let unjittered_view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Unjittered View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let prev_unjittered_view_proj =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Previous Unjittered View Projection Buffer"),
                contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let jitter = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Jitter Buffer"),
            contents: bytemuck::cast_slice(&[0.0f32; 4]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            view_proj,
            position,
            inverse_view_proj,
            prev_inverse_view_proj,
            view_rotation,
            unjittered_view_proj,
            prev_unjittered_view_proj,
            jitter,
        }
    }

    pub fn update(
        &self,
        camera: &PreparedCamera,
        prev_camera: &PreparedCamera,
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            &self.view_proj,
            0,
            bytemuck::cast_slice(&camera.view_proj.to_cols_array()),
        );
        queue.write_buffer(
            &self.position,
            0,
            bytemuck::cast_slice(&camera.state.position.to_array()),
        );
        queue.write_buffer(
            &self.inverse_view_proj,
            0,
            bytemuck::cast_slice(&camera.inverse_view_proj.to_cols_array()),
        );
        queue.write_buffer(
            &self.prev_inverse_view_proj,
            0,
            bytemuck::cast_slice(&prev_camera.inverse_view_proj.to_cols_array()),
        );
        queue.write_buffer(
            &self.view_rotation,
            0,
            bytemuck::cast_slice(&camera.view_rotation),
        );
        queue.write_buffer(
            &self.unjittered_view_proj,
            0,
            bytemuck::cast_slice(&camera.unjittered_view_proj.to_cols_array()),
        );
        queue.write_buffer(
            &self.prev_unjittered_view_proj,
            0,
            bytemuck::cast_slice(&prev_camera.unjittered_view_proj.to_cols_array()),
        );
        queue.write_buffer(
            &self.jitter,
            0,
            bytemuck::cast_slice(&[
                camera.jitter_uv.x,
                camera.jitter_uv.y,
                prev_camera.jitter_uv.x,
                prev_camera.jitter_uv.y,
            ]),
        );
    }
}

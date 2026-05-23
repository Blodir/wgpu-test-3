use glam::{Mat3, Mat4};
use wgpu::util::DeviceExt as _;

use crate::host::{
    wgpu_context::WgpuContext, world::buffers::instance_links::SnapshotInstanceLinks,
};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedInstance {
    pub m4: [[f32; 4]; 4],
    pub itr: [[f32; 4]; 3],
    pub offsets: [u32; 4],
}

fn pad_mat3(mat: Mat3) -> [[f32; 4]; 3] {
    let cols = mat.to_cols_array_2d();
    [
        [cols[0][0], cols[0][1], cols[0][2], 0.0],
        [cols[1][0], cols[1][1], cols[1][2], 0.0],
        [cols[2][0], cols[2][1], cols[2][2], 0.0],
    ]
}

impl Default for SkinnedInstance {
    fn default() -> Self {
        Self {
            m4: Mat4::IDENTITY.to_cols_array_2d(),
            itr: pad_mat3(Mat3::IDENTITY),
            offsets: [0, u32::MAX, 0, 0],
        }
    }
}
impl SkinnedInstance {
    pub fn new(transform: Mat4, palette_offset: u32) -> Self {
        let m4 = transform.to_cols_array_2d();
        let itr = Mat3::from_mat4(transform).inverse().transpose();

        SkinnedInstance {
            m4,
            itr: pad_mat3(itr),
            offsets: [palette_offset, u32::MAX, 0, 0],
        }
    }

    pub fn from(mat4: Mat4, itr: Mat3, palette_offset: u32) -> Self {
        Self {
            m4: mat4.to_cols_array_2d(),
            itr: pad_mat3(itr),
            offsets: [palette_offset, u32::MAX, 0, 0],
        }
    }
}

pub struct SkinnedInstances {
    pub buffer: wgpu::Buffer,
    pub prev_buffer: wgpu::Buffer,
    pub links: SnapshotInstanceLinks,
}
impl SkinnedInstances {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let instance_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance buffer"),
                    contents: bytemuck::cast_slice(&[SkinnedInstance::default()]),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });
        let prev_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Previous instance buffer"),
                    contents: bytemuck::cast_slice(&[SkinnedInstance::default()]),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });
        Self {
            buffer: instance_buffer,
            prev_buffer,
            links: SnapshotInstanceLinks::default(),
        }
    }

    pub fn update(
        &mut self,
        data: Vec<SkinnedInstance>,
        links: SnapshotInstanceLinks,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) {
        std::mem::swap(&mut self.buffer, &mut self.prev_buffer);
        let instance_bytes: &[u8] = bytemuck::cast_slice(&data);
        if self.buffer.size() >= instance_bytes.len() as u64 {
            queue.write_buffer(&self.buffer, 0, instance_bytes);
        } else {
            self.buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Skinned instance buffer"),
                contents: instance_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
        }
        self.links = links;
    }
}

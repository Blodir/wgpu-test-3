use glam::{Mat3, Mat4};
use wgpu::util::DeviceExt as _;

use crate::main::wgpu_context::WgpuContext;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StaticInstance {
    pub m4: [[f32; 4]; 4],
    pub itr: [[f32; 3]; 3],
}

impl Default for StaticInstance {
    fn default() -> Self {
        Self {
            m4: Mat4::IDENTITY.to_cols_array_2d(),
            itr: Mat3::IDENTITY.to_cols_array_2d(),
        }
    }
}
impl StaticInstance {
    pub fn new(transform: Mat4) -> Self {
        let m4 = transform.to_cols_array_2d();
        let itr = Mat3::from_mat4(transform)
            .inverse()
            .transpose()
            .to_cols_array_2d();

        StaticInstance { m4, itr }
    }
}

impl StaticInstance {
    const BASE_SHADER_LOCATION: u32 = 0;
    const ATTRIBUTES: [wgpu::VertexAttribute; 7] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: Self::BASE_SHADER_LOCATION + 0,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 4]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 2,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 12]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 16]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 4,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 19]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 5,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 22]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 6,
            format: wgpu::VertexFormat::Float32x3,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<StaticInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub fn from(mat4: Mat4, itr: Mat3) -> Self {
        Self {
            m4: mat4.to_cols_array_2d(),
            itr: itr.to_cols_array_2d()
        }
    }
}

pub struct StaticInstances {
    pub buffer: wgpu::Buffer,
}
impl StaticInstances {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let instance_buffer = wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance buffer"),
                contents: bytemuck::cast_slice(&vec![Mat4::IDENTITY]),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        Self {
            buffer: instance_buffer,
        }
    }

    pub fn update(&mut self, data: Vec<StaticInstance>, queue: &wgpu::Queue, device: &wgpu::Device) {
        let instance_bytes: &[u8] = bytemuck::cast_slice(&data);
        if self.buffer.size() >= instance_bytes.len() as u64 {
            queue.write_buffer(&self.buffer, 0, instance_bytes);
        } else {
            self.buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Static instance buffer"),
                contents: instance_bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        }
    }
}

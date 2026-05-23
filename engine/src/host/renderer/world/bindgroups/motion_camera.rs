use glam::Mat4;
use wgpu::util::DeviceExt as _;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MotionCameraUniform {
    curr_view_proj: [[f32; 4]; 4],
    curr_inverse_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
}

pub struct MotionCameraBinding {
    buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl MotionCameraBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
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
            label: Some("Motion Camera Bind Group Layout"),
        }
    }

    pub fn new(device: &wgpu::Device, bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = MotionCameraUniform {
            curr_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            curr_inverse_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            prev_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        };
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Motion Camera Buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Motion Camera Bind Group"),
        });
        Self { buffer, bind_group }
    }

    pub fn update(
        &self,
        curr_view_proj: &Mat4,
        curr_inverse_view_proj: &Mat4,
        prev_view_proj: &Mat4,
        queue: &wgpu::Queue,
    ) {
        let uniform = MotionCameraUniform {
            curr_view_proj: curr_view_proj.to_cols_array_2d(),
            curr_inverse_view_proj: curr_inverse_view_proj.to_cols_array_2d(),
            prev_view_proj: prev_view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));
    }
}

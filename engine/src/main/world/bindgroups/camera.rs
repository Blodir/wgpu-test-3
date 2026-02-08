use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt as _;

pub struct CameraMatrices {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub inverse_view_proj_rot: [[f32; 4]; 4],
}

pub struct CameraBinding {
    view_proj_buffer: wgpu::Buffer,
    position_buffer: wgpu::Buffer,
    inverse_view_proj_rot_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl CameraBinding {
    pub fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let view_proj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let position_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Position Buffer"),
            contents: bytemuck::cast_slice(&Vec3::ZERO.to_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let inverse_view_proj_rot_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Inverse View Projection Buffer"),
                contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_proj_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: position_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inverse_view_proj_rot_buffer.as_entire_binding(),
                },
            ],
            label: Some("Camera Bind Group"),
        });

        CameraBinding {
            bind_group,
            view_proj_buffer,
            position_buffer,
            inverse_view_proj_rot_buffer,
        }
    }

    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Camera Bind Group Layout"),
        }
    }

    pub fn update(
        &self,
        view_proj: &[f32; 16],
        position: &[f32; 3],
        inverse_view_proj_rot: &[f32; 16],
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            &self.view_proj_buffer,
            0,
            bytemuck::cast_slice(view_proj),
        );
        queue.write_buffer(
            &self.position_buffer,
            0,
            bytemuck::cast_slice(position),
        );
        queue.write_buffer(
            &self.inverse_view_proj_rot_buffer,
            0,
            bytemuck::cast_slice(inverse_view_proj_rot),
        );
    }
}

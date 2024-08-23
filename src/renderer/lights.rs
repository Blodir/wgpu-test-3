use cgmath::{InnerSpace, Vector3};
use wgpu::util::DeviceExt;

pub struct Lights {
    direction: [f32; 3],
    color: [f32; 3],
}

pub struct LightsBinding {
    pub bind_group: wgpu::BindGroup,
    direction_buffer: wgpu::Buffer,
    color_buffer: wgpu::Buffer,
}

impl Default for Lights {
    fn default() -> Self {
        Lights {
            direction: Vector3::new(1.0, -1.0, -1.0).normalize().into(),
            color: [1.0, 1.0, 1.0],
        }
    }
}

impl Lights {
    pub fn upload(&self, device: &wgpu::Device, bind_group_layout: &wgpu::BindGroupLayout) -> LightsBinding {
        let direction_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Lights Direction Buffer"),
                contents: bytemuck::cast_slice(&self.direction),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let color_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Lights Color Buffer"),
                contents: bytemuck::cast_slice(&self.color),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: direction_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_buffer.as_entire_binding(),
                },
            ],
            label: Some("Lights Bind Group"),
        });

        LightsBinding { bind_group, direction_buffer, color_buffer }
    }

    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
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
            ],
            label: Some("Lights Bind Group Layout"),
        }
    }
}


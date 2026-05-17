use wgpu::util::DeviceExt as _;

use crate::host::renderer::HbgiOptions;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HbgiSettingsUniform {
    params0: [f32; 4],
    params1: [u32; 4],
}

pub struct HbgiSettings {}
pub struct HbgiSettingsBinding {
    pub bind_group: wgpu::BindGroup,
    _settings_buffer: wgpu::Buffer,
}

impl HbgiSettings {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("HBGI Settings Bind Group Layout"),
        }
    }

    pub fn upload(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        hbgi_options: &HbgiOptions,
        frame_index: u32,
    ) -> HbgiSettingsBinding {
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HBGI Settings Buffer"),
            contents: bytemuck::bytes_of(&HbgiSettingsUniform {
                params0: [
                    hbgi_options.radius_pixels,
                    hbgi_options.radius_world,
                    hbgi_options.step_size_exponent,
                    hbgi_options.gi_intensity,
                ],
                params1: [frame_index, 0, 0, 0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: settings_buffer.as_entire_binding(),
            }],
            label: Some("HBGI Settings Bind Group"),
        });

        HbgiSettingsBinding {
            bind_group,
            _settings_buffer: settings_buffer,
        }
    }
}

use wgpu::util::DeviceExt as _;

use crate::host::renderer::GtaoOptions;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GtaoSettingsUniform {
    params0: [f32; 4],
}

pub struct GtaoSettings {}
pub struct GtaoSettingsBinding {
    pub bind_group: wgpu::BindGroup,
    _settings_buffer: wgpu::Buffer,
}

impl GtaoSettings {
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
            label: Some("GTAO Settings Bind Group Layout"),
        }
    }

    pub fn upload(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        gtao_options: &GtaoOptions,
    ) -> GtaoSettingsBinding {
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("GTAO Settings Buffer"),
            contents: bytemuck::bytes_of(&GtaoSettingsUniform {
                params0: [
                    gtao_options.radius_pixels,
                    gtao_options.ao_radius,
                    gtao_options.power,
                    0.0,
                ],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: settings_buffer.as_entire_binding(),
            }],
            label: Some("GTAO Settings Bind Group"),
        });

        GtaoSettingsBinding {
            bind_group,
            _settings_buffer: settings_buffer,
        }
    }
}

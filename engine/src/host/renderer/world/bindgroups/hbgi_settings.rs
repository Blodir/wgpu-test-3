use wgpu::util::DeviceExt as _;

use crate::host::renderer::HbgiOptions;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HbgiSettingsUniform {
    params0: [f32; 4],
    params1: [f32; 4],
}

const TEMPORAL_ROTATIONS: [f32; 6] = [60.0, 300.0, 180.0, 240.0, 120.0, 0.0];
const SPATIAL_OFFSETS: [f32; 4] = [0.0, 0.5, 0.25, 0.75];

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
        let temporal_direction =
            TEMPORAL_ROTATIONS[(frame_index % TEMPORAL_ROTATIONS.len() as u32) as usize] / 360.0;
        let temporal_offset = SPATIAL_OFFSETS[((frame_index / TEMPORAL_ROTATIONS.len() as u32)
            % SPATIAL_OFFSETS.len() as u32) as usize];
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HBGI Settings Buffer"),
            contents: bytemuck::bytes_of(&HbgiSettingsUniform {
                params0: [
                    hbgi_options.radius_pixels,
                    hbgi_options.radius_world,
                    hbgi_options.step_size_exponent,
                    hbgi_options.gi_intensity,
                ],
                params1: [temporal_direction, temporal_offset, 0.0, 0.0],
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

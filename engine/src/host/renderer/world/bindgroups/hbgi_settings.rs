use wgpu::util::DeviceExt as _;

use crate::host::renderer::HbgiOptions;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HbgiSettingsUniform {
    pub params0: [f32; 4],
    pub params1: [f32; 4],
}
impl Default for HbgiSettingsUniform {
    fn default() -> Self {
        Self::from_options(&HbgiOptions::default(), 0)
    }
}
impl HbgiSettingsUniform {
    pub fn from_options(hbgi_options: &HbgiOptions, frame_index: u32) -> Self {
        let (temporal_rotation, temporal_offset) = r2_quasirandom_sequence(frame_index);
        Self {
            params0: [
                hbgi_options.radius_pixels,
                hbgi_options.radius_world,
                hbgi_options.step_size_exponent,
                hbgi_options.gi_intensity,
            ],
            params1: [temporal_rotation, temporal_offset, 0.0, 0.0],
        }
    }
}

const TEMPORAL_ROTATIONS: [f32; 6] = [60.0, 300.0, 180.0, 240.0, 120.0, 0.0];
const SPATIAL_OFFSETS: [f32; 4] = [0.0, 0.5, 0.25, 0.75];

// https://extremelearning.com.au/unreasonable-effectiveness-of-quasirandom-sequences
fn r2_quasirandom_sequence(n: u32) -> (f32, f32) {
    let g = 1.6180339887498948482;
    let a1 = 1.0 / g;
    let a2 = 1.0 / (g * g);
    ((0.5 + a1 * n as f32) % 1.0, (0.5 + a2 * n as f32) % 1.0)
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
        // https://extremelearning.com.au/unreasonable-effectiveness-of-quasirandom-sequences
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HBGI Settings Buffer"),
            contents: bytemuck::bytes_of(&HbgiSettingsUniform::from_options(
                hbgi_options,
                frame_index,
            )),
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

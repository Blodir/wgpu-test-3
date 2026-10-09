use crate::host::renderer::HbgiSettings;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsrtSettingsUniform {
    pub params0: [f32; 4],
    pub params1: [f32; 4],
}
impl SsrtSettingsUniform {
    pub fn from_settings(settings: &HbgiSettings, frame_index: u32) -> Self {
        let (rotation, offset) = r2_quasirandom_sequence(frame_index);
        Self {
            params0: [
                settings.radius_pixels,
                settings.radius_world,
                settings.step_size_exponent,
                settings.gi_intensity,
            ],
            params1: [rotation, offset, 0.0, 0.0],
        }
    }
}

// https://extremelearning.com.au/unreasonable-effectiveness-of-quasirandom-sequences
pub(crate) fn r2_quasirandom_sequence(frame_index: u32) -> (f32, f32) {
    let golden_ratio = 1.6180339887498948482;
    let first_increment = 1.0 / golden_ratio;
    let second_increment = 1.0 / (golden_ratio * golden_ratio);
    (
        (0.5 + first_increment * frame_index as f32) % 1.0,
        (0.5 + second_increment * frame_index as f32) % 1.0,
    )
}
pub(crate) struct SsrtSettingsBindGroup;
impl SsrtSettingsBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("SSRT Settings Layout"),
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
        }
    }
}

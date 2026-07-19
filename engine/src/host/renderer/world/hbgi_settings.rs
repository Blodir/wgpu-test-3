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

// https://extremelearning.com.au/unreasonable-effectiveness-of-quasirandom-sequences
fn r2_quasirandom_sequence(n: u32) -> (f32, f32) {
    let g = 1.6180339887498948482;
    let a1 = 1.0 / g;
    let a2 = 1.0 / (g * g);
    ((0.5 + a1 * n as f32) % 1.0, (0.5 + a2 * n as f32) % 1.0)
}

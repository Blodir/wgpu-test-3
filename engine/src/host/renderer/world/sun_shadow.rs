use bytemuck::{Pod, Zeroable};

pub const SUN_SHADOW_MAX_CASCADE_COUNT: usize = 4;
pub const SUN_SHADOW_DEFAULT_CASCADE_COUNT: usize = 4;
pub const SUN_SHADOW_MAP_SIZE: u32 = 2048;
pub const SUN_SHADOW_MAX_DISTANCE: f32 = 300.0;
pub const SUN_SHADOW_DEFAULT_CASCADE_SPLIT_RATIOS: [f32; SUN_SHADOW_MAX_CASCADE_COUNT] =
    [0.08, 0.2, 0.45, 1.0];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SunShadowUniform {
    pub light_view_proj: [[f32; 16]; SUN_SHADOW_MAX_CASCADE_COUNT],
    pub split_depths: [f32; 4],
    pub cascade_params: [u32; 4],
}

impl Default for SunShadowUniform {
    fn default() -> Self {
        Self {
            light_view_proj: [glam::Mat4::IDENTITY.to_cols_array(); SUN_SHADOW_MAX_CASCADE_COUNT],
            split_depths: [0.0; 4],
            cascade_params: [SUN_SHADOW_DEFAULT_CASCADE_COUNT as u32, 0, 0, 0],
        }
    }
}

#[derive(Clone, Copy)]
pub struct PreparedSunShadowCascade {
    pub light_view_proj: [f32; 16],
    pub split_depth: f32,
}

#[derive(Clone, Copy)]
pub struct PreparedSunShadow {
    pub cascades: [PreparedSunShadowCascade; SUN_SHADOW_MAX_CASCADE_COUNT],
    pub cascade_count: usize,
}

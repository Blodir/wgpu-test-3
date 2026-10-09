mod depth_pyramid;
mod g_buffer;
mod ssrt;
mod sun_shadow;

pub(crate) use depth_pyramid::DepthPyramidTextures;
pub(crate) use g_buffer::GBufferTextures;
pub(crate) use ssrt::{SsrtTextures, SSRT_A_TROUS_PASS_COUNT};
pub(crate) use sun_shadow::SunShadowTexture2;

use crate::host::wgpu_context::WgpuContext;

pub(crate) struct Textures {
    pub(crate) gbuffer: GBufferTextures,
    pub(crate) depth_pyramid: DepthPyramidTextures,
    pub(crate) ssrt: SsrtTextures,
    pub(crate) sun_shadow: SunShadowTexture2,
}

impl Textures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        Self {
            gbuffer: GBufferTextures::new(wgpu_context, surface_config),
            depth_pyramid: DepthPyramidTextures::new(wgpu_context, surface_config),
            ssrt: SsrtTextures::new(wgpu_context, surface_config),
            sun_shadow: SunShadowTexture2::new(wgpu_context),
        }
    }
}

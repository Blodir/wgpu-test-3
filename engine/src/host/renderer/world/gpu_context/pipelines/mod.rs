mod g_buffer;
mod skinned_transparent;
pub(crate) mod ssrt;
mod static_transparent;
mod sun_shadow;

pub(crate) use g_buffer::GBufferPipeline;
pub(crate) use skinned_transparent::SkinnedTransparentPipeline;
pub(crate) use ssrt::{FullscreenPipeline, SsrtPipelines};
pub(crate) use static_transparent::StaticTransparentPipeline;
pub(crate) use sun_shadow::SunShadowPipeline;

use crate::host::{
    renderer::world::gpu_context::BGLayouts, shader_cache::ShaderCache, wgpu_context::WgpuContext,
};

pub(crate) struct Pipelines {
    pub(crate) g_buffer: GBufferPipeline,
    pub(crate) ssrt: SsrtPipelines,
    pub(crate) sun_shadow: SunShadowPipeline,
    pub(crate) skinned_transparent: SkinnedTransparentPipeline,
    pub(crate) static_transparent: StaticTransparentPipeline,
}
impl Pipelines {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        Self {
            g_buffer: GBufferPipeline::new(wgpu_context, shader_cache, layouts),
            ssrt: SsrtPipelines::new(wgpu_context, shader_cache, layouts),
            sun_shadow: SunShadowPipeline::new(wgpu_context, shader_cache, layouts),
            skinned_transparent: SkinnedTransparentPipeline::new(
                wgpu_context,
                shader_cache,
                layouts,
            ),
            static_transparent: StaticTransparentPipeline::new(wgpu_context, shader_cache, layouts),
        }
    }
}

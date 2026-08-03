mod deferred_lighting;
mod g_buffer;
mod hbgi;
mod hbgi_history_fix;
mod hbgi_pyramid;
mod hbgi_reproject;
mod hbgi_svgf;
mod post_processing;
mod skinned_transparent;
mod skybox;
mod static_transparent;
mod sun_shadow;

pub(crate) use deferred_lighting::DeferredLightingPipeline;
pub(crate) use g_buffer::GBufferPipeline;
pub(crate) use hbgi::HbgiPipeline;
pub(crate) use hbgi_history_fix::HbgiHistoryFixPipeline;
pub(crate) use hbgi_pyramid::HbgiPyramidPipelines;
pub(crate) use hbgi_reproject::HbgiReprojectPipeline;
pub(crate) use hbgi_svgf::HbgiSvgfPipeline;
pub(crate) use post_processing::PostProcessingPipeline;
pub(crate) use skinned_transparent::SkinnedTransparentPipeline;
pub(crate) use skybox::SkyboxPipeline;
pub(crate) use static_transparent::StaticTransparentPipeline;
pub(crate) use sun_shadow::SunShadowPipeline;

use crate::host::{
    renderer::world::gpu_context::BGLayouts, shader_cache::ShaderCache, wgpu_context::WgpuContext,
};

pub(crate) struct Pipelines {
    pub(crate) g_buffer: GBufferPipeline,
    pub(crate) hbgi: HbgiPipeline,
    pub(crate) hbgi_pyramid: HbgiPyramidPipelines,
    pub(crate) hbgi_history_fix: HbgiHistoryFixPipeline,
    pub(crate) hbgi_svgf: HbgiSvgfPipeline,
    pub(crate) deferred_lighting: DeferredLightingPipeline,
    pub(crate) hbgi_reproject: HbgiReprojectPipeline,
    pub(crate) skybox: SkyboxPipeline,
    pub(crate) sun_shadow: SunShadowPipeline,
    pub(crate) skinned_transparent: SkinnedTransparentPipeline,
    pub(crate) static_transparent: StaticTransparentPipeline,
    pub(crate) post: PostProcessingPipeline,
}

impl Pipelines {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let g_buffer = GBufferPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi = HbgiPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi_pyramid = HbgiPyramidPipelines::new(wgpu_context, shader_cache, layouts);
        let hbgi_history_fix = HbgiHistoryFixPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi_svgf = HbgiSvgfPipeline::new(wgpu_context, shader_cache, layouts);
        let deferred_lighting = DeferredLightingPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi_reproject = HbgiReprojectPipeline::new(wgpu_context, shader_cache, layouts);
        let skybox = SkyboxPipeline::new(wgpu_context, shader_cache, layouts);
        let sun_shadow = SunShadowPipeline::new(wgpu_context, shader_cache, layouts);
        let skinned_transparent =
            SkinnedTransparentPipeline::new(wgpu_context, shader_cache, layouts);
        let static_transparent =
            StaticTransparentPipeline::new(wgpu_context, shader_cache, layouts);
        let post = PostProcessingPipeline::new(wgpu_context, shader_cache, layouts);

        Self {
            g_buffer,
            hbgi,
            hbgi_pyramid,
            hbgi_history_fix,
            hbgi_svgf,
            deferred_lighting,
            hbgi_reproject,
            skybox,
            sun_shadow,
            skinned_transparent,
            static_transparent,
            post,
        }
    }
}

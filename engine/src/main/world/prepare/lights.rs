use crate::snapshot_handoff::SnapshotGuard;
use crate::{main::{world::bindgroups::lights::LightsBinding, sampler_cache::SamplerCache, wgpu_context::WgpuContext}, main::assets::{io::asset_formats::materialfile, store::RenderAssetStore}};

pub fn prepare_lights(
    snaps: &SnapshotGuard,
    lights_binding: &mut LightsBinding,
    render_resources: &RenderAssetStore,
    sampler_cache: &mut SamplerCache,
    wgpu_context: &WgpuContext,
    bind_group_layout: &wgpu::BindGroupLayout,
) {
    lights_binding.update_sun(&snaps.curr.lights.sun, &wgpu_context.queue);

    if let Some(e) = &snaps.curr.lights.environment_map {
        // if one of env maps has changed, we must rebuild the bindgroup entirely
        if e.prefiltered != lights_binding.curr_prefiltered_render_id || e.di != lights_binding.curr_di_render_id || e.brdf != lights_binding.curr_brdf_render_id {
            let gpu_textures = &render_resources.textures;
            let (prefiltered, di, brdf) = (gpu_textures.get(e.prefiltered.into()).unwrap(), gpu_textures.get(e.di.into()).unwrap(), gpu_textures.get(e.brdf.into()).unwrap());
            let default_sampler = sampler_cache.get(&materialfile::Sampler::default(), wgpu_context);
            lights_binding.update_environment_map(
                wgpu_context, bind_group_layout,
                &prefiltered.texture_view, &default_sampler,
                &di.texture_view, &default_sampler,
                &brdf.texture_view, &default_sampler
            );
            lights_binding.curr_prefiltered_render_id = e.prefiltered;
            lights_binding.curr_di_render_id = e.di;
            lights_binding.curr_brdf_render_id = e.brdf;
        }
    }
}

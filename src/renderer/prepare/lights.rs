use crate::{render_snapshot::SnapshotGuard, renderer::{bindgroups::lights::LightsBinding, sampler_cache::SamplerCache, wgpu_context::WgpuContext}, resource_manager::{file_formats::materialfile, registry::GpuState, resource_manager::ResourceManager}};

pub fn prepare_lights(
    snaps: &SnapshotGuard,
    lights_binding: &mut LightsBinding,
    resource_manager: &ResourceManager,
    sampler_cache: &mut SamplerCache,
    wgpu_context: &WgpuContext,
    bind_group_layout: &wgpu::BindGroupLayout,
) {
    lights_binding.update_sun(&snaps.curr.environment.sun, &wgpu_context.queue);

    let e = &snaps.curr.environment;
    let reg = resource_manager.registry.lock().unwrap();
    if let (Some(p_entry), Some(d_entry), Some(b_entry)) = (reg.get_id(&e.prefiltered), reg.get_id(&e.di), reg.get_id(&e.brdf)) {
        if let (GpuState::Ready(p_gpu_idx), GpuState::Ready(d_gpu_idx), GpuState::Ready(b_gpu_idx)) = (&p_entry.gpu_state, &d_entry.gpu_state, &b_entry.gpu_state) {
            // if one of env maps has changed, we must rebuild the bindgroup entirely
            if *p_gpu_idx != lights_binding.curr_prefiltered_gpu_id || *d_gpu_idx != lights_binding.curr_prefiltered_gpu_id || *b_gpu_idx != lights_binding.curr_brdf_gpu_id {
                let gpu_textures = resource_manager.gpu.textures.lock().unwrap();
                let (prefiltered, di, brdf) = (gpu_textures.get(*p_gpu_idx).unwrap(), gpu_textures.get(*d_gpu_idx).unwrap(), gpu_textures.get(*b_gpu_idx).unwrap());
                let default_sampler = sampler_cache.get(&materialfile::Sampler::default(), wgpu_context);
                lights_binding.update_environment_map(
                    wgpu_context, bind_group_layout,
                    &prefiltered.texture_view, &default_sampler,
                    &di.texture_view, &default_sampler,
                    &brdf.texture_view, &default_sampler
                );
                lights_binding.curr_prefiltered_gpu_id = *p_gpu_idx;
                lights_binding.curr_di_gpu_id = *d_gpu_idx;
                lights_binding.curr_brdf_gpu_id = *b_gpu_idx;
            }
        }
    } else {
        println!("Warning: stale handle id when updating environment map");
    }
}

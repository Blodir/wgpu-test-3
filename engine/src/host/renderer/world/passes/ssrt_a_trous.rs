use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::{
    resources::textures::SSRT_A_TROUS_PASS_COUNT, WorldGpuContext,
};

pub(crate) fn render_ssrt_a_trous_passes(
    encoder: &mut wgpu::CommandEncoder,
    context: &mut WorldGpuContext,
) {
    for pass_index in 0..SSRT_A_TROUS_PASS_COUNT {
        let textures = &context.resources.textures.ssrt;
        let views = &context.descriptors.texture_views.ssrt;
        let groups = &context.descriptors.bind_groups.ssrt;
        let current = textures.ssgi_denoise.get_write_idx() as usize;
        let history = textures.normal_roughness_history.get_write_idx() as usize;
        render_fullscreen_pass(
            encoder,
            "SSRT A-Trous Pass",
            &context.pipelines.ssrt.a_trous.pipeline,
            &groups.history[history].a_trous[current],
            &[
                &groups.a_trous_settings[pass_index],
                &context.descriptors.bind_groups.camera.0,
            ],
            [
                views.ssgi_denoise.get_write_view(&textures.ssgi_denoise),
                views
                    .bent_normal_ao_denoise
                    .get_write_view(&textures.bent_normal_ao_denoise),
            ],
            context,
        );
        context.resources.textures.ssrt.ssgi_denoise.swap();
        context
            .resources
            .textures
            .ssrt
            .bent_normal_ao_denoise
            .swap();
    }
}

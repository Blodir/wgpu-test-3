use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_ssrt_history_fix_pass(
    encoder: &mut wgpu::CommandEncoder,
    context: &mut WorldGpuContext,
) {
    let textures = &mut context.resources.textures.ssrt;
    if textures.ssgi_denoise.get_write_idx() != 0 {
        textures.ssgi_denoise.swap();
    }
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    render_fullscreen_pass(
        encoder,
        "SSRT History Fix Pass",
        &context.pipelines.ssrt.history_fix.pipeline,
        &groups.history[current].history_fix,
        &[&context.descriptors.bind_groups.camera.0],
        [
            views.ssgi_denoise.get_write_view(&textures.ssgi_denoise),
            &views.ssr_denoise,
        ],
        context,
    );
    context.resources.textures.ssrt.ssgi_denoise.swap();
}

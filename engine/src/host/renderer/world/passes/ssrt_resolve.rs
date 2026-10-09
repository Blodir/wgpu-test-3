use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_ssrt_resolve_pass(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    render_fullscreen_pass(
        encoder,
        "SSRT Resolve Pass",
        &context.pipelines.ssrt.resolve.pipeline,
        &groups.history[current].resolve,
        &[&context.descriptors.bind_groups.camera.0],
        [
            views.ssgi_history.get_write_view(&textures.ssgi_history),
            views.ssr_history.get_write_view(&textures.ssr_history),
            views
                .moments_history
                .get_write_view(&textures.moments_history),
        ],
        context,
    );
}

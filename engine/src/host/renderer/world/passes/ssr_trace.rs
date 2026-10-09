use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_ssr_trace_pass(encoder: &mut wgpu::CommandEncoder, context: &WorldGpuContext) {
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    render_fullscreen_pass(
        encoder,
        "SSR Trace Pass",
        &context.pipelines.ssrt.ssr_trace.pipeline,
        &groups.history[current].ssr_trace,
        &[
            &groups.settings,
            &context.descriptors.bind_groups.camera.0,
            &context.descriptors.bind_groups.lights.0,
        ],
        [&views.ssr_trace],
        context,
    );
}

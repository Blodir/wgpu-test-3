use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_lighting_composite_pass(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let textures = &context.resources.textures.ssrt;
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    render_fullscreen_pass(
        encoder,
        "Lighting Composite Pass",
        &context.pipelines.ssrt.lighting_composite.pipeline,
        &groups.lighting_composite,
        &[
            &context.descriptors.bind_groups.camera.0,
            &context.descriptors.bind_groups.lights.0,
        ],
        [
            &views.final_hdr,
            views
                .diffuse_history
                .get_write_view(&textures.diffuse_history),
        ],
        context,
    );
}

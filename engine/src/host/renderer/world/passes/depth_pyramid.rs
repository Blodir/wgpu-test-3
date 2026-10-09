use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_depth_pyramid_passes(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let views = &context.descriptors.texture_views.depth_pyramid;
    let groups = &context.descriptors.bind_groups.ssrt;
    render_fullscreen_pass(
        encoder,
        "Depth Pyramid Base Pass",
        &context.pipelines.ssrt.depth_base.pipeline,
        &groups.depth_base,
        &[],
        [&views.mips[0]],
        context,
    );
    for (index, group) in groups.depth_downsample.iter().enumerate() {
        render_fullscreen_pass(
            encoder,
            "Depth Pyramid Downsample Pass",
            &context.pipelines.ssrt.depth_downsample.pipeline,
            group,
            &[],
            [&views.mips[index + 1]],
            context,
        );
    }
}

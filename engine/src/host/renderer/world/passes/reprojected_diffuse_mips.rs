use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_reprojected_diffuse_mip_passes(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    for (index, group) in groups.reprojected_diffuse_downsample.iter().enumerate() {
        render_fullscreen_pass(
            encoder,
            "Reprojected Diffuse Downsample Pass",
            &context
                .pipelines
                .ssrt
                .reprojected_diffuse_downsample
                .pipeline,
            group,
            &[],
            [&views.reprojected_diffuse_mips[index + 1]],
            context,
        );
    }
}

use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_diffuse_normal_reprojects_pass(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    render_fullscreen_pass(
        encoder,
        "Diffuse Normal Reprojects Pass",
        &context.pipelines.ssrt.diffuse_normal_reprojects.pipeline,
        &groups.history[current].diffuse_normal_reprojects,
        &[&context.descriptors.bind_groups.camera.0],
        [
            views
                .normal_roughness_history
                .get_write_view(&textures.normal_roughness_history),
            &views.reprojected_diffuse_mips[0],
            &views.reprojected_bent_normal_ao,
        ],
        context,
    );
}

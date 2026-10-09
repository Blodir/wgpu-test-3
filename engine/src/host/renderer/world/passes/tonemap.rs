use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_tonemap_pass(
    encoder: &mut wgpu::CommandEncoder,
    output_texture_view: &wgpu::TextureView,
    context: &WorldGpuContext,
) {
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    render_fullscreen_pass(
        encoder,
        "Tonemap Pass",
        &context.pipelines.ssrt.tonemap.pipeline,
        &context.descriptors.bind_groups.ssrt.history[current].tonemap,
        &[],
        [output_texture_view],
        context,
    );
}

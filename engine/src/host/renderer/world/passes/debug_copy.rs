use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_debug_copy_normal_history_pass(
    encoder: &mut wgpu::CommandEncoder,
    output_texture_view: &wgpu::TextureView,
    context: &WorldGpuContext,
) {
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    render_fullscreen_pass(
        encoder,
        "Debug Copy Normal History Pass",
        &context.pipelines.ssrt.debug_copy_normal_history.pipeline,
        &context.descriptors.bind_groups.ssrt.history[current].debug_copy_normal_history,
        &[],
        [output_texture_view],
        context,
    );
}

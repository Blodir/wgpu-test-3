use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_taa_pass(encoder: &mut wgpu::CommandEncoder, context: &WorldGpuContext) {
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    let depth_history = textures.depth_history.get_write_tex();
    for mip_level in 0..depth_history.mip_level_count() {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &context.resources.textures.depth_pyramid.texture,
                mip_level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: depth_history,
                mip_level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: (depth_history.width() >> mip_level).max(1),
                height: (depth_history.height() >> mip_level).max(1),
                depth_or_array_layers: 1,
            },
        );
    }
    render_fullscreen_pass(
        encoder,
        "TAA Pass",
        &context.pipelines.ssrt.taa.pipeline,
        &groups.history[current].taa,
        &[&context.descriptors.bind_groups.camera.0],
        [views
            .final_hdr_history
            .get_write_view(&textures.final_hdr_history)],
        context,
    );
}

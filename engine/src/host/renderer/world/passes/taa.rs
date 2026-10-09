use super::fullscreen::render_fullscreen_pass;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(crate) fn render_taa_pass(encoder: &mut wgpu::CommandEncoder, context: &WorldGpuContext) {
    let textures = &context.resources.textures.ssrt;
    let current = textures.normal_roughness_history.get_write_idx() as usize;
    let views = &context.descriptors.texture_views.ssrt;
    let groups = &context.descriptors.bind_groups.ssrt;
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &context.resources.textures.depth_pyramid.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: textures.depth_history.get_write_tex(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        textures.depth_history.get_write_tex().size(),
    );
    render_fullscreen_pass(
        encoder,
        "TAA Pass",
        &context.pipelines.ssrt.taa.pipeline,
        &groups.history[current].taa,
        &[],
        [views
            .final_hdr_history
            .get_write_view(&textures.final_hdr_history)],
        context,
    );
}

use crate::host::renderer::world::gpu_context::WorldGpuContext;

fn render_clear_pass(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Clear Temporal History Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
}

pub(crate) fn render_clear_temporal_inputs_passes(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let textures = &context.resources.textures.ssrt;
    let views = &context.descriptors.texture_views.ssrt;
    for (history, texture) in [
        (
            &views.normal_roughness_history,
            &textures.normal_roughness_history,
        ),
        (&views.diffuse_history, &textures.diffuse_history),
        (&views.ssgi_history, &textures.ssgi_history),
        (
            &views.bent_normal_ao_history,
            &textures.bent_normal_ao_history,
        ),
        (
            &views.bent_normal_ao_denoise,
            &textures.bent_normal_ao_denoise,
        ),
        (&views.ssr_history, &textures.ssr_history),
        (&views.moments_history, &textures.moments_history),
        (&views.final_hdr_history, &textures.final_hdr_history),
    ] {
        render_clear_pass(encoder, history.get_read_view(texture));
        render_clear_pass(encoder, history.get_write_view(texture));
    }
    for mip in &views.depth_history_mips {
        render_clear_pass(encoder, mip.get_read_view(&textures.depth_history));
        render_clear_pass(encoder, mip.get_write_view(&textures.depth_history));
    }
}

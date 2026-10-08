use crate::host::renderer::world::gpu_context::WorldGpuContext;

use super::FULLSCREEN_QUAD_INDEX_COUNT;

fn render_mipmap_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    dst_view: &wgpu::TextureView,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: dst_view,
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

    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, bind_group, &[]);
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

pub(crate) fn render_gi_source_mips(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let texture_views = &context.descriptors.texture_views.lighting_target;
    let bind_groups = &context.descriptors.bind_groups.mipmap;

    for (idx, bind_group) in bind_groups.downsample.iter().enumerate() {
        let mip_level = idx + 1;
        render_mipmap_pass(
            encoder,
            "GI Source Mipmap Pass",
            &context.pipelines.mipmap.render_pipeline,
            bind_group,
            &texture_views.diffuse_radiance_ao_mips[mip_level],
            context,
        );
    }
}

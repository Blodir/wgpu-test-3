use crate::host::renderer::world::gpu_context::WorldGpuContext;

use super::FULLSCREEN_QUAD_INDEX_COUNT;

pub(crate) fn render_post_processing_pass(
    encoder: &mut wgpu::CommandEncoder,
    output_texture_view: &wgpu::TextureView,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Post Processing Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: output_texture_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&context.pipelines.post.render_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.post_processing.0, &[]);
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

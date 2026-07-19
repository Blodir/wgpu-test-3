use crate::host::renderer::world::gpu_context::WorldGpuContext;

use super::FULLSCREEN_QUAD_INDEX_COUNT;

pub(crate) fn render_skybox_pass(encoder: &mut wgpu::CommandEncoder, context: &WorldGpuContext) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Skybox Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &context.descriptors.texture_views.sky,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&context.pipelines.skybox.render_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.lights.0, &[]);
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

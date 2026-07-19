use crate::host::renderer::world::gpu_context::WorldGpuContext;

use super::FULLSCREEN_QUAD_INDEX_COUNT;

pub(crate) fn render_gi_blur_pass(encoder: &mut wgpu::CommandEncoder, context: &WorldGpuContext) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("GI Blur Pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gi_blur.bent_ao,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context
                    .descriptors
                    .texture_views
                    .gi_blur
                    .near_field_irradiance,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
        ],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&context.pipelines.gi_blur.render_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.gi_blur.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

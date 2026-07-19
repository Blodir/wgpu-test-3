use crate::host::renderer::world::gpu_context::WorldGpuContext;

use super::FULLSCREEN_QUAD_INDEX_COUNT;

pub(crate) fn render_hbgi_pyramid_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    hbgi_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    normal_view: &wgpu::TextureView,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: hbgi_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: depth_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: normal_view,
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

    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, bind_group, &[]);
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

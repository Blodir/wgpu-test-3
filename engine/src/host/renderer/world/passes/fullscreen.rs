use super::FULLSCREEN_QUAD_INDEX_COUNT;
use crate::host::renderer::world::gpu_context::WorldGpuContext;

pub(super) fn render_fullscreen_pass<const ATTACHMENT_COUNT: usize>(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    pipeline: &wgpu::RenderPipeline,
    inputs: &wgpu::BindGroup,
    additional_groups: &[&wgpu::BindGroup],
    outputs: [&wgpu::TextureView; ATTACHMENT_COUNT],
    context: &WorldGpuContext,
) {
    let color_attachments = outputs.map(|view| {
        Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, inputs, &[]);
    for (index, group) in additional_groups.iter().enumerate() {
        render_pass.set_bind_group(index as u32 + 1, *group, &[]);
    }
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

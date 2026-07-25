use crate::host::{renderer::world::gpu_context::WorldGpuContext, wgpu_context::WgpuContext};

use super::FULLSCREEN_QUAD_INDEX_COUNT;

const HBGI_SVGF_PASS_COUNT: u32 = 5;

pub(crate) fn render_hbgi_svgf_pass(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
    wgpu_context: &WgpuContext,
) {
    for pass_idx in 0..HBGI_SVGF_PASS_COUNT {
        context
            .resources
            .buffers
            .update_hbgi_svgf_settings(pass_idx, &wgpu_context.queue);

        let (input_bind_group, output_bent_ao, output_irradiance_variance) = match pass_idx {
            0 => (
                &context.descriptors.bind_groups.hbgi_svgf.initial.0,
                &context.descriptors.texture_views.hbgi_svgf.bent_ao_a,
                &context.descriptors.texture_views.hbgi_svgf.irradiance_variance_a,
            ),
            _ if pass_idx % 2 == 1 => (
                &context.descriptors.bind_groups.hbgi_svgf.history_a.0,
                &context.descriptors.texture_views.hbgi_svgf.bent_ao_b,
                &context.descriptors.texture_views.hbgi_svgf.irradiance_variance_b,
            ),
            _ => (
                &context.descriptors.bind_groups.hbgi_svgf.history_b.0,
                &context.descriptors.texture_views.hbgi_svgf.bent_ao_a,
                &context.descriptors.texture_views.hbgi_svgf.irradiance_variance_a,
            ),
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("HBGI SVGF Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: output_bent_ao,
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
                    view: output_irradiance_variance,
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

        render_pass.set_pipeline(&context.pipelines.hbgi_svgf.render_pipeline);
        render_pass.set_bind_group(0, input_bind_group, &[]);
        render_pass.set_bind_group(1, &context.descriptors.bind_groups.camera.0, &[]);
        render_pass.set_bind_group(2, &context.descriptors.bind_groups.hbgi_svgf.settings.0, &[]);
        render_pass.set_index_buffer(
            context.resources.buffers.fullscreen_quad_indices.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
    }
}

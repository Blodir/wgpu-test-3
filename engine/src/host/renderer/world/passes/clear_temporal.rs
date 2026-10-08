use crate::host::renderer::world::gpu_context::WorldGpuContext;

fn render_clear_pass<const ATTACHMENT_COUNT: usize>(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    color_attachments: [(&wgpu::TextureView, wgpu::Color); ATTACHMENT_COUNT],
) {
    let color_attachments = color_attachments.map(|(view, clear_color)| {
        Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear_color),
                store: wgpu::StoreOp::Store,
            },
        })
    });
    let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
}

pub(crate) fn render_clear_temporal_inputs_passes(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    for view in &context
        .descriptors
        .texture_views
        .lighting_target
        .diffuse_radiance_ao_mips
    {
        render_clear_pass(
            encoder,
            "Lighting Target > Diffuse Radiance AO - Clear Pass",
            [(view, wgpu::Color::TRANSPARENT)],
        );
    }

    for (label, view, clear_color) in [
        (
            "Reproject > Bent AO - Clear Pass",
            context
                .descriptors
                .texture_views
                .reproject
                .bent_ao
                .get_read_view(&context.resources.textures.reproject.bent_ao),
            wgpu::Color::TRANSPARENT,
        ),
        (
            "Reproject > Near Field Irradiance - Clear Pass",
            context
                .descriptors
                .texture_views
                .reproject
                .near_field_irradiance
                .get_read_view(
                    &context
                        .resources
                        .textures
                        .reproject
                        .near_field_irradiance,
                ),
            wgpu::Color::TRANSPARENT,
        ),
        (
            "Reproject > Depth History - Clear Pass",
            context
                .descriptors
                .texture_views
                .reproject
                .depth_history
                .get_read_view(&context.resources.textures.reproject.depth_history),
            wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        ),
        (
            "Reproject > Normal History - Clear Pass",
            context
                .descriptors
                .texture_views
                .reproject
                .normal_history
                .get_read_view(&context.resources.textures.reproject.normal_history),
            wgpu::Color::TRANSPARENT,
        ),
    ] {
        render_clear_pass(encoder, label, [(view, clear_color)]);
    }
}

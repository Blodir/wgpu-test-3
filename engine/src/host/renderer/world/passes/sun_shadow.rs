use crate::host::{
    assets::store::RenderAssetStore,
    renderer::world::{gpu_context::WorldGpuContext, prepare::mesh::PassDrawContext},
};

pub(crate) fn draw_sun_shadow_skinned<'a>(
    pass_draw: &PassDrawContext<'a>,
    render_pass: &mut wgpu::RenderPass<'a>,
    render_resources: &'a RenderAssetStore,
) {
    let models = &render_resources.models;
    let meshes = &render_resources.meshes;
    for material_batch in &pass_draw.batch.material_batches[pass_draw.batch.skinned_batch.clone()] {
        for mesh_batch in &pass_draw.batch.mesh_batches[material_batch.mesh_range.clone()] {
            let model = models.get(mesh_batch.model_id.into()).unwrap();
            let mesh = meshes.get(model.mesh_id.into()).unwrap();
            render_pass.set_index_buffer(
                mesh.buffer
                    .slice(0..model.vertex_buffer_start_offset as u64),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.set_vertex_buffer(
                0,
                mesh.buffer.slice(model.vertex_buffer_start_offset as u64..),
            );
            for draw_idx in mesh_batch.submesh_range.clone() {
                let submesh_batch = &pass_draw.batch.submesh_batches[draw_idx];
                let submesh = &model.submeshes[submesh_batch.submesh_idx];
                render_pass.draw_indexed(
                    submesh.index_range.clone(),
                    submesh.base_vertex as i32,
                    pass_draw.instance_ranges[draw_idx].clone(),
                );
            }
        }
    }
}

pub(crate) fn draw_sun_shadow_static<'a>(
    pass_draw: &PassDrawContext<'a>,
    render_pass: &mut wgpu::RenderPass<'a>,
    render_resources: &'a RenderAssetStore,
) {
    let models = &render_resources.models;
    let meshes = &render_resources.meshes;
    for material_batch in &pass_draw.batch.material_batches[pass_draw.batch.static_batch.clone()] {
        for mesh_batch in &pass_draw.batch.mesh_batches[material_batch.mesh_range.clone()] {
            let model = models.get(mesh_batch.model_id.into()).unwrap();
            let mesh = meshes.get(model.mesh_id.into()).unwrap();
            render_pass.set_index_buffer(
                mesh.buffer
                    .slice(0..model.vertex_buffer_start_offset as u64),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.set_vertex_buffer(
                0,
                mesh.buffer.slice(model.vertex_buffer_start_offset as u64..),
            );
            for draw_idx in mesh_batch.submesh_range.clone() {
                let submesh_batch = &pass_draw.batch.submesh_batches[draw_idx];
                let submesh = &model.submeshes[submesh_batch.submesh_idx];
                render_pass.draw_indexed(
                    submesh.index_range.clone(),
                    submesh.base_vertex as i32,
                    pass_draw.instance_ranges[draw_idx].clone(),
                );
            }
        }
    }
}

pub(crate) fn render_sun_shadow_pass<'a>(
    encoder: &mut wgpu::CommandEncoder,
    skinned_opaque_pass: &'a PassDrawContext<'a>,
    static_opaque_pass: &'a PassDrawContext<'a>,
    shadow_depth_view: &wgpu::TextureView,
    sun_shadow_matrix_bind_group: &wgpu::BindGroup,
    render_resources: &'a RenderAssetStore,
    context: &WorldGpuContext,
) {
    {
        let mut skinned_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sun Shadow Skinned Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: shadow_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        skinned_pass.set_pipeline(&context.pipelines.sun_shadow.skinned_pipeline);
        skinned_pass.set_bind_group(0, sun_shadow_matrix_bind_group, &[]);
        skinned_pass.set_bind_group(
            1,
            &context.descriptors.bind_groups.bones.bones_bind_group,
            &[],
        );
        draw_sun_shadow_skinned(skinned_opaque_pass, &mut skinned_pass, render_resources);
    }

    let mut static_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Sun Shadow Static Pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: shadow_depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    static_pass.set_pipeline(&context.pipelines.sun_shadow.static_pipeline);
    static_pass.set_bind_group(0, sun_shadow_matrix_bind_group, &[]);
    static_pass.set_bind_group(1, &context.descriptors.bind_groups.static_instances.0, &[]);
    draw_sun_shadow_static(static_opaque_pass, &mut static_pass, render_resources);
}

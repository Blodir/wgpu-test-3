use crate::host::{
    assets::store::RenderAssetStore,
    renderer::world::{gpu_context::WorldGpuContext, prepare::mesh::PassDrawContext},
};

pub(crate) fn draw_skinned_transparent_pass<'a>(
    pass_draw: &PassDrawContext<'a>,
    render_pass: &mut wgpu::RenderPass<'a>,
    render_resources: &'a RenderAssetStore,
) {
    let models = &render_resources.models;
    let materials = &render_resources.materials;
    let meshes = &render_resources.meshes;
    for material_batch in &pass_draw.batch.material_batches[pass_draw.batch.skinned_batch.clone()] {
        let material = materials.get(material_batch.material_id.into()).unwrap();
        render_pass.set_bind_group(2, &material.bind_group, &[]);
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

pub(crate) fn render_skinned_transparent_pass<'a>(
    encoder: &mut wgpu::CommandEncoder,
    pass_draw: &'a PassDrawContext<'a>,
    render_resources: &'a RenderAssetStore,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Skinned Transparent Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &context.descriptors.texture_views.ssr.scene_color,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &context.descriptors.texture_views.gbuffer.depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&context.pipelines.skinned_transparent.pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.lights.0, &[]);
    render_pass.set_bind_group(
        3,
        &context.descriptors.bind_groups.bones.motion_bind_group,
        &[],
    );
    draw_skinned_transparent_pass(pass_draw, &mut render_pass, render_resources);
}

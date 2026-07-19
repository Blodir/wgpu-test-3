use super::{prepare::mesh::PassDrawContext, gpu_context::WorldGpuContext};
use crate::host::assets::store::RenderAssetStore;

const FULLSCREEN_QUAD_INDEX_COUNT: u32 = 6;

pub(crate) fn render_hbgi_pass(encoder: &mut wgpu::CommandEncoder, context: &WorldGpuContext) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("HBGI Pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.hbgi.bent_ao,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.hbgi.near_field_irradiance,
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

    render_pass.set_pipeline(&context.pipelines.hbgi.render_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.hbgi.inputs.0, &[]);
    render_pass.set_bind_group(2, &context.descriptors.bind_groups.hbgi.settings.0, &[]);
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

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

pub(crate) fn render_hbgi_reproject_pass(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("HBGI Reproject Pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: context
                    .descriptors
                    .texture_views
                    .reproject
                    .bent_ao
                    .get_write_view(&context.resources.textures.reproject.bent_ao),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: -1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: context
                    .descriptors
                    .texture_views
                    .reproject
                    .depth_history
                    .get_write_view(&context.resources.textures.reproject.depth_history),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: context
                    .descriptors
                    .texture_views
                    .reproject
                    .normal_history
                    .get_write_view(&context.resources.textures.reproject.normal_history),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: context
                    .descriptors
                    .texture_views
                    .reproject
                    .near_field_irradiance
                    .get_write_view(&context.resources.textures.reproject.near_field_irradiance),
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

    render_pass.set_pipeline(&context.pipelines.hbgi_reproject.render_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(
        1,
        &context.descriptors.bind_groups.hbgi_reproject.inputs.0,
        &[],
    );
    render_pass.set_bind_group(
        2,
        &context.descriptors.bind_groups.hbgi_reproject.settings.0,
        &[],
    );
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

pub(crate) fn render_deferred_lighting_pass(
    encoder: &mut wgpu::CommandEncoder,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Deferred Lighting Pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.lighting_target.lit_hdr,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context
                    .descriptors
                    .texture_views
                    .lighting_target
                    .diffuse_radiance_ao,
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

    render_pass.set_pipeline(&context.pipelines.deferred_lighting.render_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.lights.0, &[]);
    render_pass.set_bind_group(
        2,
        &context.descriptors.bind_groups.deferred_lighting.gbuffer.0,
        &[],
    );
    render_pass.set_bind_group(
        3,
        &context.descriptors.bind_groups.deferred_lighting.hbgi.0,
        &[],
    );
    render_pass.set_index_buffer(
        context.resources.buffers.fullscreen_quad_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.draw_indexed(0..FULLSCREEN_QUAD_INDEX_COUNT, 0, 0..1);
}

pub(crate) fn draw_gbuffer_skinned<'a>(
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

pub(crate) fn draw_gbuffer_static<'a>(
    pass_draw: &PassDrawContext<'a>,
    render_pass: &mut wgpu::RenderPass<'a>,
    render_resources: &'a RenderAssetStore,
) {
    let models = &render_resources.models;
    let materials = &render_resources.materials;
    let meshes = &render_resources.meshes;
    for material_batch in &pass_draw.batch.material_batches[pass_draw.batch.static_batch.clone()] {
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

pub(crate) fn render_gbuffer_skinned_opaque_pass<'a>(
    encoder: &mut wgpu::CommandEncoder,
    pass_draw: &'a PassDrawContext<'a>,
    render_resources: &'a RenderAssetStore,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Skinned G-Buffer Render Pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.albedo_ao,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.normal_roughness,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.emissive_metallic,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.motion_vectors,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
        ],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &context.descriptors.texture_views.gbuffer.depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&context.pipelines.g_buffer.skinned_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.lights.0, &[]);
    render_pass.set_bind_group(
        3,
        &context.descriptors.bind_groups.bones.motion_bind_group,
        &[],
    );
    draw_gbuffer_skinned(pass_draw, &mut render_pass, render_resources);
}

pub(crate) fn render_gbuffer_static_opaque_pass<'a>(
    encoder: &mut wgpu::CommandEncoder,
    pass_draw: &'a PassDrawContext<'a>,
    render_resources: &'a RenderAssetStore,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Static G-Buffer Render Pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.albedo_ao,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.normal_roughness,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.emissive_metallic,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &context.descriptors.texture_views.gbuffer.motion_vectors,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
        ],
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

    render_pass.set_pipeline(&context.pipelines.g_buffer.static_pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.lights.0, &[]);
    render_pass.set_bind_group(3, &context.descriptors.bind_groups.static_instances.0, &[]);
    draw_gbuffer_static(pass_draw, &mut render_pass, render_resources);
}

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
            view: &context.descriptors.texture_views.lighting_target.lit_hdr,
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

pub(crate) fn draw_static_transparent_pass<'a>(
    pass_draw: &PassDrawContext<'a>,
    render_pass: &mut wgpu::RenderPass<'a>,
    render_resources: &'a RenderAssetStore,
) {
    let models = &render_resources.models;
    let materials = &render_resources.materials;
    let meshes = &render_resources.meshes;
    for material_batch in &pass_draw.batch.material_batches[pass_draw.batch.static_batch.clone()] {
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

pub(crate) fn render_static_transparent_pass<'a>(
    encoder: &mut wgpu::CommandEncoder,
    pass_draw: &'a PassDrawContext<'a>,
    render_resources: &'a RenderAssetStore,
    context: &WorldGpuContext,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Static Transparent Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &context.descriptors.texture_views.lighting_target.lit_hdr,
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

    render_pass.set_pipeline(&context.pipelines.static_transparent.pipeline);
    render_pass.set_bind_group(0, &context.descriptors.bind_groups.camera.0, &[]);
    render_pass.set_bind_group(1, &context.descriptors.bind_groups.lights.0, &[]);
    render_pass.set_bind_group(3, &context.descriptors.bind_groups.static_instances.0, &[]);
    draw_static_transparent_pass(pass_draw, &mut render_pass, render_resources);
}

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

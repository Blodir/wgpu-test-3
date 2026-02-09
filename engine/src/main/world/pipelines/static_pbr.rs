use std::ops::Range;

use crate::game::assets::registry::{MaterialId, MeshId, RenderState};
use crate::main::assets::resource_manager::ResourceManager;
use crate::main::assets::store::{self, MaterialRenderId, MeshRenderId, RenderAssetStore};
use crate::{render_snapshot::MeshDrawSnapshot, main::world::{attachments::depth::DepthTexture }};
use crate::main::world::{buffers::{skinned_instance::SkinnedInstance, skinned_vertex::SkinnedVertex, static_instance::StaticInstance, static_vertex::StaticVertex}, prepare::mesh::DrawContext};
use crate::main::{shader_cache::ShaderCache, wgpu_context::WgpuContext};

pub struct StaticPbrPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
}

impl StaticPbrPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let render_pipeline = Self::build_pipeline(
            wgpu_context,
            shader_cache,
            camera_bind_group_layout,
            lights_bind_group_layout,
            &material_bind_group_layout,
        );

        Self {
            render_pipeline,
        }
    }

    pub fn build_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[StaticInstance::desc(), StaticVertex::desc()];
        let bind_group_layouts = &[
            camera_bind_group_layout,
            lights_bind_group_layout,
            material_bind_group_layout,
        ];
        let render_pipeline_layout =
            wgpu_context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Static PBR Pipeline Layout"),
                bind_group_layouts,
                push_constant_ranges: &[],
            });
        let vertex_shader_module = shader_cache.get("engine/src/main/world/shaders/static_pbr.vert.wgsl".to_string(), wgpu_context);
        let fragment_shader_module = shader_cache.get("engine/src/main/world/shaders/pbr.frag.wgsl".to_string(), wgpu_context);
        wgpu_context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Static PBR Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader_module,
                entry_point: "vs_main",
                buffers: vertex_buffer_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu_context.surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 4,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }

    pub fn render<'a>(
        &self,
        draw_context: DrawContext,
        instance_buffer: &wgpu::Buffer,
        encoder: &mut wgpu::CommandEncoder,
        msaa_texture_view: &wgpu::TextureView,
        msaa_resolve_texture_view: &wgpu::TextureView,
        depth_texture_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
        render_resources: &RenderAssetStore
    ) {
        let models = &render_resources.models;
        let materials = &render_resources.materials;
        let meshes = &render_resources.meshes;

        // TODO can this descriptor be reused?
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Static PBR Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &msaa_texture_view,
                // last pass drawing to msaa resolves
                resolve_target: Some(&msaa_resolve_texture_view),
                ops: wgpu::Operations {
                    // don't clear previous passes work
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1, lights_bind_group, &[]);

        for material_batch in &draw_context.snap.material_batches[draw_context.snap.static_batch.clone()] {
            let material = materials.get(material_batch.material_id.into()).unwrap();
            render_pass.set_bind_group(
                2u32,
                &material.bind_group,
                &[],
            );
            for mesh_batch in &draw_context.snap.mesh_batches[material_batch.mesh_range.clone()] {
                let model = models.get(mesh_batch.model_id.into()).unwrap();
                let mesh = meshes.get(model.mesh_id.into()).unwrap();
                render_pass.set_index_buffer(
                    mesh.buffer.slice(0..model.vertex_buffer_start_offset as u64),
                    wgpu::IndexFormat::Uint32,
                );
                // apparently there's no performance benefit to not just taking the whole instace buffer slice
                render_pass.set_vertex_buffer(0, instance_buffer.slice(..));
                render_pass.set_vertex_buffer(
                    1u32,
                    mesh.buffer.slice(model.vertex_buffer_start_offset as u64..)
                );
                for draw_idx in mesh_batch.submesh_range.clone() {
                    let submesh_batch = &draw_context.snap.submesh_batches[draw_idx];
                    let submesh = &model.submeshes[submesh_batch.submesh_idx];
                    render_pass.draw_indexed(
                        submesh.index_range.clone(),
                        submesh.base_vertex as i32,
                        draw_context.instance_ranges[draw_idx].clone(),
                    );
                }
            }
        }
    }
}

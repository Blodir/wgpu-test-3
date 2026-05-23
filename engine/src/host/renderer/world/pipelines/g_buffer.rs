use crate::global_paths::{
    SHADER_G_BUFFER_FRAG_WGSL, SHADER_SKINNED_PBR_VERT_WGSL, SHADER_STATIC_PBR_VERT_WGSL,
};
use crate::host::assets::store::RenderAssetStore;
use crate::host::world::{
    attachments::deferred::GBufferTargets,
    attachments::depth::DepthTexture,
    buffers::{skinned_vertex::SkinnedVertex, static_vertex::StaticVertex},
    prepare::mesh::PassDrawContext,
};
use crate::host::{shader_cache::ShaderCache, wgpu_context::WgpuContext};

pub struct GBufferPipeline {
    skinned_pipeline: wgpu::RenderPipeline,
    static_pipeline: wgpu::RenderPipeline,
}

impl GBufferPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        bones_bind_group_layout: &wgpu::BindGroupLayout,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let skinned_pipeline = Self::build_skinned_pipeline(
            wgpu_context,
            shader_cache,
            camera_bind_group_layout,
            lights_bind_group_layout,
            material_bind_group_layout,
            bones_bind_group_layout,
        );
        let static_pipeline = Self::build_static_pipeline(
            wgpu_context,
            shader_cache,
            camera_bind_group_layout,
            lights_bind_group_layout,
            material_bind_group_layout,
            instance_bind_group_layout,
        );

        Self {
            skinned_pipeline,
            static_pipeline,
        }
    }

    fn build_skinned_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        bones_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[
            camera_bind_group_layout,
            lights_bind_group_layout,
            material_bind_group_layout,
            bones_bind_group_layout,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Skinned G-Buffer Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module =
            shader_cache.get(SHADER_SKINNED_PBR_VERT_WGSL.to_string(), wgpu_context);
        let fragment_shader_module =
            shader_cache.get(SHADER_G_BUFFER_FRAG_WGSL.to_string(), wgpu_context);
        let targets = &[
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Skinned G-Buffer Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[SkinnedVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: Some("fs_main"),
                    targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            })
    }

    fn build_static_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[
            camera_bind_group_layout,
            lights_bind_group_layout,
            material_bind_group_layout,
            instance_bind_group_layout,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Static G-Buffer Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module =
            shader_cache.get(SHADER_STATIC_PBR_VERT_WGSL.to_string(), wgpu_context);
        let fragment_shader_module =
            shader_cache.get(SHADER_G_BUFFER_FRAG_WGSL.to_string(), wgpu_context);
        let targets = &[
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Static G-Buffer Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[StaticVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: Some("fs_main"),
                    targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            })
    }

    fn draw_skinned<'a>(
        pass_draw: &PassDrawContext<'a>,
        render_pass: &mut wgpu::RenderPass<'a>,
        render_resources: &'a RenderAssetStore,
    ) {
        let models = &render_resources.models;
        let materials = &render_resources.materials;
        let meshes = &render_resources.meshes;
        for material_batch in
            &pass_draw.batch.material_batches[pass_draw.batch.skinned_batch.clone()]
        {
            let material = materials.get(material_batch.material_id.into()).unwrap();
            render_pass.set_bind_group(2u32, &material.bind_group, &[]);
            for mesh_batch in &pass_draw.batch.mesh_batches[material_batch.mesh_range.clone()] {
                let model = models.get(mesh_batch.model_id.into()).unwrap();
                let mesh = meshes.get(model.mesh_id.into()).unwrap();
                render_pass.set_index_buffer(
                    mesh.buffer
                        .slice(0..model.vertex_buffer_start_offset as u64),
                    wgpu::IndexFormat::Uint32,
                );
                render_pass.set_vertex_buffer(
                    0u32,
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

    fn draw_static<'a>(
        pass_draw: &PassDrawContext<'a>,
        render_pass: &mut wgpu::RenderPass<'a>,
        render_resources: &'a RenderAssetStore,
    ) {
        let models = &render_resources.models;
        let materials = &render_resources.materials;
        let meshes = &render_resources.meshes;
        for material_batch in
            &pass_draw.batch.material_batches[pass_draw.batch.static_batch.clone()]
        {
            let material = materials.get(material_batch.material_id.into()).unwrap();
            render_pass.set_bind_group(2u32, &material.bind_group, &[]);
            for mesh_batch in &pass_draw.batch.mesh_batches[material_batch.mesh_range.clone()] {
                let model = models.get(mesh_batch.model_id.into()).unwrap();
                let mesh = meshes.get(model.mesh_id.into()).unwrap();
                render_pass.set_index_buffer(
                    mesh.buffer
                        .slice(0..model.vertex_buffer_start_offset as u64),
                    wgpu::IndexFormat::Uint32,
                );
                render_pass.set_vertex_buffer(
                    0u32,
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

    pub fn render_skinned_opaque<'a>(
        &self,
        pass_draw: &'a PassDrawContext<'a>,
        encoder: &mut wgpu::CommandEncoder,
        gbuffer: &'a GBufferTargets,
        depth_texture_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
        bones_bind_group: &wgpu::BindGroup,
        render_resources: &'a RenderAssetStore,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skinned G-Buffer Render Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.albedo_ao.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.normal_roughness.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.emissive_metallic.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.skinned_pipeline);
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1u32, lights_bind_group, &[]);
        render_pass.set_bind_group(3u32, bones_bind_group, &[]);
        Self::draw_skinned(pass_draw, &mut render_pass, render_resources);
    }

    pub fn render_static_opaque<'a>(
        &self,
        pass_draw: &'a PassDrawContext<'a>,
        encoder: &mut wgpu::CommandEncoder,
        gbuffer: &'a GBufferTargets,
        depth_texture_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
        instance_bind_group: &wgpu::BindGroup,
        render_resources: &'a RenderAssetStore,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Static G-Buffer Render Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.albedo_ao.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.normal_roughness.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &gbuffer.emissive_metallic.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
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

        render_pass.set_pipeline(&self.static_pipeline);
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1u32, lights_bind_group, &[]);
        render_pass.set_bind_group(3u32, instance_bind_group, &[]);
        Self::draw_static(pass_draw, &mut render_pass, render_resources);
    }
}

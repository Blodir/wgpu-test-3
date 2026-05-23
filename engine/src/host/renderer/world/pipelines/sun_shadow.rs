use crate::global_paths::{
    SHADER_SUN_SHADOW_SKINNED_VERT_WGSL, SHADER_SUN_SHADOW_STATIC_VERT_WGSL,
};
use crate::host::assets::store::RenderAssetStore;
use crate::host::world::{
    attachments::depth::DepthTexture,
    buffers::{skinned_vertex::SkinnedVertex, static_vertex::StaticVertex},
    prepare::mesh::PassDrawContext,
};
use crate::host::{shader_cache::ShaderCache, wgpu_context::WgpuContext};

pub struct SunShadowPipeline {
    skinned_pipeline: wgpu::RenderPipeline,
    static_pipeline: wgpu::RenderPipeline,
}

impl SunShadowPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        sun_shadow_matrix_bind_group_layout: &wgpu::BindGroupLayout,
        bones_bind_group_layout: &wgpu::BindGroupLayout,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let skinned_pipeline = Self::build_skinned_pipeline(
            wgpu_context,
            shader_cache,
            sun_shadow_matrix_bind_group_layout,
            bones_bind_group_layout,
        );
        let static_pipeline = Self::build_static_pipeline(
            wgpu_context,
            shader_cache,
            sun_shadow_matrix_bind_group_layout,
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
        sun_shadow_matrix_bind_group_layout: &wgpu::BindGroupLayout,
        bones_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[sun_shadow_matrix_bind_group_layout, bones_bind_group_layout];
        let pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Sun Shadow Skinned Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader = shader_cache.get(
            SHADER_SUN_SHADOW_SKINNED_VERT_WGSL.to_string(),
            wgpu_context,
        );
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Sun Shadow Skinned Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[SkinnedVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: None,
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
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
    }

    fn build_static_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        sun_shadow_matrix_bind_group_layout: &wgpu::BindGroupLayout,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[
            sun_shadow_matrix_bind_group_layout,
            instance_bind_group_layout,
        ];
        let pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Sun Shadow Static Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader =
            shader_cache.get(SHADER_SUN_SHADOW_STATIC_VERT_WGSL.to_string(), wgpu_context);
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Sun Shadow Static Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[StaticVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: None,
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
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
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
        let meshes = &render_resources.meshes;
        for material_batch in
            &pass_draw.batch.material_batches[pass_draw.batch.skinned_batch.clone()]
        {
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

    fn draw_static<'a>(
        pass_draw: &PassDrawContext<'a>,
        render_pass: &mut wgpu::RenderPass<'a>,
        render_resources: &'a RenderAssetStore,
    ) {
        let models = &render_resources.models;
        let meshes = &render_resources.meshes;
        for material_batch in
            &pass_draw.batch.material_batches[pass_draw.batch.static_batch.clone()]
        {
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

    pub fn render<'a>(
        &self,
        skinned_opaque_pass: &'a PassDrawContext<'a>,
        static_opaque_pass: &'a PassDrawContext<'a>,
        encoder: &mut wgpu::CommandEncoder,
        shadow_depth_view: &wgpu::TextureView,
        sun_shadow_matrix_bind_group: &wgpu::BindGroup,
        bones_bind_group: &wgpu::BindGroup,
        static_instance_bind_group: &wgpu::BindGroup,
        render_resources: &'a RenderAssetStore,
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
            skinned_pass.set_pipeline(&self.skinned_pipeline);
            skinned_pass.set_bind_group(0, sun_shadow_matrix_bind_group, &[]);
            skinned_pass.set_bind_group(1, bones_bind_group, &[]);
            Self::draw_skinned(skinned_opaque_pass, &mut skinned_pass, render_resources);
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
        static_pass.set_pipeline(&self.static_pipeline);
        static_pass.set_bind_group(0, sun_shadow_matrix_bind_group, &[]);
        static_pass.set_bind_group(1, static_instance_bind_group, &[]);
        Self::draw_static(static_opaque_pass, &mut static_pass, render_resources);
    }
}

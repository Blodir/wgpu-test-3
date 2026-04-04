use crate::global_paths::{SHADER_PBR_FRAG_WGSL, SHADER_SKINNED_PBR_VERT_WGSL};
use crate::host::assets::store::RenderAssetStore;
use crate::host::world::{
    attachments::depth::DepthTexture,
    buffers::{skinned_instance::SkinnedInstance, skinned_vertex::SkinnedVertex},
    prepare::mesh::PassDrawContext,
};
use crate::host::{shader_cache::ShaderCache, wgpu_context::WgpuContext};

pub struct SkinnedPbrPipeline {
    pub opaque_pipeline: wgpu::RenderPipeline,
    pub transparent_pipeline: wgpu::RenderPipeline,
}

impl SkinnedPbrPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        bones_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let opaque_pipeline = Self::build_pipeline(
            wgpu_context,
            shader_cache,
            camera_bind_group_layout,
            lights_bind_group_layout,
            &material_bind_group_layout,
            &bones_bind_group_layout,
            false,
        );
        let transparent_pipeline = Self::build_pipeline(
            wgpu_context,
            shader_cache,
            camera_bind_group_layout,
            lights_bind_group_layout,
            &material_bind_group_layout,
            &bones_bind_group_layout,
            true,
        );

        Self {
            opaque_pipeline,
            transparent_pipeline,
        }
    }

    pub fn build_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        bones_bind_group_layout: &wgpu::BindGroupLayout,
        transparent: bool,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[SkinnedInstance::desc(), SkinnedVertex::desc()];
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
                    label: Some("Skinned PBR Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module =
            shader_cache.get(SHADER_SKINNED_PBR_VERT_WGSL.to_string(), wgpu_context);
        let fragment_shader_module =
            shader_cache.get(SHADER_PBR_FRAG_WGSL.to_string(), wgpu_context);
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Skinned PBR Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: Some("vs_main"),
                    buffers: vertex_buffer_layouts,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(if transparent {
                            wgpu::BlendState::ALPHA_BLENDING
                        } else {
                            wgpu::BlendState::REPLACE
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
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
                    depth_write_enabled: !transparent,
                    depth_compare: if transparent {
                        wgpu::CompareFunction::LessEqual
                    } else {
                        wgpu::CompareFunction::Less
                    },
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

    fn draw_pass<'a>(
        pass_draw: &PassDrawContext<'a>,
        render_pass: &mut wgpu::RenderPass<'a>,
        instance_buffer: &wgpu::Buffer,
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
                render_pass.set_vertex_buffer(0, instance_buffer.slice(..));
                render_pass.set_vertex_buffer(
                    1u32,
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

    pub fn render_opaque<'a>(
        &self,
        pass_draw: &'a PassDrawContext<'a>,
        instance_buffer: &wgpu::Buffer,
        encoder: &mut wgpu::CommandEncoder,
        hdr_color_view: &wgpu::TextureView,
        depth_texture_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
        bones_bind_group: &wgpu::BindGroup,
        render_resources: &'a RenderAssetStore,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skinned PBR Opaque Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &hdr_color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
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

        render_pass.set_pipeline(&self.opaque_pipeline);
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1, lights_bind_group, &[]);
        render_pass.set_bind_group(3, bones_bind_group, &[]);
        Self::draw_pass(
            pass_draw,
            &mut render_pass,
            instance_buffer,
            render_resources,
        );
    }

    pub fn render_transparent<'a>(
        &self,
        pass_draw: &'a PassDrawContext<'a>,
        instance_buffer: &wgpu::Buffer,
        encoder: &mut wgpu::CommandEncoder,
        hdr_color_view: &wgpu::TextureView,
        depth_texture_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
        bones_bind_group: &wgpu::BindGroup,
        render_resources: &'a RenderAssetStore,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skinned PBR Transparent Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &hdr_color_view,
                resolve_target: None,
                ops: wgpu::Operations {
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

        render_pass.set_pipeline(&self.transparent_pipeline);
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1, lights_bind_group, &[]);
        render_pass.set_bind_group(3, bones_bind_group, &[]);
        Self::draw_pass(
            pass_draw,
            &mut render_pass,
            instance_buffer,
            render_resources,
        );
    }
}

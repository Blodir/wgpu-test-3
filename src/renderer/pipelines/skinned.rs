use std::ops::Range;

use crate::{render_snapshot::SkinnedMeshDrawSnapshot, renderer::{attachments::depth::DepthTexture, buffers::{instance::Instance, vertex::Vertex}, shader_cache::ShaderCache, wgpu_context::WgpuContext}, resource_system::{registry::{MaterialId, MeshId, RenderState}, render_resources::{self, MaterialRenderId, MeshRenderId, RenderResources}, resource_manager::ResourceManager}};

pub struct DrawContext<'a> {
    pub snap: &'a SkinnedMeshDrawSnapshot,
    pub instance_ranges: Vec<Range<u32>>,
}

pub struct ModelPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
}

impl ModelPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        bones_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let render_pipeline = Self::build_pipeline(
            wgpu_context,
            shader_cache,
            camera_bind_group_layout,
            lights_bind_group_layout,
            &material_bind_group_layout,
            &bones_bind_group_layout,
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
        bones_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[Instance::desc(), Vertex::desc()];
        let bind_group_layouts = &[
            camera_bind_group_layout,
            lights_bind_group_layout,
            material_bind_group_layout,
            bones_bind_group_layout,
        ];
        let render_pipeline_layout =
            wgpu_context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Model Pipeline Layout"),
                bind_group_layouts,
                push_constant_ranges: &[],
            });
        let shader_module = shader_cache.get("src/renderer/shaders/pbr.wgsl".to_string(), wgpu_context);
        wgpu_context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Model Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: vertex_buffer_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu_context.surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                // TODO gltf may have different topologies
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
        bones_bind_group: &wgpu::BindGroup,
        render_resources: &RenderResources
    ) {
        let gpu_materials = &render_resources.materials;
        let gpu_meshes = &render_resources.meshes;

        // TODO can this descriptor be reused?
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Model Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &msaa_texture_view,
                resolve_target: Some(&msaa_resolve_texture_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Discard,
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

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1, lights_bind_group, &[]);
        render_pass.set_bind_group(3, bones_bind_group, &[]);

        for material_batch in &draw_context.snap.material_batches {
            let material = gpu_materials.get(material_batch.material.into()).unwrap();
            render_pass.set_bind_group(
                2u32,
                &material.bind_group,
                &[],
            );
            for mesh_batch in &draw_context.snap.mesh_batches[material_batch.mesh_range.clone()] {
                let mesh = gpu_meshes.get(mesh_batch.mesh.into()).unwrap();
                render_pass.set_index_buffer(
                    mesh.buffer.slice(0..mesh_batch.vertex_buffer_start_offset),
                    wgpu::IndexFormat::Uint32,
                );
                // apparently there's no performance benefit to not just taking the whole instace buffer slice
                render_pass.set_vertex_buffer(0, instance_buffer.slice(..));
                render_pass.set_vertex_buffer(
                    1u32,
                    mesh.buffer.slice(mesh_batch.vertex_buffer_start_offset..)
                );
                for draw_idx in mesh_batch.draw_range.clone() {
                    let draw = &draw_context.snap.draws[draw_idx];
                    render_pass.draw_indexed(
                        draw.index_range.clone(),
                        draw.base_vertex,
                        draw_context.instance_ranges[draw_idx].clone(),
                    );
                }
            }
        }
    }
}

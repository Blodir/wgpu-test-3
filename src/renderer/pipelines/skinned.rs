use std::{ops::Range, sync::Arc};

use wgpu::{core::device, util::DeviceExt};

use crate::{renderer::{attachments::depth::DepthTexture, buffers::{instance::Instance, vertex::Vertex}, shader_cache::ShaderCache, wgpu_context::WgpuContext}, resource_manager::{registry::{GpuState, MaterialId, MeshId}, resource_manager::ResourceManager}};

pub struct ResolvedSubmesh {
    pub index_range: Range<u32>,
    pub instance_range: Range<u32>,
    pub base_vertex: i32,
}

pub struct MeshBatch {
    pub mesh: MeshId,
    pub vertex_buffer_start_offset: u64,
    pub draw_range: std::ops::Range<usize>,
}

pub struct MaterialBatch {
    pub material: MaterialId,
    pub mesh_range: std::ops::Range<usize>,
}

pub struct DrawContext {
    pub draws: Vec<ResolvedSubmesh>,
    pub material_batches: Vec<MaterialBatch>,
    pub mesh_batches: Vec<MeshBatch>,
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
        resource_manager: &Arc<ResourceManager>
    ) {
        let reg = resource_manager.registry.lock().unwrap();
        let gpu_materials = resource_manager.gpu.materials.lock().unwrap();
        let gpu_meshes = resource_manager.gpu.meshes.lock().unwrap();

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

        'mat_loop: for material_batch in &draw_context.material_batches {
            let mat_entry = match reg.get_id(&material_batch.material) {
                Some(e) => e,
                None => continue 'mat_loop,
            };
            let material = match mat_entry.gpu_state {
                GpuState::Ready(index) => gpu_materials.get(index).unwrap(),
                _ => continue 'mat_loop,
            };
            render_pass.set_bind_group(
                2u32,
                &material.bind_group,
                &[],
            );
            'mesh_loop: for mesh_batch in &draw_context.mesh_batches[material_batch.mesh_range.clone()] {
                let mesh_entry = match reg.get_id(&mesh_batch.mesh) {
                    Some(e) => e,
                    None => continue 'mesh_loop,
                };
                let mesh = match mesh_entry.gpu_state {
                    GpuState::Ready(index) => gpu_meshes.get(index).unwrap(),
                    _ => continue 'mat_loop,
                };
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
                for draw in &draw_context.draws[mesh_batch.draw_range.clone()] {
                    render_pass.draw_indexed(
                        draw.index_range.clone(),
                        draw.base_vertex,
                        draw.instance_range.clone(),
                    );
                }
            }
        }
    }
}

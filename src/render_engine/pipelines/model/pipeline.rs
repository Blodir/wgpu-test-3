use cgmath::{Matrix, Matrix3, Matrix4, SquareMatrix, Transform};
use wgpu::{core::device, util::DeviceExt};

use crate::{
    render_engine::{
        pipelines::{
            model::{instance::Instance, material::MaterialBinding, vertex::Vertex},
            resources::depth_texture::DepthTexture,
        },
        render_resources::{modelfile, RenderResources, TextureHandle},
        utils,
    },
    scene_tree::{RenderDataType, Scene},
};

pub struct ModelPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
}

impl ModelPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        sun_bind_group_layout: &wgpu::BindGroupLayout,
        environment_map_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let material_bind_group_layout = device.create_bind_group_layout(&MaterialBinding::desc());
        let render_pipeline = Self::build_pipeline(
            device,
            surface_config,
            camera_bind_group_layout,
            sun_bind_group_layout,
            &material_bind_group_layout,
            environment_map_bind_group_layout,
        );

        Self {
            render_pipeline,
            material_bind_group_layout,
        }
    }

    pub fn build_pipeline(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        sun_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        environment_map_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[Instance::desc(), Vertex::desc()];
        let bind_group_layouts = &[
            camera_bind_group_layout,
            sun_bind_group_layout,
            material_bind_group_layout,
            environment_map_bind_group_layout,
        ];
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Model Pipeline Layout"),
                bind_group_layouts,
                push_constant_ranges: &[],
            });
        let shader_module =
            utils::create_shader_module(device, "src/render_engine/pipelines/shaders/pbr.wgsl");
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format: surface_config.format,
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

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        msaa_texture_view: &wgpu::TextureView,
        msaa_resolve_texture_view: &wgpu::TextureView,
        depth_texture_view: &wgpu::TextureView,
        render_resources: &RenderResources,
        scene: &Scene,
    ) {
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
        render_pass.set_bind_group(0u32, &render_resources.camera.bind_group, &[]);
        render_pass.set_bind_group(1u32, &render_resources.sun.bind_group, &[]);
        render_pass.set_bind_group(
            3u32,
            &render_resources.environment_maps[&scene.environment].bind_group,
            &[],
        );

        // TEMPORARILY RENDER ONLY ROOT NODE
        let node = &scene.root;
        let model_handle = match &node.render_data {
            RenderDataType::Model(model) => model,
        };
        let model = render_resources.models.get(&model_handle).unwrap();
        let prims = &model.json.primitives;
        model.update_instance_buffer(queue, &node.transform);

        render_pass.set_vertex_buffer(0, model.instance_buffer.slice(..));
        render_pass.set_index_buffer(
            model
                .index_vertex_buffer
                .slice(0..model.json.vertex_buffer_start_offset as u64),
            // we are currently forcing u32 in modelfile... maybe in the future should allow u16
            wgpu::IndexFormat::Uint32,
        );
        render_pass.set_vertex_buffer(
            1u32,
            model
                .index_vertex_buffer
                .slice(model.json.vertex_buffer_start_offset as u64..),
        );

        let mut instance_offset = 0u32;
        for prim in prims {
            let mat_handle = &model.materials[prim.material as usize];
            render_pass.set_bind_group(
                2u32,
                &render_resources
                    .materials
                    .get(&mat_handle)
                    .unwrap()
                    .bind_group,
                &[],
            );
            let index_start = prim.index_byte_offset / 4;
            let index_count = prim.index_byte_length / 4;
            render_pass.draw_indexed(
                index_start..index_start + index_count,
                0,
                instance_offset..instance_offset + prim.instances.len() as u32,
            );
            instance_offset += prim.instances.len() as u32;
        }
    }
}

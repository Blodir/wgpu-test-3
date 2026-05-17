use wgpu::util::DeviceExt;

use crate::global_paths::SHADER_HBGI_REPROJECT_WGSL;
use crate::host::{
    shader_cache::ShaderCache,
    wgpu_context::WgpuContext,
    world::attachments::{
        color::HdrColorTexture, deferred::GBufferTexture, hbgi_pyramid::FloatPyramidTexture,
        motion_vectors::MotionVectorsTexture,
    },
};

const INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub struct HbgiReprojectPipeline {
    render_pipeline: wgpu::RenderPipeline,
    index_buffer: wgpu::Buffer,
    inputs_bind_group_layout: wgpu::BindGroupLayout,
    inputs_bind_group: wgpu::BindGroup,
}

impl HbgiReprojectPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        motion_vectors: &MotionVectorsTexture,
        prev_gi_source: &HdrColorTexture,
        prev_hbgi_reproject: &HdrColorTexture,
        depth_texture_view: &wgpu::TextureView,
        current_normal: &GBufferTexture,
        prev_depth_history: &FloatPyramidTexture,
        prev_normal_history: &HdrColorTexture,
    ) -> Self {
        let inputs_bind_group_layout =
            wgpu_context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 6,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                    ],
                    label: Some("HBGI Reproject Inputs Bind Group Layout"),
                });
        let inputs_bind_group = Self::create_inputs_bind_group(
            &wgpu_context.device,
            &inputs_bind_group_layout,
            motion_vectors,
            prev_gi_source,
            prev_hbgi_reproject,
            depth_texture_view,
            current_normal,
            prev_depth_history,
            prev_normal_history,
        );
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Reproject Pipeline Layout"),
                    bind_group_layouts: &[&inputs_bind_group_layout],
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_HBGI_REPROJECT_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Reproject Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::R32Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
        let index_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("HBGI Reproject Index Buffer"),
                    contents: bytemuck::cast_slice(INDICES),
                    usage: wgpu::BufferUsages::INDEX,
                });

        Self {
            render_pipeline,
            index_buffer,
            inputs_bind_group_layout,
            inputs_bind_group,
        }
    }

    fn create_inputs_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        motion_vectors: &MotionVectorsTexture,
        prev_gi_source: &HdrColorTexture,
        prev_hbgi_reproject: &HdrColorTexture,
        depth_texture_view: &wgpu::TextureView,
        current_normal: &GBufferTexture,
        prev_depth_history: &FloatPyramidTexture,
        prev_normal_history: &HdrColorTexture,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&motion_vectors.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&prev_gi_source.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&prev_hbgi_reproject.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(depth_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&current_normal.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&prev_depth_history.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&prev_normal_history.view),
                },
            ],
            label: Some("HBGI Reproject Inputs Bind Group"),
        })
    }

    pub fn update_input_bindgroup(
        &mut self,
        device: &wgpu::Device,
        motion_vectors: &MotionVectorsTexture,
        prev_gi_source: &HdrColorTexture,
        prev_hbgi_reproject: &HdrColorTexture,
        depth_texture_view: &wgpu::TextureView,
        current_normal: &GBufferTexture,
        prev_depth_history: &FloatPyramidTexture,
        prev_normal_history: &HdrColorTexture,
    ) {
        self.inputs_bind_group = Self::create_inputs_bind_group(
            device,
            &self.inputs_bind_group_layout,
            motion_vectors,
            prev_gi_source,
            prev_hbgi_reproject,
            depth_texture_view,
            current_normal,
            prev_depth_history,
            prev_normal_history,
        );
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        hbgi_reproject_view: &wgpu::TextureView,
        hbgi_reproject_depth_view: &wgpu::TextureView,
        hbgi_reproject_normal_view: &wgpu::TextureView,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("HBGI Reproject Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: hbgi_reproject_view,
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
                    view: hbgi_reproject_depth_view,
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
                    view: hbgi_reproject_normal_view,
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

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0u32, &self.inputs_bind_group, &[]);
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
    }
}

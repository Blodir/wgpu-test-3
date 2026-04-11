use wgpu::util::DeviceExt;

use crate::global_paths::SHADER_GTAO_WGSL;
use crate::host::renderer::GtaoOptions;
use crate::host::world::attachments::{
    color::HdrColorTexture,
    deferred::{GBufferTargets, GtaoTexture},
};
use crate::host::world::bindgroups::gtao_settings::{GtaoSettings, GtaoSettingsBinding};
use crate::host::{shader_cache::ShaderCache, wgpu_context::WgpuContext};

const INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub struct GtaoPipeline {
    render_pipeline: wgpu::RenderPipeline,
    index_buffer: wgpu::Buffer,
    inputs_bind_group_layout: wgpu::BindGroupLayout,
    inputs_bind_group: wgpu::BindGroup,
    settings_bind_group_layout: wgpu::BindGroupLayout,
    settings_bind_group: GtaoSettingsBinding,
}

impl GtaoPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        gbuffer_targets: &GBufferTargets,
        gi_source_texture: &HdrColorTexture,
        gtao_options: &GtaoOptions,
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
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                    ],
                    label: Some("GTAO Inputs Bind Group Layout"),
                });
        let inputs_bind_group = Self::create_inputs_bind_group(
            &wgpu_context.device,
            &inputs_bind_group_layout,
            gbuffer_targets,
            gi_source_texture,
        );
        let settings_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&GtaoSettings::desc());
        let settings_bind_group = GtaoSettings::upload(
            &wgpu_context.device,
            &settings_bind_group_layout,
            gtao_options,
        );
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("GTAO Pipeline Layout"),
                    bind_group_layouts: &[
                        camera_bind_group_layout,
                        &inputs_bind_group_layout,
                        &settings_bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_GTAO_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("GTAO Pipeline"),
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
                    label: Some("GTAO Index Buffer"),
                    contents: bytemuck::cast_slice(INDICES),
                    usage: wgpu::BufferUsages::INDEX,
                });

        Self {
            render_pipeline,
            index_buffer,
            inputs_bind_group_layout,
            inputs_bind_group,
            settings_bind_group_layout,
            settings_bind_group,
        }
    }

    fn create_inputs_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        gbuffer_targets: &GBufferTargets,
        gi_source_texture: &HdrColorTexture,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &gbuffer_targets.normal_roughness.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        &gbuffer_targets.normal_roughness.sampler,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &gbuffer_targets.world_position.view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(
                        &gbuffer_targets.world_position.sampler,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&gi_source_texture.view),
                },
            ],
            label: Some("GTAO Inputs Bind Group"),
        })
    }

    pub fn update_input_bindgroups(
        &mut self,
        device: &wgpu::Device,
        gbuffer_targets: &GBufferTargets,
        gi_source_texture: &HdrColorTexture,
        gtao_options: &GtaoOptions,
    ) {
        self.inputs_bind_group = Self::create_inputs_bind_group(
            device,
            &self.inputs_bind_group_layout,
            gbuffer_targets,
            gi_source_texture,
        );
        self.settings_bind_group =
            GtaoSettings::upload(device, &self.settings_bind_group_layout, gtao_options);
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        gtao_texture: &GtaoTexture,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GTAO Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &gtao_texture.view,
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
                    view: &gtao_texture.hbil_diffuse_view,
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
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1u32, &self.inputs_bind_group, &[]);
        render_pass.set_bind_group(2u32, &self.settings_bind_group.bind_group, &[]);
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
    }
}

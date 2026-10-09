use crate::host::renderer::world::gpu_context::{
    descriptors::texture_views::TextureViews,
    pipelines::{FullscreenPipeline, SsrtPipelines},
    resources::{
        textures::{SsrtTextures, SSRT_A_TROUS_PASS_COUNT},
        Buffers, Samplers,
    },
    BGLayouts,
};

pub(crate) struct SsrtHistoryBindGroups {
    pub diffuse_normal_reprojects: wgpu::BindGroup,
    pub resolve: wgpu::BindGroup,
    pub ssgi_trace: wgpu::BindGroup,
    pub ssr_trace: wgpu::BindGroup,
    pub history_fix: wgpu::BindGroup,
    pub a_trous: [wgpu::BindGroup; 2],
    pub taa: wgpu::BindGroup,
    pub tonemap: wgpu::BindGroup,
    pub debug_copy_normal_history: wgpu::BindGroup,
}
pub(crate) struct SsrtBindGroups {
    pub history: [SsrtHistoryBindGroups; 2],
    pub lighting_composite: wgpu::BindGroup,
    pub depth_base: wgpu::BindGroup,
    pub depth_downsample: Vec<wgpu::BindGroup>,
    pub a_trous_settings: [wgpu::BindGroup; SSRT_A_TROUS_PASS_COUNT],
    pub settings: wgpu::BindGroup,
}
impl SsrtBindGroups {
    pub fn new(
        device: &wgpu::Device,
        views: &TextureViews,
        textures: &mut SsrtTextures,
        buffers: &Buffers,
        samplers: &Samplers,
        layouts: &BGLayouts,
        pipelines: &SsrtPipelines,
    ) -> Self {
        let create =
            |label: &str, pipeline: &FullscreenPipeline, sources: &[&wgpu::TextureView]| {
                let entries: Vec<_> = sources
                    .iter()
                    .enumerate()
                    .map(|(index, view)| wgpu::BindGroupEntry {
                        binding: index as u32,
                        resource: wgpu::BindingResource::TextureView(view),
                    })
                    .collect();
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout: &pipeline.input_layout,
                    entries: &entries,
                })
            };
        let ssrt = &views.ssrt;
        let history = std::array::from_fn(|_| {
            let bind_groups = SsrtHistoryBindGroups {
                diffuse_normal_reprojects: create(
                    "Diffuse Normal Reprojects Inputs",
                    &pipelines.diffuse_normal_reprojects,
                    &[
                        &views.gbuffer.normal_roughness,
                        ssrt.diffuse_history
                            .get_read_view(&textures.diffuse_history),
                        &views.gbuffer.motion_vectors,
                        &views.gbuffer.depth,
                        ssrt.depth_history.get_read_view(&textures.depth_history),
                        ssrt.normal_roughness_history
                            .get_read_view(&textures.normal_roughness_history),
                    ],
                ),
                resolve: create(
                    "SSRT Resolve Inputs",
                    &pipelines.resolve,
                    &[
                        &ssrt.ssgi_trace,
                        ssrt.ssgi_history.get_read_view(&textures.ssgi_history),
                        &ssrt.ssr_trace,
                        ssrt.ssr_history.get_read_view(&textures.ssr_history),
                        ssrt.moments_history
                            .get_read_view(&textures.moments_history),
                        &views.gbuffer.motion_vectors,
                        &views.gbuffer.depth,
                        ssrt.normal_roughness_history
                            .get_write_view(&textures.normal_roughness_history),
                        ssrt.depth_history.get_read_view(&textures.depth_history),
                        ssrt.normal_roughness_history
                            .get_read_view(&textures.normal_roughness_history),
                    ],
                ),
                ssgi_trace: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SSGI Trace Inputs"),
                    layout: &pipelines.ssgi_trace.input_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                ssrt.normal_roughness_history
                                    .get_write_view(&textures.normal_roughness_history),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&ssrt.reprojected_diffuse),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(
                                &views.depth_pyramid.all_mips,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(
                                &views.external.blue_noise,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(&samplers.linear),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&samplers.linear),
                        },
                    ],
                }),
                ssr_trace: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SSR Trace Inputs"),
                    layout: &pipelines.ssr_trace.input_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                ssrt.normal_roughness_history
                                    .get_write_view(&textures.normal_roughness_history),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&ssrt.reprojected_diffuse),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(
                                &views.depth_pyramid.all_mips,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&samplers.linear),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(&samplers.linear),
                        },
                    ],
                }),
                history_fix: create(
                    "SSRT History Fix Inputs",
                    &pipelines.history_fix,
                    &[
                        ssrt.ssgi_history.get_write_view(&textures.ssgi_history),
                        ssrt.ssr_history.get_write_view(&textures.ssr_history),
                        ssrt.normal_roughness_history
                            .get_write_view(&textures.normal_roughness_history),
                        &views.depth_pyramid.mips[0],
                        ssrt.moments_history
                            .get_write_view(&textures.moments_history),
                    ],
                ),
                a_trous: std::array::from_fn(|_| {
                    let bind_group = create(
                        "SSRT A-Trous Inputs",
                        &pipelines.a_trous,
                        &[
                            ssrt.ssgi_denoise.get_read_view(&textures.ssgi_denoise),
                            ssrt.normal_roughness_history
                                .get_write_view(&textures.normal_roughness_history),
                            &views.depth_pyramid.mips[0],
                        ],
                    );
                    textures.ssgi_denoise.swap();
                    bind_group
                }),
                taa: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("TAA Inputs"),
                    layout: &pipelines.taa.input_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&ssrt.final_hdr),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&samplers.linear),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(
                                ssrt.final_hdr_history
                                    .get_read_view(&textures.final_hdr_history),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(
                                &views.depth_pyramid.mips[0],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(
                                ssrt.depth_history.get_read_view(&textures.depth_history),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::TextureView(
                                &views.gbuffer.motion_vectors,
                            ),
                        },
                    ],
                }),
                tonemap: create(
                    "Tonemap Inputs",
                    &pipelines.tonemap,
                    &[ssrt
                        .final_hdr_history
                        .get_write_view(&textures.final_hdr_history)],
                ),
                debug_copy_normal_history: create(
                    "Debug Copy Normal History Inputs",
                    &pipelines.debug_copy_normal_history,
                    &[ssrt
                        .normal_roughness_history
                        .get_write_view(&textures.normal_roughness_history)],
                ),
            };
            textures.rotate();
            bind_groups
        });
        let lighting_composite = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lighting Composite Inputs"),
            layout: &pipelines.lighting_composite.input_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views.gbuffer.albedo_ao),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&samplers.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&views.gbuffer.normal_roughness),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&samplers.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&views.gbuffer.emissive_metallic),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&samplers.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&views.gbuffer.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        if SSRT_A_TROUS_PASS_COUNT % 2 == 0 {
                            ssrt.ssgi_denoise.get_write_view(&textures.ssgi_denoise)
                        } else {
                            ssrt.ssgi_denoise.get_read_view(&textures.ssgi_denoise)
                        },
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&samplers.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&ssrt.ssr_denoise),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&samplers.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&ssrt.reprojected_diffuse),
                },
            ],
        });
        let depth_base = create(
            "Depth Pyramid Base Inputs",
            &pipelines.depth_base,
            &[&views.gbuffer.depth],
        );
        let depth_downsample = views
            .depth_pyramid
            .mips
            .iter()
            .take(views.depth_pyramid.mips.len() - 1)
            .map(|view| {
                create(
                    "Depth Pyramid Downsample Inputs",
                    &pipelines.depth_downsample,
                    &[view],
                )
            })
            .collect();
        let create_uniform =
            |label: &str, buffer: &wgpu::Buffer, layout: &wgpu::BindGroupLayout| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                })
            };
        let a_trous_settings = std::array::from_fn(|index| {
            create_uniform(
                "SSRT A-Trous Settings",
                &buffers.a_trous_settings[index],
                &layouts.a_trous_settings,
            )
        });
        let settings = create_uniform(
            "SSRT Settings",
            &buffers.ssrt_settings,
            &layouts.ssrt_settings,
        );
        Self {
            history,
            lighting_composite,
            depth_base,
            depth_downsample,
            a_trous_settings,
            settings,
        }
    }
}

use crate::{
    global_paths::{
        SHADER_DEFERRED_LIGHTING_WGSL, SHADER_GI_BLUR_WGSL, SHADER_G_BUFFER_FRAG_WGSL,
        SHADER_G_BUFFER_SKINNED_VERT_WGSL, SHADER_G_BUFFER_STATIC_VERT_WGSL,
        SHADER_HBGI_PYRAMID_WGSL, SHADER_HBGI_REPROJECT_WGSL, SHADER_HBGI_WGSL,
        SHADER_PBR_FRAG_WGSL, SHADER_POST_PROCESSING_WGSL, SHADER_SKINNED_TRANSPARENT_VERT_WGSL,
        SHADER_SKYBOX_WGSL, SHADER_STATIC_TRANSPARENT_VERT_WGSL,
        SHADER_SUN_SHADOW_SKINNED_VERT_WGSL, SHADER_SUN_SHADOW_STATIC_VERT_WGSL,
    },
    host::{
        renderer::{
            world::{
                gpu_context::{
                    BGLayouts, GBufferTextures, HbgiInputsBindGroup, HbgiSettingsBindGroup,
                },
                skinned_vertex::SkinnedVertex,
                static_vertex::StaticVertex,
            },
        },
        shader_cache::ShaderCache,
        wgpu_context::WgpuContext,
    },
};

pub(crate) struct GBufferPipeline {
    pub(crate) skinned_pipeline: wgpu::RenderPipeline,
    pub(crate) static_pipeline: wgpu::RenderPipeline,
}
impl GBufferPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let skinned_pipeline = Self::build_skinned_pipeline(wgpu_context, shader_cache, layouts);
        let static_pipeline = Self::build_static_pipeline(wgpu_context, shader_cache, layouts);

        Self {
            skinned_pipeline,
            static_pipeline,
        }
    }

    fn build_skinned_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.material,
            &layouts.motion_bones,
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
            shader_cache.get(SHADER_G_BUFFER_SKINNED_VERT_WGSL.to_string(), wgpu_context);
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
                    format: GBufferTextures::DEPTH_FORMAT,
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
        layouts: &BGLayouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.material,
            &layouts.instance_storage,
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
            shader_cache.get(SHADER_G_BUFFER_STATIC_VERT_WGSL.to_string(), wgpu_context);
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
                    format: GBufferTextures::DEPTH_FORMAT,
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
}

pub(crate) struct HbgiPipeline {
    pub(crate) render_pipeline: wgpu::RenderPipeline,
}
impl HbgiPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let hbgi_inputs_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&HbgiInputsBindGroup::desc());
        let hbgi_settings_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&HbgiSettingsBindGroup::desc());
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Pipeline Layout"),
                    bind_group_layouts: &[
                        &layouts.camera,
                        &hbgi_inputs_bind_group_layout,
                        &hbgi_settings_bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_HBGI_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Pipeline"),
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
        Self { render_pipeline }
    }
}

const HBGI_PYRAMID_COLOR_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const HBGI_PYRAMID_DEPTH_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

pub(crate) struct HbgiPyramidPipelines {
    pub(crate) base_pipeline: wgpu::RenderPipeline,
    pub(crate) downsample_pipeline: wgpu::RenderPipeline,
}
impl HbgiPyramidPipelines {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let shader_module = shader_cache.get(SHADER_HBGI_PYRAMID_WGSL.to_string(), wgpu_context);
        let base_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Pyramid Base Pipeline Layout"),
                    bind_group_layouts: &[&layouts.hbgi_pyramid_base],
                    push_constant_ranges: &[],
                });
        let downsample_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Pyramid Downsample Pipeline Layout"),
                    bind_group_layouts: &[&layouts.hbgi_pyramid_downsample],
                    push_constant_ranges: &[],
                });
        let color_targets = &[
            Some(wgpu::ColorTargetState {
                format: HBGI_PYRAMID_COLOR_TARGET_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: HBGI_PYRAMID_DEPTH_TARGET_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: HBGI_PYRAMID_COLOR_TARGET_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];
        let base_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Pyramid Base Pipeline"),
                    layout: Some(&base_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_copy_base"),
                        targets: color_targets,
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
        let downsample_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Pyramid Downsample Pipeline"),
                    layout: Some(&downsample_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_downsample"),
                        targets: color_targets,
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

        Self {
            base_pipeline,
            downsample_pipeline,
        }
    }
}

pub(crate) struct GiBlurPipeline {
    pub(crate) render_pipeline: wgpu::RenderPipeline,
}
impl GiBlurPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("GI Blur Pipeline Layout"),
                    bind_group_layouts: &[&layouts.gi_blur, &layouts.camera],
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_GI_BLUR_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("GI Blur Pipeline"),
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

        Self { render_pipeline }
    }
}

pub(crate) struct DeferredLightingPipeline {
    pub(crate) render_pipeline: wgpu::RenderPipeline,
}
impl DeferredLightingPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.deferred_lighting_gbuffer,
            &layouts.deferred_lighting_hbgi,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Deferred Lighting Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let shader_module =
            shader_cache.get(SHADER_DEFERRED_LIGHTING_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Deferred Lighting Pipeline"),
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
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState::REPLACE),
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

        Self { render_pipeline }
    }
}

pub(crate) struct HbgiReprojectPipeline {
    pub(crate) render_pipeline: wgpu::RenderPipeline,
}
impl HbgiReprojectPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Reproject Pipeline Layout"),
                    bind_group_layouts: &[
                        &layouts.camera,
                        &layouts.hbgi_reproject_inputs,
                        &layouts.hbgi_reproject_settings,
                    ],
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

        Self { render_pipeline }
    }
}

pub(crate) struct SkyboxPipeline {
    pub(crate) render_pipeline: wgpu::RenderPipeline,
}
impl SkyboxPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let bind_group_layouts = &[&layouts.camera, &layouts.lights];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Skybox Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_SKYBOX_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Skybox Render Pipeline"),
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
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
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

        Self { render_pipeline }
    }
}

pub(crate) struct SunShadowPipeline {
    pub(crate) skinned_pipeline: wgpu::RenderPipeline,
    pub(crate) static_pipeline: wgpu::RenderPipeline,
}
impl SunShadowPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let skinned_pipeline = Self::build_skinned_pipeline(wgpu_context, shader_cache, layouts);
        let static_pipeline = Self::build_static_pipeline(wgpu_context, shader_cache, layouts);

        Self {
            skinned_pipeline,
            static_pipeline,
        }
    }

    fn build_skinned_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[&layouts.sun_shadow_matrix, &layouts.bones];
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
                    format: GBufferTextures::DEPTH_FORMAT,
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
        layouts: &BGLayouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[&layouts.sun_shadow_matrix, &layouts.instance_storage];
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
                    format: GBufferTextures::DEPTH_FORMAT,
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
}

pub(crate) struct SkinnedTransparentPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
}
impl SkinnedTransparentPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let pipeline = Self::build_pipeline(wgpu_context, shader_cache, layouts);
        Self { pipeline }
    }

    fn build_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[SkinnedVertex::desc()];
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.pbr_material,
            &layouts.motion_bones,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Skinned Transparent Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module = shader_cache.get(
            SHADER_SKINNED_TRANSPARENT_VERT_WGSL.to_string(),
            wgpu_context,
        );
        let fragment_shader_module =
            shader_cache.get(SHADER_PBR_FRAG_WGSL.to_string(), wgpu_context);

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Skinned Transparent Pipeline"),
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
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                    format: GBufferTextures::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
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
}

pub(crate) struct StaticTransparentPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
}
impl StaticTransparentPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let pipeline = Self::build_pipeline(wgpu_context, shader_cache, layouts);
        Self { pipeline }
    }

    fn build_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[StaticVertex::desc()];
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.pbr_material,
            &layouts.instance_storage,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Static Transparent Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module = shader_cache.get(
            SHADER_STATIC_TRANSPARENT_VERT_WGSL.to_string(),
            wgpu_context,
        );
        let fragment_shader_module =
            shader_cache.get(SHADER_PBR_FRAG_WGSL.to_string(), wgpu_context);

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Static Transparent Pipeline"),
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
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                    format: GBufferTextures::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
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
}

pub(crate) struct PostProcessingPipeline {
    pub(crate) render_pipeline: wgpu::RenderPipeline,
}
impl PostProcessingPipeline {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let bind_group_layouts = &[&layouts.post_processing];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Post Processing Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_POST_PROCESSING_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Post Processing Render Pipeline"),
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
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu_context.surface_config.format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
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

        Self { render_pipeline }
    }
}

pub(crate) struct Pipelines {
    pub(crate) g_buffer: GBufferPipeline,
    pub(crate) hbgi: HbgiPipeline,
    pub(crate) hbgi_pyramid: HbgiPyramidPipelines,
    pub(crate) gi_blur: GiBlurPipeline,
    pub(crate) deferred_lighting: DeferredLightingPipeline,
    pub(crate) hbgi_reproject: HbgiReprojectPipeline,
    pub(crate) skybox: SkyboxPipeline,
    pub(crate) sun_shadow: SunShadowPipeline,
    pub(crate) skinned_transparent: SkinnedTransparentPipeline,
    pub(crate) static_transparent: StaticTransparentPipeline,
    pub(crate) post: PostProcessingPipeline,
}
impl Pipelines {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        let g_buffer = GBufferPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi = HbgiPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi_pyramid = HbgiPyramidPipelines::new(wgpu_context, shader_cache, layouts);
        let gi_blur = GiBlurPipeline::new(wgpu_context, shader_cache, layouts);
        let deferred_lighting = DeferredLightingPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi_reproject = HbgiReprojectPipeline::new(wgpu_context, shader_cache, layouts);
        let skybox = SkyboxPipeline::new(wgpu_context, shader_cache, layouts);
        let sun_shadow = SunShadowPipeline::new(wgpu_context, shader_cache, layouts);
        let skinned_transparent =
            SkinnedTransparentPipeline::new(wgpu_context, shader_cache, layouts);
        let static_transparent =
            StaticTransparentPipeline::new(wgpu_context, shader_cache, layouts);
        let post = PostProcessingPipeline::new(wgpu_context, shader_cache, layouts);

        Self {
            g_buffer,
            hbgi,
            hbgi_pyramid,
            gi_blur,
            deferred_lighting,
            hbgi_reproject,
            skybox,
            sun_shadow,
            skinned_transparent,
            static_transparent,
            post,
        }
    }
}

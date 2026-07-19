use crate::{
    global_paths::{
        SHADER_G_BUFFER_FRAG_WGSL, SHADER_G_BUFFER_SKINNED_VERT_WGSL,
        SHADER_G_BUFFER_STATIC_VERT_WGSL,
    },
    host::{
        renderer::world::{
            gpu_context::{BGLayouts, GBufferTextures},
            skinned_vertex::SkinnedVertex,
            static_vertex::StaticVertex,
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

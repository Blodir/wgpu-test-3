use crate::{
    global_paths::{SHADER_SUN_SHADOW_SKINNED_VERT_WGSL, SHADER_SUN_SHADOW_STATIC_VERT_WGSL},
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

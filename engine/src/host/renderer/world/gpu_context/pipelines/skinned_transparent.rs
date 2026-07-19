use crate::{
    global_paths::{SHADER_PBR_FRAG_WGSL, SHADER_SKINNED_TRANSPARENT_VERT_WGSL},
    host::{
        renderer::world::{
            external::SkinnedVertex,
            gpu_context::{BGLayouts, GBufferTextures},
        },
        shader_cache::ShaderCache,
        wgpu_context::WgpuContext,
    },
};

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

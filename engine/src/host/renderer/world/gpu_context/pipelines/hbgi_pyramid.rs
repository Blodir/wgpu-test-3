use crate::{
    global_paths::SHADER_HBGI_PYRAMID_WGSL,
    host::{
        renderer::world::gpu_context::BGLayouts, shader_cache::ShaderCache,
        wgpu_context::WgpuContext,
    },
};

const HBGI_PYRAMID_DEPTH_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
const HBGI_PYRAMID_NORMAL_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

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
                format: HBGI_PYRAMID_DEPTH_TARGET_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: HBGI_PYRAMID_NORMAL_TARGET_FORMAT,
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

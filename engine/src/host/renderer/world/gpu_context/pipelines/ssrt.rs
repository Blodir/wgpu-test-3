use super::super::BGLayouts;
use crate::host::{shader_cache::ShaderCache, wgpu_context::WgpuContext};

pub(crate) enum TextureInput {
    Float,
    FilterableFloat,
    Depth,
    Sampler,
}
pub(crate) struct FullscreenPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub input_layout: wgpu::BindGroupLayout,
}
impl FullscreenPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        label: &str,
        shader_path: &str,
        inputs: &[TextureInput],
        formats: &[wgpu::TextureFormat],
        extra_layouts: &[&wgpu::BindGroupLayout],
    ) -> Self {
        let device = &wgpu_context.device;
        let entries: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(index, input)| {
                let ty = match input {
                    TextureInput::Float | TextureInput::FilterableFloat => {
                        wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: matches!(input, TextureInput::FilterableFloat),
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        }
                    }
                    TextureInput::Depth => wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    TextureInput::Sampler => {
                        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                    }
                };
                wgpu::BindGroupLayoutEntry {
                    binding: index as u32,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty,
                    count: None,
                }
            })
            .collect();
        let input_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &entries,
        });
        let layouts: Vec<_> = std::iter::once(&input_layout)
            .chain(extra_layouts.iter().copied())
            .collect();
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &layouts,
            push_constant_ranges: &[],
        });
        let shader = shader_cache.get(shader_path.to_string(), wgpu_context);
        let targets: Vec<_> = formats
            .iter()
            .map(|format| {
                Some(wgpu::ColorTargetState {
                    format: *format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
            .collect();
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &targets,
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            input_layout,
        }
    }
}
pub(crate) struct SsrtPipelines {
    pub depth_base: FullscreenPipeline,
    pub depth_downsample: FullscreenPipeline,
    pub diffuse_normal_reprojects: FullscreenPipeline,
    pub reprojected_diffuse_downsample: FullscreenPipeline,
    pub ssgi_trace: FullscreenPipeline,
    pub ssr_trace: FullscreenPipeline,
    pub resolve: FullscreenPipeline,
    pub history_fix: FullscreenPipeline,
    pub a_trous: FullscreenPipeline,
    pub lighting_composite: FullscreenPipeline,
    pub taa: FullscreenPipeline,
    pub tonemap: FullscreenPipeline,
    pub debug_copy_normal_history: FullscreenPipeline,
    pub debug_copy_ssr_trace: FullscreenPipeline,
}
impl SsrtPipelines {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &BGLayouts,
    ) -> Self {
        use crate::global_paths::*;
        use TextureInput::{Depth, FilterableFloat, Float, Sampler};
        let color = wgpu::TextureFormat::Rgba16Float;
        let mut create = |label,
                          path,
                          inputs: &[TextureInput],
                          formats: &[wgpu::TextureFormat],
                          extra_layouts: &[&wgpu::BindGroupLayout]| {
            FullscreenPipeline::new(
                wgpu_context,
                shader_cache,
                label,
                path,
                inputs,
                formats,
                extra_layouts,
            )
        };
        Self {
            depth_base: create(
                "Depth Pyramid Base",
                SHADER_DEPTH_PYRAMID_WGSL,
                &[Depth],
                &[wgpu::TextureFormat::R32Float],
                &[],
            ),
            depth_downsample: create(
                "Depth Pyramid Downsample",
                SHADER_DEPTH_PYRAMID_DOWNSAMPLE_WGSL,
                &[Float],
                &[wgpu::TextureFormat::R32Float],
                &[],
            ),
            diffuse_normal_reprojects: create(
                "Diffuse Normal Reprojects",
                SHADER_DIFFUSE_NORMAL_REPROJECTS_WGSL,
                &[Float, Float, Float, Depth, Float, Float, Float, Float],
                &[color, color, color, wgpu::TextureFormat::R16Float],
                &[&layouts.camera],
            ),
            reprojected_diffuse_downsample: create(
                "Reprojected Diffuse Downsample",
                SHADER_REPROJECTED_DIFFUSE_DOWNSAMPLE_WGSL,
                &[Float],
                &[color],
                &[],
            ),
            ssgi_trace: create(
                "SSGI Trace",
                SHADER_SSGI_TRACE_WGSL,
                &[
                    FilterableFloat,
                    FilterableFloat,
                    Float,
                    Float,
                    Sampler,
                    Sampler,
                ],
                &[color; 2],
                &[&layouts.ssrt_settings, &layouts.camera],
            ),
            ssr_trace: create(
                "SSR Trace",
                SHADER_SSR_TRACE_WGSL,
                &[
                    FilterableFloat,
                    FilterableFloat,
                    Float,
                    Sampler,
                    Sampler,
                    FilterableFloat,
                    FilterableFloat,
                ],
                &[color],
                &[&layouts.ssrt_settings, &layouts.camera, &layouts.lights],
            ),
            resolve: create(
                "SSRT Resolve",
                SHADER_SSRT_RESOLVE_WGSL,
                &[
                    Float, Float, Float, Float, Float, Float, Depth, Float, Float, Float, Float,
                    Float,
                ],
                &[color; 4],
                &[&layouts.camera],
            ),
            history_fix: create(
                "SSRT History Fix",
                SHADER_SSRT_HISTORY_FIX_WGSL,
                &[Float, Float, Float, Float, Float, Float],
                &[color; 3],
                &[&layouts.camera],
            ),
            a_trous: create(
                "SSRT A-Trous",
                SHADER_SSRT_A_TROUS_WGSL,
                &[Float, Float, Float, Float],
                &[color; 2],
                &[&layouts.a_trous_settings, &layouts.camera],
            ),
            lighting_composite: create(
                "Lighting Composite",
                SHADER_LIGHTING_COMPOSITE_WGSL,
                &[
                    FilterableFloat,
                    Sampler,
                    FilterableFloat,
                    Sampler,
                    FilterableFloat,
                    Sampler,
                    Depth,
                    FilterableFloat,
                    Sampler,
                    FilterableFloat,
                    Sampler,
                    Float,
                    FilterableFloat,
                    Float,
                    Float,
                ],
                &[color; 2],
                &[&layouts.camera, &layouts.lights],
            ),
            taa: create(
                "TAA",
                SHADER_TAA_WGSL,
                &[
                    FilterableFloat,
                    Sampler,
                    FilterableFloat,
                    Float,
                    Float,
                    Float,
                ],
                &[color],
                &[&layouts.camera],
            ),
            tonemap: create(
                "Tonemap",
                SHADER_TONEMAP_WGSL,
                &[Float],
                &[wgpu_context.surface_config.format],
                &[],
            ),
            debug_copy_normal_history: create(
                "Debug Copy Normal History",
                SHADER_DEBUG_COPY_NORMAL_HISTORY_WGSL,
                &[Float],
                &[wgpu_context.surface_config.format],
                &[],
            ),
            debug_copy_ssr_trace: create(
                "Debug Copy SSR Trace",
                SHADER_DEBUG_COPY_SSR_TRACE_WGSL,
                &[Float],
                &[wgpu_context.surface_config.format],
                &[],
            ),
        }
    }
}

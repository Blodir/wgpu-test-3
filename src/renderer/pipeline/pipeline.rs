use std::{collections::{BTreeSet, HashMap}, rc::Rc};

use wgpu::SurfaceConfiguration;

use crate::{renderer::depth_texture::DepthTexture, renderer::pipeline::shader::{ShaderCache, ShaderCapability}};

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct PipelineConfig {
    pub vertex_buffer_layouts: Vec<wgpu::VertexBufferLayout<'static>>,
    pub bind_group_layouts: Vec<Vec<wgpu::BindGroupLayoutEntry>>,
    pub shader_capabilities: BTreeSet<ShaderCapability>,
}

#[derive(Default)]
pub struct PipelineCache {
    pipelines: HashMap<PipelineConfig, Rc<wgpu::RenderPipeline>>,
}

impl PipelineCache {
    pub fn get_pipeline(
        &mut self, config: &PipelineConfig, global_bind_group_layouts: &Vec<Vec<wgpu::BindGroupLayoutEntry>>,
        device: &wgpu::Device, surface_config: &SurfaceConfiguration, shader_cache: &mut ShaderCache
    ) -> Rc<wgpu::RenderPipeline> {
        self.pipelines.entry(config.clone()).or_insert_with(|| {
            Rc::new(create_pipeline(config, global_bind_group_layouts, device, surface_config, shader_cache))
        }).clone()
    }
}

fn create_pipeline(
    config: &PipelineConfig, global_bind_group_layouts: &Vec<Vec<wgpu::BindGroupLayoutEntry>>,
    device: &wgpu::Device, surface_config: &SurfaceConfiguration, shader_cache: &mut ShaderCache 
) -> wgpu::RenderPipeline {
    let shader_module = shader_cache.get_shader(&config.shader_capabilities, device).get_shader_module_ref();

    let bind_group_layouts: Vec<wgpu::BindGroupLayout> = global_bind_group_layouts.iter().chain(config.bind_group_layouts.iter()).map(|layout| {
        device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: layout,
                label: None, //Some("bind_group_layout_n"),
            }
        )
    }).collect();

    let bind_group_layouts_ref: Vec<&wgpu::BindGroupLayout> = bind_group_layouts.iter().collect();

    let render_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &bind_group_layouts_ref,
            push_constant_ranges: &[],
        });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: "vs_main",
            buffers: &config.vertex_buffer_layouts,
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
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
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
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    });

    render_pipeline
}


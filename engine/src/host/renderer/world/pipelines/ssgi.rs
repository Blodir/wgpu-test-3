use wgpu::util::DeviceExt;

use crate::global_paths::SHADER_SSGI_WGSL;
use crate::host::{
    renderer::SsgiOptions,
    shader_cache::ShaderCache,
    wgpu_context::WgpuContext,
    world::{
        attachments::{
            color::HdrColorTexture,
            deferred::{GBufferTargets, GtaoTexture},
        },
        bindgroups::{
            g_buffer::{GBufferInputs, GBufferInputsBinding},
            gi_source::{GiSourceInputs, GiSourceInputsBinding},
            gtao::{GtaoInputs, GtaoInputsBinding},
        },
    },
};

const INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub struct SsgiPipeline {
    render_pipeline: wgpu::RenderPipeline,
    passthrough_pipeline: wgpu::RenderPipeline,
    index_buffer: wgpu::Buffer,
    gtao_inputs_bind_group_layout: wgpu::BindGroupLayout,
    gtao_inputs_bind_group: GtaoInputsBinding,
    gbuffer_inputs_bind_group_layout: wgpu::BindGroupLayout,
    gbuffer_inputs_bind_group: GBufferInputsBinding,
    gi_source_inputs_bind_group_layout: wgpu::BindGroupLayout,
    gi_source_inputs_bind_group: GiSourceInputsBinding,
}

impl SsgiPipeline {
    fn create_render_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        shader_module: &wgpu::ShaderModule,
        fragment_entry_point: &'static str,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SSGI Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
                entry_point: Some(fragment_entry_point),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState::REPLACE),
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
        })
    }

    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        gbuffer_targets: &GBufferTargets,
        gtao_texture: &GtaoTexture,
        final_color_texture: &HdrColorTexture,
        gi_source_texture: &HdrColorTexture,
        ssgi_options: &SsgiOptions,
    ) -> Self {
        let gtao_inputs_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&GtaoInputs::desc());
        let gtao_inputs_bind_group = GtaoInputs::upload(
            &wgpu_context.device,
            &gtao_inputs_bind_group_layout,
            gtao_texture,
        );

        let gbuffer_inputs_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&GBufferInputs::desc());
        let gbuffer_inputs_bind_group = GBufferInputs::upload(
            &wgpu_context.device,
            &gbuffer_inputs_bind_group_layout,
            gbuffer_targets,
        );

        let gi_source_inputs_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&GiSourceInputs::desc());
        let gi_source_inputs_bind_group = GiSourceInputs::upload(
            &wgpu_context.device,
            &gi_source_inputs_bind_group_layout,
            final_color_texture,
            gi_source_texture,
            ssgi_options,
        );

        let bind_group_layouts = &[
            camera_bind_group_layout,
            &gtao_inputs_bind_group_layout,
            &gbuffer_inputs_bind_group_layout,
            &gi_source_inputs_bind_group_layout,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("SSGI Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_SSGI_WGSL.to_string(), wgpu_context);
        let render_pipeline = Self::create_render_pipeline(
            &wgpu_context.device,
            &render_pipeline_layout,
            &shader_module,
            "fs_main",
        );
        let passthrough_pipeline = Self::create_render_pipeline(
            &wgpu_context.device,
            &render_pipeline_layout,
            &shader_module,
            "fs_passthrough",
        );
        let index_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("SSGI Index Buffer"),
                    contents: bytemuck::cast_slice(INDICES),
                    usage: wgpu::BufferUsages::INDEX,
                });

        Self {
            render_pipeline,
            passthrough_pipeline,
            index_buffer,
            gtao_inputs_bind_group_layout,
            gtao_inputs_bind_group,
            gbuffer_inputs_bind_group_layout,
            gbuffer_inputs_bind_group,
            gi_source_inputs_bind_group_layout,
            gi_source_inputs_bind_group,
        }
    }

    pub fn update_input_bindgroups(
        &mut self,
        device: &wgpu::Device,
        gbuffer_targets: &GBufferTargets,
        gtao_texture: &GtaoTexture,
        final_color_texture: &HdrColorTexture,
        gi_source_texture: &HdrColorTexture,
        ssgi_options: &SsgiOptions,
    ) {
        self.gtao_inputs_bind_group =
            GtaoInputs::upload(device, &self.gtao_inputs_bind_group_layout, gtao_texture);
        self.gbuffer_inputs_bind_group = GBufferInputs::upload(
            device,
            &self.gbuffer_inputs_bind_group_layout,
            gbuffer_targets,
        );
        self.gi_source_inputs_bind_group = GiSourceInputs::upload(
            device,
            &self.gi_source_inputs_bind_group_layout,
            final_color_texture,
            gi_source_texture,
            ssgi_options,
        );
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        hdr_color_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        ssgi_enabled: bool,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SSGI Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: hdr_color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(if ssgi_enabled {
            &self.render_pipeline
        } else {
            &self.passthrough_pipeline
        });
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1u32, &self.gtao_inputs_bind_group.bind_group, &[]);
        render_pass.set_bind_group(2u32, &self.gbuffer_inputs_bind_group.bind_group, &[]);
        render_pass.set_bind_group(3u32, &self.gi_source_inputs_bind_group.bind_group, &[]);
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
    }
}

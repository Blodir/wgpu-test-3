use wgpu::util::DeviceExt;

use crate::renderer::{
    pipelines::resources::{msaa_textures::MSAATextures, skybox_output::SkyboxOutputTexture}, shader_cache::ShaderCache, utils, wgpu_context::{self, WgpuContext}
};

const INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

struct PostProcessingInputs {}
struct PostProcessingInputsBinding {
    bind_group: wgpu::BindGroup,
}
impl PostProcessingInputs {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
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
            ],
            label: Some("Post Processing Inputs Bind Group Layout"),
        }
    }

    pub fn upload(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        skybox_texture: &SkyboxOutputTexture,
        msaa_textures: &MSAATextures,
    ) -> PostProcessingInputsBinding {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&skybox_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&skybox_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &msaa_textures.resolve_texture_view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&msaa_textures.resolve_sampler),
                },
            ],
            label: Some("Post Processing Inputs Bind Group"),
        });

        PostProcessingInputsBinding { bind_group }
    }
}

pub struct PostProcessingPipeline {
    render_pipeline: wgpu::RenderPipeline,
    index_buffer: wgpu::Buffer,
    inputs_binding: PostProcessingInputsBinding,
    inputs_bind_group_layout: wgpu::BindGroupLayout,
}
impl PostProcessingPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        skybox_texture: &SkyboxOutputTexture,
        msaa_textures: &MSAATextures,
    ) -> Self {
        let inputs_bind_group_layout =
            wgpu_context.device.create_bind_group_layout(&PostProcessingInputs::desc());
        let bind_group_layouts = &[&inputs_bind_group_layout];
        let render_pipeline_layout =
            wgpu_context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Post Processing Pipeline Layout"),
                bind_group_layouts,
                push_constant_ranges: &[],
            });
        let shader_module = shader_cache.get("src/renderer/pipelines/shaders/post_processing.wgsl".to_string(), wgpu_context);
        let render_pipeline = wgpu_context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Post Processing Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu_context.surface_config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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
        });

        let index_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let inputs_binding = PostProcessingInputs::upload(
            &wgpu_context.device,
            &inputs_bind_group_layout,
            skybox_texture,
            msaa_textures,
        );

        Self {
            render_pipeline,
            index_buffer,
            inputs_binding,
            inputs_bind_group_layout,
        }
    }

    pub fn update_input_bindgroup(
        &mut self,
        device: &wgpu::Device,
        skybox_texture: &SkyboxOutputTexture,
        msaa_textures: &MSAATextures,
    ) {
        self.inputs_binding = PostProcessingInputs::upload(
            device,
            &self.inputs_bind_group_layout,
            skybox_texture,
            msaa_textures,
        );
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_texture_view: &wgpu::TextureView,
    ) -> Result<(), wgpu::SurfaceError> {
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Post Processing Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0u32, &self.inputs_binding.bind_group, &[]);
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
        }

        Ok(())
    }
}

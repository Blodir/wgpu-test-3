struct InputTexture {}
struct InputTextureBinding {
    bind_group: wgpu::BindGroup,
}
impl InputTexture {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Mipmap Input Texture Bind Group Layout"),
        }
    }

}

impl InputTextureBinding {
    fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Input Texture Bind Group"),
        });

        InputTextureBinding { bind_group }
    }
}

pub struct MipmapPipeline {
    render_pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
}
impl MipmapPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture_bind_group_layout = device.create_bind_group_layout(&InputTexture::desc());
        let bind_group_layouts = &[&texture_bind_group_layout];
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mipmap Pipeline Layout"),
            bind_group_layouts,
            push_constant_ranges: &[],
        });
        let shader_module = crate::renderer::utils::create_shader_module(device, "src/renderer/shaders/mipmap.wgsl");
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mipmap Render Pipeline"),
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
                    format: wgpu::TextureFormat::Rgba8Unorm,
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

        Self { render_pipeline, texture_bind_group_layout }
    }

    pub fn generate_mipmaps(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        mip_level_count: u32,
        face_index: u32,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Mipmap Command Encoder") });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            ..Default::default()
        });

        for mip_level in 1..mip_level_count {
            let source_view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: face_index,
                array_layer_count: Some(1),
                base_mip_level: mip_level - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let target_view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: face_index,
                array_layer_count: Some(1),
                base_mip_level: mip_level,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let source_texture_binding = InputTextureBinding::new(device, &self.texture_bind_group_layout, &source_view, &sampler);

            let render_pass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("Mipmap Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            };

            let mut render_pass = encoder.begin_render_pass(&render_pass_descriptor);

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &source_texture_binding.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        queue.submit(Some(encoder.finish()));
    }
}


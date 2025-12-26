use wgpu::util::DeviceExt;

use crate::renderer::{shader_cache::ShaderCache, utils, wgpu_context::WgpuContext};

const INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub struct SkyboxPipeline {
    render_pipeline: wgpu::RenderPipeline,
    index_buffer: wgpu::Buffer,
}
impl SkyboxPipeline {
    pub fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layouts = &[camera_bind_group_layout, lights_bind_group_layout];
        let render_pipeline_layout =
            wgpu_context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox Pipeline Layout"),
                bind_group_layouts,
                push_constant_ranges: &[],
            });
        let shader_module = shader_cache.get("src/renderer/shaders/skybox.wgsl".to_string(), wgpu_context);
        let render_pipeline = wgpu_context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Render Pipeline"),
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
                    format: wgpu::TextureFormat::Rgba16Float,
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

        Self {
            render_pipeline,
            index_buffer,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_texture_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skybox Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0u32, camera_bind_group, &[]);
        render_pass.set_bind_group(1u32, &lights_bind_group, &[]);
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
    }
}

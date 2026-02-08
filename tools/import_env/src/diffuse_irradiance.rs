use glam::Mat4;
use wgpu::util::DeviceExt;

use super::equirectangular::FaceRotation;

const INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub struct DiffuseIrradiancePipeline {
    render_pipeline: wgpu::RenderPipeline,
}

impl DiffuseIrradiancePipeline {
    pub fn new(
        device: &wgpu::Device,
        face_rot_bind_group_layout: &wgpu::BindGroupLayout,
        environment_map_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layouts = &[
            environment_map_bind_group_layout,
            face_rot_bind_group_layout,
        ];
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Diffuse Irradiance Pipeline Layout"),
                bind_group_layouts,
                push_constant_ranges: &[],
            });
        let shader_module = engine::renderer::utils::create_shader_module(
            device,
            "tools/import_env/src/diffuse_irradiance.wgsl",
        );
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Diffuse Irradiance Render Pipeline"),
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

        Self { render_pipeline }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        environment_map_bind_group: &wgpu::BindGroup,
        face_rot_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<wgpu::Texture, wgpu::SurfaceError> {
        let cubemap_face_resolution = 32;

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = INDICES.len() as u32;

        let cubemap_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Diffuse Irradiance Texture"),
            size: wgpu::Extent3d {
                width: cubemap_face_resolution,
                height: cubemap_face_resolution,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let face_views: Vec<wgpu::TextureView> = (0..6)
            .map(|face_index| {
                cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: face_index,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        let face_rotations: &[Mat4] = &[
            Mat4::from_rotation_y((-90f32).to_radians()), // right
            Mat4::from_rotation_y((90f32).to_radians()),  // left
            Mat4::from_rotation_x((90f32).to_radians()),  // top
            Mat4::from_rotation_x((-90f32).to_radians()), // bottom
            Mat4::IDENTITY,                               // front
            Mat4::from_rotation_y((180f32).to_radians()), // back
        ];

        for face_index in 0..6 {
            let fr: Mat4 = face_rotations[face_index];
            let face_rotation = FaceRotation::from(fr);
            let face_rotation_binding =
                face_rotation.upload(device, queue, &face_rot_bind_group_layout);

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Diffuse Irradiance Render Encoder"),
            });

            let render_pass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("Diffuse Irradiance Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &face_views[face_index],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            };

            {
                let mut render_pass = encoder.begin_render_pass(&render_pass_descriptor);
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, &environment_map_bind_group, &[]);
                render_pass.set_bind_group(1, &face_rotation_binding.bind_group, &[]);
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..num_indices, 0, 0..1);
            }

            queue.submit(Some(encoder.finish()));
        }

        Ok(cubemap_texture)
    }
}

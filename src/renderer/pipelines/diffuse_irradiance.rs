use cgmath::{Deg, Matrix4, SquareMatrix};
use wgpu::util::DeviceExt;

use crate::renderer::renderer::EnvironmentMapBinding;

use super::equirectangular::FaceRotation;

const INDICES: &[u16] = &[
    0, 2, 1,
    3, 2, 0,
];

pub struct DiffuseIrradiancePipeline {
    render_pipeline: wgpu::RenderPipeline,
}

impl DiffuseIrradiancePipeline {
    pub fn new(
        device: &wgpu::Device,
        face_rot_bind_group_layout: &wgpu::BindGroupLayout,
        environment_map_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layouts = &[environment_map_bind_group_layout, face_rot_bind_group_layout];
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Diffuse Irradiance Pipeline Layout"),
            bind_group_layouts,
            push_constant_ranges: &[],
        });
        let shader_module = crate::renderer::utils::create_shader_module(device, "src/renderer/shaders/diffuse_irradiance.wgsl");
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

        Self { render_pipeline }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        environment_map_binding: &EnvironmentMapBinding,
        face_rot_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<wgpu::Texture, wgpu::SurfaceError> {
        let cubemap_face_resolution = 32;

        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            }
        );

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
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
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

        let face_rotations: &[Matrix4<f32>] = &[
            Matrix4::from_angle_y(Deg(-90f32)), // right
            Matrix4::from_angle_y(Deg(90f32)), // left
            Matrix4::from_angle_x(Deg(90f32)), // top
            Matrix4::from_angle_x(Deg(-90f32)), // bottom
            Matrix4::identity(), // front
            Matrix4::from_angle_y(Deg(180f32)), // back
        ];

        for face_index in 0..6 {
            let fr: Matrix4<f32> = face_rotations[face_index];
            let face_rotation = FaceRotation::from(fr);
            let face_rotation_binding = face_rotation.upload(device, queue, &face_rot_bind_group_layout);

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
                render_pass.set_bind_group(0, &environment_map_binding.bind_group, &[]);
                render_pass.set_bind_group(1, &face_rotation_binding.bind_group, &[]);
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..num_indices, 0, 0..1);
            }

            queue.submit(Some(encoder.finish()));
        }

        Ok(cubemap_texture)
    }
}


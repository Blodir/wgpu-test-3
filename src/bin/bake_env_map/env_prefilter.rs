use glam::Mat4;
use wgpu::util::DeviceExt;

use super::equirectangular::FaceRotation;

const INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub struct Roughness {
    roughness: f32,
}

pub struct RoughnessBinding {
    roughness_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl Roughness {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Roughness Bind Group Layout"),
        }
    }

    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> RoughnessBinding {
        let roughness_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Roughness Buffer"),
            contents: bytemuck::cast_slice(&[self.roughness]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: roughness_buffer.as_entire_binding(),
            }],
            label: Some("Roughness Bind Group"),
        });

        RoughnessBinding {
            bind_group,
            roughness_buffer,
        }
    }
}

impl RoughnessBinding {
    pub fn update(&self, roughness: f32, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.roughness_buffer,
            0,
            bytemuck::cast_slice(&[roughness]),
        );
    }
}

pub struct EnvPrefilterPipeline {
    render_pipeline: wgpu::RenderPipeline,
    roughness_bind_group_layout: wgpu::BindGroupLayout,
}

impl EnvPrefilterPipeline {
    pub fn new(
        device: &wgpu::Device,
        face_rot_bind_group_layout: &wgpu::BindGroupLayout,
        environment_map_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let roughness_bind_group_layout = device.create_bind_group_layout(&Roughness::desc());
        let bind_group_layouts = &[
            environment_map_bind_group_layout,
            face_rot_bind_group_layout,
            &roughness_bind_group_layout,
        ];
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Environment Map Prefilter Pipeline Layout"),
                bind_group_layouts,
                push_constant_ranges: &[],
            });
        let shader_module = wgpu_test_3::renderer::utils::create_shader_module(
            device,
            "src/bin/bake_env_map/env_prefilter.wgsl",
        );
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Environment Map Prefilter Render Pipeline"),
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

        Self {
            render_pipeline,
            roughness_bind_group_layout,
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        environment_map: &wgpu::Texture,
        environment_map_binding: &wgpu::BindGroup,
        face_rot_bind_group_layout: &wgpu::BindGroupLayout,
        cubemap_face_resolution: u32,
    ) -> Result<wgpu::Texture, wgpu::SurfaceError> {
        let mipmap_count = 6;

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = INDICES.len() as u32;

        let size = wgpu::Extent3d {
            width: cubemap_face_resolution,
            height: cubemap_face_resolution,
            depth_or_array_layers: 6,
        };
        let cubemap_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Prefiltered Environment Map Texture"),
            size,
            mip_level_count: mipmap_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 0th level is the original environment map itself
        copy_texture_to_texture(device, queue, environment_map, &cubemap_texture, size);

        let mut roughness = 0f32;
        let roughness_binding = {
            let temp = Roughness { roughness };
            temp.upload(device, queue, &self.roughness_bind_group_layout)
        };

        for mip_index in 1..mipmap_count {
            roughness = mip_index as f32 / (mipmap_count as f32 - 1f32);
            roughness_binding.update(roughness, queue);

            let face_views: Vec<wgpu::TextureView> = (0..6)
                .map(|face_index| {
                    cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_mip_level: mip_index,
                        mip_level_count: Some(1),
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
                    label: Some("Environment Map Prefilter Render Encoder"),
                });

                let render_pass_descriptor = wgpu::RenderPassDescriptor {
                    label: Some("Environment Map Prefilter Render Pass"),
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
                    render_pass.set_bind_group(0, &environment_map_binding, &[]);
                    render_pass.set_bind_group(1, &face_rotation_binding.bind_group, &[]);
                    render_pass.set_bind_group(2, &roughness_binding.bind_group, &[]);
                    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    render_pass.draw_indexed(0..num_indices, 0, 0..1);
                }

                queue.submit(Some(encoder.finish()));
            }
        }

        Ok(cubemap_texture)
    }
}

fn copy_texture_to_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    src_texture: &wgpu::Texture,
    dst_texture: &wgpu::Texture,
    texture_size: wgpu::Extent3d,
) {
    // Create a command encoder
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Texture Copy Encoder"),
    });

    // Define the source texture copy parameters
    let src_copy = wgpu::ImageCopyTexture {
        texture: src_texture,
        mip_level: 0,                     // Mip level to copy from
        origin: wgpu::Origin3d::ZERO,     // Start at the origin of the source texture
        aspect: wgpu::TextureAspect::All, // Copy all aspects (depth, stencil, color)
    };

    // Define the destination texture copy parameters
    let dst_copy = wgpu::ImageCopyTexture {
        texture: dst_texture,
        mip_level: 0,                     // Mip level to copy to
        origin: wgpu::Origin3d::ZERO,     // Start at the origin of the destination texture
        aspect: wgpu::TextureAspect::All, // Copy all aspects (depth, stencil, color)
    };

    // Define the size of the texture to copy
    let copy_size = texture_size;

    // Record the texture copy command
    encoder.copy_texture_to_texture(src_copy, dst_copy, copy_size);

    // Submit the command encoder
    queue.submit(Some(encoder.finish()));
}

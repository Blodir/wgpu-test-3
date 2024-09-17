use std::{fs::File, io::{self, Read}};

use cgmath::{Deg, Matrix4, SquareMatrix};
use wgpu::util::DeviceExt as _;

use crate::renderer::{pbr::SamplerOptions, texture::Texture};

use super::mipmap::MipmapPipeline;

struct EquirectangularHdrEnvironmentMap {
    map: (image::DynamicImage, Option<SamplerOptions>),
}

struct EquirectangularHdrEnvironmentMapBinding {
    bind_group: wgpu::BindGroup,
}

impl EquirectangularHdrEnvironmentMap {
    fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false},
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
            label: Some("EquirectangularHdrEnvironmentMap Bind Group Layout"),
        }
    }

    fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue, bind_group_layout: &wgpu::BindGroupLayout) -> EquirectangularHdrEnvironmentMapBinding {
        let texture = Texture::from_image(device, queue, &self.map, false);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            min_filter: wgpu::FilterMode::Nearest,
            mag_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("EquirectangularHdrEnvironmentMap Bind Group"),
        });

        EquirectangularHdrEnvironmentMapBinding { bind_group }
    }
}

pub struct FaceRotation {
    m4: [[f32; 4]; 4],
}

pub struct FaceRotationBinding {
    pub bind_group: wgpu::BindGroup,
}

impl FaceRotation {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("FaceRotation Bind Group Layout"),
        }
    }

    pub fn from(mat4: Matrix4<f32>) -> Self {
        Self {
            m4: mat4.into(),
        }
    }

    pub fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue, bind_group_layout: &wgpu::BindGroupLayout) -> FaceRotationBinding {
        let buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("FaceRotation Buffer"),
                contents: bytemuck::cast_slice(&self.m4),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
            ],
            label: Some("FaceRotation Bind Group"),
        });

        FaceRotationBinding { bind_group }
    }
}

struct EquirectangularReaderPipeline {
    render_pipeline: wgpu::RenderPipeline
}
impl EquirectangularReaderPipeline {
    fn new(
        device: &wgpu::Device,
        equirectangular_bind_group_layout: &wgpu::BindGroupLayout,
        face_rotation_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> EquirectangularReaderPipeline {
        let bind_group_layouts = &[equirectangular_bind_group_layout, face_rotation_bind_group_layout];
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("IrradiancePreprocessing Pipeline Layout"),
            bind_group_layouts,
            push_constant_ranges: &[],
        });
        let shader_module = crate::renderer::utils::create_shader_module(device, "src/renderer/shaders/equirectangular.wgsl");
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Equirectangular Map Render Pipeline"),
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
}

const INDICES: &[u16] = &[
    0, 2, 1,
    3, 2, 0,
];

pub fn render_cubemap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: image::DynamicImage,
) -> io::Result<wgpu::Texture> {
    let cubemap_face_resolution = image.height() / 2;

    let mipmap_pipeline = MipmapPipeline::new(device);
    let mip_level_count = 5;

    let eem_bind_group_layout = device.create_bind_group_layout(&EquirectangularHdrEnvironmentMap::desc());
    let equirectangular_environment_map = EquirectangularHdrEnvironmentMap { map: (image, Some(SamplerOptions {
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
    })) };
    let equirectangular_environment_map_binding = equirectangular_environment_map.upload(
        device, queue, &eem_bind_group_layout
    );

    let fr_bind_group_layout = device.create_bind_group_layout(&FaceRotation::desc());
    
    let pipeline = EquirectangularReaderPipeline::new(device, &eem_bind_group_layout, &fr_bind_group_layout);

    let index_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        }
    );

    let num_indices = INDICES.len() as u32;

    let cubemap_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Cubemap Texture"),
        size: wgpu::Extent3d {
            width: cubemap_face_resolution,
            height: cubemap_face_resolution,
            depth_or_array_layers: 6,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let face_views: Vec<wgpu::TextureView> = (0..6)
        .map(|face_index| {
            cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: face_index,
                array_layer_count: Some(1),
                base_mip_level: 0,
                mip_level_count: Some(1),
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
        let face_rotation_binding = face_rotation.upload(device, queue, &fr_bind_group_layout);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Cubemap Render Encoder"),
        });

        let render_pass_descriptor = wgpu::RenderPassDescriptor {
            label: Some("Cubemap Render Pass"),
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
            render_pass.set_pipeline(&pipeline.render_pipeline);
            render_pass.set_bind_group(0, &equirectangular_environment_map_binding.bind_group, &[]);
            render_pass.set_bind_group(1, &face_rotation_binding.bind_group, &[]);
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..num_indices, 0, 0..1);
        }

        queue.submit(Some(encoder.finish()));

        mipmap_pipeline.generate_mipmaps(device, queue, &cubemap_texture, mip_level_count, face_index as u32);
    }

    /*
    write_texture_to_file(device, queue, &cubemap_texture, 0);
    write_texture_to_file(device, queue, &cubemap_texture, 1);
    write_texture_to_file(device, queue, &cubemap_texture, 2);
    write_texture_to_file(device, queue, &cubemap_texture, 3);
    write_texture_to_file(device, queue, &cubemap_texture, 4);
    write_texture_to_file(device, queue, &cubemap_texture, 5);
    */
    //write_texture_to_file(device, queue, &cubemap_texture, 0, 0);

    Ok(cubemap_texture)
}

// for testing:
pub fn write_texture_to_file(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, face_index: u32, mip_level: u32) {
    let cubemap_face_resolution = texture.width();
    let mip_resolution = (cubemap_face_resolution >> mip_level).max(1);
    // Get the texture from the GPU and write it to a file
    let buffer_size = (mip_resolution * mip_resolution * 4) as wgpu::BufferAddress;
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Command encoder to copy texture to the buffer
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Copy Texture to Buffer Encoder"),
    });

    // Define the copy operation
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: face_index,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(mip_resolution * 4),
                rows_per_image: Some(mip_resolution),
            },
        },
        wgpu::Extent3d {
            width: mip_resolution,
            height: mip_resolution,
            depth_or_array_layers: 1,
        },
    );

    // Submit the command encoder
    queue.submit(Some(encoder.finish()));

    // Map the buffer to get access to its content
    let buffer_slice = staging_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |result| {
        assert!(result.is_ok());
    });

    // Wait for the mapping to complete
    device.poll(wgpu::Maintain::Wait);

    let data = buffer_slice.get_mapped_range();
    let image_data: Vec<u8> = data.to_vec(); // This is the raw pixel data (RGBA8)
    drop(data); // Unmap the buffer

    let img_buffer: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(mip_resolution, mip_resolution, image_data).expect("Failed to create ImageBuffer");

    // Save the image
    convert_rgba8_to_rgb32f(img_buffer).save(format!("cubemap_face_{face_index}.hdr")).expect("Failed to save image");
}

fn convert_rgba8_to_rgb32f(
    img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>
) -> image::ImageBuffer<image::Rgb<f32>, Vec<f32>> {
    let (width, height) = img.dimensions();

    // Create a vector to store RGB32F data (3 floats per pixel)
    let mut rgb32f_data: Vec<f32> = Vec::with_capacity((width * height * 3) as usize);

    for pixel in img.pixels() {
        let rgba = pixel.0;
        // Convert each channel from u8 (0-255) to f32 (0.0-1.0)
        let r = rgba[0] as f32 / 255.0;
        let g = rgba[1] as f32 / 255.0;
        let b = rgba[2] as f32 / 255.0;
        rgb32f_data.push(r);
        rgb32f_data.push(g);
        rgb32f_data.push(b);
    }

    // Return a new ImageBuffer with the RGB<f32> color type
    image::ImageBuffer::from_raw(width, height, rgb32f_data).expect("Failed to create RGB32F ImageBuffer")
}


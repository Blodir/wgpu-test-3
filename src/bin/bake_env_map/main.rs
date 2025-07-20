use ddsfile::{Caps2, Dds, DxgiFormat, NewDxgiParams};
use image::ImageReader;
use pollster::FutureExt as _;
use std::path::{Path, PathBuf};
use std::{env, fs::File};
use wgpu::{Extent3d, ImageDataLayout, TextureAspect};

use crate::diffuse_irradiance::DiffuseIrradiancePipeline;
use crate::env_prefilter::EnvPrefilterPipeline;
use crate::equirectangular::{render_cubemap, FaceRotation};

mod diffuse_irradiance;
mod env_prefilter;
mod equirectangular;
mod mipmap;

fn copy_texture_to_buffer(
    texture: &wgpu::Texture,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<Vec<u8>> {
    let cubemap_face_resolution = texture.width();
    let bytes_per_pixel = 8; // for Rgba16
    let mut mip_data = Vec::new();

    for face in 0..6 {
        for mip_level in 0..texture.mip_level_count() {
            let mip_resolution = (cubemap_face_resolution >> mip_level).max(1);
            let mip_height = mip_resolution;
            let mip_width = mip_resolution;
            let mip_size = mip_width.max(1) * mip_height.max(1) * bytes_per_pixel;

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Cubemap Render Encoder"),
            });

            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: mip_size as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: face,
                    },
                    aspect: TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &buffer,
                    layout: ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_pixel * mip_width.max(1)),
                        rows_per_image: Some(mip_height.max(1)),
                    },
                },
                Extent3d {
                    width: mip_width.max(1),
                    height: mip_height.max(1),
                    depth_or_array_layers: 1,
                },
            );

            // Submit the command
            queue.submit(Some(encoder.finish()));

            // Map and read buffer
            {
                let buffer_slice = buffer.slice(..);
                buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
                device.poll(wgpu::Maintain::Wait);
                let data = buffer_slice.get_mapped_range().to_vec();
                mip_data.push(data);
                drop(buffer_slice);
                buffer.unmap();
            }
        }
    }
    mip_data
}

fn write_texture_to_file(
    texture: &wgpu::Texture,
    output_path: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let data = copy_texture_to_buffer(&texture, device, queue);
    let mut dds = Dds::new_dxgi(NewDxgiParams {
        height: texture.height(),
        width: texture.width(),
        depth: None,
        format: DxgiFormat::R16G16B16A16_Float,
        mipmap_levels: Some(texture.mip_level_count()),
        array_layers: Some(6),
        caps2: Some(Caps2::CUBEMAP | Caps2::CUBEMAP_ALLFACES),
        is_cubemap: true,
        resource_dimension: ddsfile::D3D10ResourceDimension::Texture2D,
        alpha_mode: ddsfile::AlphaMode::Straight,
    })
    .unwrap();

    dds.data = data.into_iter().flatten().collect();
    let mut file = File::create(output_path).unwrap();
    dds.write(&mut file).unwrap();
}

async fn run() {
    let args: Vec<String> = env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("assets/kloofendal_overcast_puresky_8k.hdr");
    let hdr = ImageReader::open(path)
        .expect("Failed to open environment map")
        .decode()
        .expect("Failed to decode environment map");

    let instance = wgpu::Instance::default();
    let adapter = instance.request_adapter(&Default::default()).await.unwrap();
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
            },
            None,
        )
        .await
        .unwrap();

    let cubemap = render_cubemap(&device, &queue, hdr).unwrap();

    let cubemap_view = cubemap.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let env_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Cubemap Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge, // Ensure texture coordinates are clamped across all cubemap faces
        mag_filter: wgpu::FilterMode::Linear,           // Smooth magnification
        min_filter: wgpu::FilterMode::Linear,           // Smooth minification
        mipmap_filter: wgpu::FilterMode::Linear, // Smooth mipmap transition if mipmaps are used
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0, // High value to cover any mipmap range
        compare: None,        // Not typically used for cubemaps unless needed for specific effects
        anisotropy_clamp: 1,  // Optionally enable anisotropic filtering (e.g., Some(16))
        border_color: None,   // Only relevant if using ClampToBorder
    });
    let temp_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
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
            ],
            label: Some("Environment Map Bind Group Layout"),
        });

    let temp_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Environment Cubemap Bind Group"),
        layout: &temp_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&cubemap_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&env_map_sampler),
            },
        ],
    });

    // render prefiltered_env_map
    let face_rot_bind_group_layout = device.create_bind_group_layout(&FaceRotation::desc());
    let pipeline = EnvPrefilterPipeline::new(
        &device,
        &face_rot_bind_group_layout,
        &temp_bind_group_layout,
    );
    let prefiltered_tex = {
        let resolution = cubemap.width();
        pipeline
            .render(
                &device,
                &queue,
                &cubemap,
                &temp_bind_group,
                &face_rot_bind_group_layout,
                resolution,
            )
            .unwrap()
    };

    // render diffuse_irradiance
    let di_tex = {
        let pipeline = DiffuseIrradiancePipeline::new(
            &device,
            &face_rot_bind_group_layout,
            &temp_bind_group_layout,
        );
        pipeline
            .render(
                &device,
                &queue,
                &temp_bind_group,
                &face_rot_bind_group_layout,
            )
            .unwrap()
    };

    let output_path = wgpu_test_3::strip_extension(path);

    // write prefiltered_env_map
    write_texture_to_file(
        &prefiltered_tex,
        &(output_path.clone() + ".prefiltered.dds"),
        &device,
        &queue,
    );

    // write diffuse_irradiance
    write_texture_to_file(&di_tex, &(output_path + ".di.dds"), &device, &queue);
}

fn main() {
    run().block_on();
}

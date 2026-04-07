use wgpu::util::DeviceExt as _;

use crate::host::renderer::SsgiOptions;
use crate::host::world::attachments::color::HdrColorTexture;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SsgiSettingsUniform {
    params0: [f32; 4],
    params1: [f32; 4],
}

pub struct SsgiResolveInputs {}
pub struct SsgiResolveInputsBinding {
    pub bind_group: wgpu::BindGroup,
    _ssgi_settings_buffer: wgpu::Buffer,
}

impl SsgiResolveInputs {
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
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("SSGI Resolve Inputs Bind Group Layout"),
        }
    }

    pub fn upload(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        final_color_texture: &HdrColorTexture,
        ssgi_indirect_texture: &HdrColorTexture,
        ssgi_options: &SsgiOptions,
    ) -> SsgiResolveInputsBinding {
        let ssgi_settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSGI Resolve Settings Buffer"),
            contents: bytemuck::bytes_of(&SsgiSettingsUniform {
                params0: [
                    ssgi_options.radius_pixels,
                    ssgi_options.world_radius,
                    ssgi_options.thickness,
                    ssgi_options.depth_reject_scale,
                ],
                params1: [ssgi_options.intensity, 0.0, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&final_color_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&final_color_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: ssgi_settings_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&ssgi_indirect_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&ssgi_indirect_texture.sampler),
                },
            ],
            label: Some("SSGI Resolve Inputs Bind Group"),
        });

        SsgiResolveInputsBinding {
            bind_group,
            _ssgi_settings_buffer: ssgi_settings_buffer,
        }
    }
}

use crate::host::world::attachments::color::HdrColorTexture;

pub struct SsgiResolveInputs {}
pub struct SsgiResolveInputsBinding {
    pub bind_group: wgpu::BindGroup,
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
    ) -> SsgiResolveInputsBinding {
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

        SsgiResolveInputsBinding { bind_group }
    }
}

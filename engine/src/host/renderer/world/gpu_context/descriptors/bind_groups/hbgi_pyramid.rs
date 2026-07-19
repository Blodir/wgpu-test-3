use super::super::bg_layouts::BGLayouts;
use crate::host::renderer::world::gpu_context::resources::MipPyramidTextureViews;

pub(crate) struct HbgiPyramidBindGroups {
    pub base: wgpu::BindGroup,
    pub downsample: Vec<wgpu::BindGroup>,
}
impl HbgiPyramidBindGroups {
    pub fn base_desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
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
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("HBGI Pyramid Base Bind Group Layout"),
        }
    }

    pub fn downsample_desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("HBGI Pyramid Downsample Bind Group Layout"),
        }
    }

    fn create_base_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        source_hbgi_view: &wgpu::TextureView,
        source_hbgi_sampler: &wgpu::Sampler,
        source_depth_view: &wgpu::TextureView,
        source_normal_view: &wgpu::TextureView,
        source_normal_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_hbgi_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(source_hbgi_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(source_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(source_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(source_normal_sampler),
                },
            ],
            label: Some("HBGI Pyramid Base Bind Group"),
        })
    }

    fn create_downsample_bind_groups(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        downsample_sampler: &wgpu::Sampler,
        pyramids: &MipPyramidTextureViews,
    ) -> Vec<wgpu::BindGroup> {
        (1..pyramids.diffuse_radiance_ao_mips.len())
            .map(|dst_mip| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramids.diffuse_radiance_ao_mips[dst_mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramids.depth_mips[dst_mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramids.normal_mips[dst_mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::Sampler(downsample_sampler),
                        },
                    ],
                    label: Some("HBGI Pyramid Downsample Bind Group"),
                })
            })
            .collect()
    }

    pub fn new(
        device: &wgpu::Device,
        layouts: &BGLayouts,
        downsample_sampler: &wgpu::Sampler,
        source_hbgi_view: &wgpu::TextureView,
        source_hbgi_sampler: &wgpu::Sampler,
        source_depth_view: &wgpu::TextureView,
        source_normal_view: &wgpu::TextureView,
        source_normal_sampler: &wgpu::Sampler,
        pyramids: &MipPyramidTextureViews,
    ) -> Self {
        let base = Self::create_base_bind_group(
            device,
            &layouts.hbgi_pyramid_base,
            source_hbgi_view,
            source_hbgi_sampler,
            source_depth_view,
            source_normal_view,
            source_normal_sampler,
        );
        let downsample = Self::create_downsample_bind_groups(
            device,
            &layouts.hbgi_pyramid_downsample,
            downsample_sampler,
            pyramids,
        );

        Self { base, downsample }
    }
}

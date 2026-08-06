pub(crate) struct SsrBindGroup(pub(crate) wgpu::BindGroup);

impl SsrBindGroup {
    pub(crate) fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("SSR Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        depth_pyramid_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        lighting_view: &wgpu::TextureView,
        lighting_sampler: &wgpu::Sampler,
        environment_map_intensity: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth_pyramid_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normal_roughness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(normal_roughness_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(lighting_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(lighting_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: environment_map_intensity.as_entire_binding(),
                },
            ],
            label: Some("SSR Inputs Bind Group"),
        })
    }

    pub(crate) fn new(
        depth_pyramid_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        lighting_view: &wgpu::TextureView,
        lighting_sampler: &wgpu::Sampler,
        environment_map_intensity: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self(Self::create_bind_group(
            depth_pyramid_view,
            normal_roughness_view,
            normal_roughness_sampler,
            lighting_view,
            lighting_sampler,
            environment_map_intensity,
            layout,
            device,
        ))
    }
}

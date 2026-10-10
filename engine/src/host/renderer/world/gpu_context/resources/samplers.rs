pub(crate) struct Samplers {
    pub(crate) linear: wgpu::Sampler,
    pub(crate) comparison: wgpu::Sampler,
}
impl Samplers {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let comparison = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Comparison Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self { linear, comparison }
    }
}

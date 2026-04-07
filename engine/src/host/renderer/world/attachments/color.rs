pub struct HdrColorTexture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl HdrColorTexture {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        Self::new_scaled(device, surface_config, 1, "HDR Color Texture")
    }

    pub fn new_half_res(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        Self::new_scaled(
            device,
            surface_config,
            2,
            "Half Resolution HDR Color Texture",
        )
    }

    fn new_scaled(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        scale_divisor: u32,
        label: &'static str,
    ) -> Self {
        let hdr_format = wgpu::TextureFormat::Rgba16Float;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: (surface_config.width / scale_divisor).max(1),
                height: (surface_config.height / scale_divisor).max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        Self {
            texture,
            view,
            sampler,
        }
    }
}

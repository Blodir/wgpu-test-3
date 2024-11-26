pub struct MSAATextures {
    msaa_texture: wgpu::Texture,
    pub msaa_texture_view: wgpu::TextureView,
    pub msaa_sampler: wgpu::Sampler,
    resolve_texture: wgpu::Texture,
    pub resolve_texture_view: wgpu::TextureView,
    pub resolve_sampler: wgpu::Sampler,
}

impl MSAATextures {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let msaa_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA Texture"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_texture_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let msaa_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        let resolve_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA Resolve Texture"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let resolve_texture_view = resolve_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let resolve_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        Self { 
            msaa_texture, msaa_texture_view, resolve_texture, resolve_texture_view, msaa_sampler, resolve_sampler
        }
    }
}


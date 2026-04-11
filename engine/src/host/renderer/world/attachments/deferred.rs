pub struct GBufferTexture {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl GBufferTexture {
    fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        Self {
            _texture: texture,
            view,
            sampler,
        }
    }
}

pub struct GBufferTargets {
    pub albedo_ao: GBufferTexture,
    pub normal_roughness: GBufferTexture,
    pub emissive_metallic: GBufferTexture,
    pub world_position: GBufferTexture,
}

impl GBufferTargets {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        Self {
            albedo_ao: GBufferTexture::new(device, surface_config, "GBuffer AlbedoAO"),
            normal_roughness: GBufferTexture::new(
                device,
                surface_config,
                "GBuffer NormalRoughness",
            ),
            emissive_metallic: GBufferTexture::new(
                device,
                surface_config,
                "GBuffer EmissiveMetallic",
            ),
            world_position: GBufferTexture::new(device, surface_config, "GBuffer WorldPosition"),
        }
    }
}

pub struct GtaoTexture {
    _ao_texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    _hbil_diffuse_texture: wgpu::Texture,
    pub hbil_diffuse_view: wgpu::TextureView,
    pub hbil_diffuse_sampler: wgpu::Sampler,
}

impl GtaoTexture {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        Self::new_scaled(device, surface_config, 2, "Half Resolution GTAO Texture")
    }

    fn new_scaled(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        scale_divisor: u32,
        label: &'static str,
    ) -> Self {
        let ao_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: (surface_config.width / scale_divisor).max(1),
                height: (surface_config.height / scale_divisor).max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = ao_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let hbil_diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Half Resolution HBIL Diffuse Irradiance Texture"),
            size: wgpu::Extent3d {
                width: (surface_config.width / scale_divisor).max(1),
                height: (surface_config.height / scale_divisor).max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hbil_diffuse_view =
            hbil_diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let hbil_diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            _ao_texture: ao_texture,
            view,
            sampler,
            _hbil_diffuse_texture: hbil_diffuse_texture,
            hbil_diffuse_view,
            hbil_diffuse_sampler,
        }
    }
}

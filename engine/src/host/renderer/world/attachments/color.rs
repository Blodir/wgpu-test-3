pub struct HdrColorTexture {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampled_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    mip_views: Vec<wgpu::TextureView>,
}

impl HdrColorTexture {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        Self::new_scaled(device, surface_config, 1, "HDR Color Texture", false)
    }

    pub fn new_mipmapped(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        label: &'static str,
    ) -> Self {
        Self::new_scaled(device, surface_config, 1, label, true)
    }

    pub fn new_half_res(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        Self::new_scaled(device, surface_config, 2, "Half Resolution HDR Color Texture", false)
    }

    fn new_scaled(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        scale_divisor: u32,
        label: &'static str,
        mipmapped: bool,
    ) -> Self {
        let hdr_format = wgpu::TextureFormat::Rgba16Float;
        let width = (surface_config.width / scale_divisor).max(1);
        let height = (surface_config.height / scale_divisor).max(1);
        let mip_level_count = if mipmapped {
            width.max(height).ilog2() + 1
        } else {
            1
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(label),
            format: Some(hdr_format),
            dimension: Some(wgpu::TextureViewDimension::D2),
            usage: Some(
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(1),
        });
        let sampled_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = if mipmapped {
            device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            })
        } else {
            device.create_sampler(&wgpu::SamplerDescriptor::default())
        };
        let mip_views = (0..mip_level_count)
            .map(|mip_level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(label),
                    format: Some(hdr_format),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Some(
                        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    ),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: mip_level,
                    mip_level_count: Some(1),
                    base_array_layer: 0,
                    array_layer_count: Some(1),
                })
            })
            .collect();

        Self {
            _texture: texture,
            view,
            sampled_view,
            sampler,
            mip_views,
        }
    }

    pub fn mip_level_count(&self) -> u32 {
        self.mip_views.len() as u32
    }

    pub fn mip_view(&self, mip_level: u32) -> &wgpu::TextureView {
        &self.mip_views[mip_level as usize]
    }
}

use super::color::HdrColorTexture;

pub struct FloatPyramidTexture {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    mip_views: Vec<wgpu::TextureView>,
}

impl FloatPyramidTexture {
    pub fn new_mipmapped(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        label: &'static str,
    ) -> Self {
        let width = surface_config.width.max(1);
        let height = surface_config.height.max(1);
        let mip_level_count = width.max(height).ilog2() + 1;
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
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mip_views = (0..mip_level_count)
            .map(|mip_level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(label),
                    format: Some(wgpu::TextureFormat::R32Float),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Some(
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
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
            mip_views,
        }
    }

    pub fn mip_view(&self, mip_level: u32) -> &wgpu::TextureView {
        &self.mip_views[mip_level as usize]
    }
}

pub struct HbgiPyramidTextures {
    pub hbgi: HdrColorTexture,
    pub depth: FloatPyramidTexture,
    pub normal: HdrColorTexture,
}

impl HbgiPyramidTextures {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        Self {
            hbgi: HdrColorTexture::new_mipmapped(device, surface_config, "HBGI Pyramid Texture"),
            depth: FloatPyramidTexture::new_mipmapped(
                device,
                surface_config,
                "HBGI Depth Pyramid Texture",
            ),
            normal: HdrColorTexture::new_mipmapped(
                device,
                surface_config,
                "HBGI Normal Pyramid Texture",
            ),
        }
    }

    pub fn mip_level_count(&self) -> u32 {
        self.hbgi.mip_level_count()
    }
}

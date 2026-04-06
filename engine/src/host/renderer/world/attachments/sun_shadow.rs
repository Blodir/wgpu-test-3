pub struct SunShadowTexture {
    texture: wgpu::Texture,
    pub array_view: wgpu::TextureView,
    pub cascade_views: Vec<wgpu::TextureView>,
}

impl SunShadowTexture {
    pub const SHADOW_MAP_SIZE: u32 = crate::host::world::sun_shadow::SUN_SHADOW_MAP_SIZE;
    pub const CASCADE_COUNT: u32 =
        crate::host::world::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT as u32;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Sun Shadow Map"),
            size: wgpu::Extent3d {
                width: Self::SHADOW_MAP_SIZE,
                height: Self::SHADOW_MAP_SIZE,
                depth_or_array_layers: Self::CASCADE_COUNT,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sun Shadow Map Array View"),
            format: Some(Self::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            usage: Some(
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(Self::CASCADE_COUNT),
        });
        let cascade_views = (0..Self::CASCADE_COUNT)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Sun Shadow Map Cascade View"),
                    format: Some(Self::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                })
            })
            .collect();
        Self {
            texture,
            array_view: view,
            cascade_views,
        }
    }
}

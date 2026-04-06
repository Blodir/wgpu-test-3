pub struct SunShadowTexture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl SunShadowTexture {
    pub const SHADOW_MAP_SIZE: u32 = 2048;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Sun Shadow Map"),
            size: wgpu::Extent3d {
                width: Self::SHADOW_MAP_SIZE,
                height: Self::SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}

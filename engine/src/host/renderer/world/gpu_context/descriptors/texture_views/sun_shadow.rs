use crate::host::renderer::world::gpu_context::resources::SunShadowTexture2;

pub struct SunShadowTextureViews {
    pub array: wgpu::TextureView,
    pub cascades: Vec<wgpu::TextureView>,
}
impl SunShadowTextureViews {
    pub fn new(texture: &wgpu::Texture) -> Self {
        let array = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sun Shadow Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });

        let cascades = (0..SunShadowTexture2::CASCADE_COUNT)
            .map(|idx| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("Sun Shadow Cascade {idx} Texture View")),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    base_array_layer: idx,
                    array_layer_count: Some(1),
                })
            })
            .collect();

        Self { array, cascades }
    }
}

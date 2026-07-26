use crate::host::renderer::world::gpu_context::resources::HbgiSvgfTextures;

pub struct HbgiSvgfTextureViews {
    pub bent_ao_a: wgpu::TextureView,
    pub bent_ao_b: wgpu::TextureView,
    pub irradiance_variance_a: wgpu::TextureView,
    pub irradiance_variance_b: wgpu::TextureView,
}

impl HbgiSvgfTextureViews {
    pub fn new(textures: &HbgiSvgfTextures) -> Self {
        let make_view = |texture: &wgpu::Texture, label: &'static str| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(label),
                ..wgpu::TextureViewDescriptor::default()
            })
        };

        Self {
            bent_ao_a: make_view(
                &textures.bent_ao_a,
                "HBGI SVGF: Bent Normal + AO Texture View A",
            ),
            bent_ao_b: make_view(
                &textures.bent_ao_b,
                "HBGI SVGF: Bent Normal + AO Texture View B",
            ),
            irradiance_variance_a: make_view(
                &textures.irradiance_variance_a,
                "HBGI SVGF: Irradiance + Variance Texture View A",
            ),
            irradiance_variance_b: make_view(
                &textures.irradiance_variance_b,
                "HBGI SVGF: Irradiance + Variance Texture View B",
            ),
        }
    }
}

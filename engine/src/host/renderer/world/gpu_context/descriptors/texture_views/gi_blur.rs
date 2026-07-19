use crate::host::renderer::world::gpu_context::resources::GiBlurTextures;

pub struct GiBlurTextureViews {
    pub bent_ao: wgpu::TextureView,
    pub near_field_irradiance: wgpu::TextureView,
}
impl GiBlurTextureViews {
    pub fn new(textures: &GiBlurTextures) -> Self {
        let bent_ao = textures.bent_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("GI Blur: Bent Normals + AO Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let near_field_irradiance =
            textures
                .near_field_irradiance
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("GI Blur: Near Field Irradiance Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });

        Self {
            bent_ao,
            near_field_irradiance,
        }
    }
}

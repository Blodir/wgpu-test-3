use crate::host::renderer::world::gpu_context::resources::HbgiTextures;

pub struct HbgiTextureViews {
    pub bent_ao: wgpu::TextureView,
    pub near_field_irradiance: wgpu::TextureView,
}
impl HbgiTextureViews {
    pub fn new(textures: &HbgiTextures) -> Self {
        let bent_ao = textures.bent_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HBGI: Bent Normals + AO Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let near_field_irradiance =
            textures
                .near_field_irradiance
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("HBGI: Near Field Irradiance Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });

        Self {
            bent_ao,
            near_field_irradiance,
        }
    }
}

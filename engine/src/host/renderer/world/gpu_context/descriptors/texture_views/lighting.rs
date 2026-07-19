use crate::host::renderer::world::gpu_context::resources::LightingTextures;

pub(crate) struct LightingTextureViews {
    pub(crate) lit_hdr: wgpu::TextureView,
    pub(crate) diffuse_radiance_ao: wgpu::TextureView,
}
impl LightingTextureViews {
    pub(crate) fn new(textures: &LightingTextures) -> Self {
        let lit_hdr = textures.lit_hdr.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Lighting: Lit HDR Texture View"),
            ..Default::default()
        });
        let diffuse_radiance_ao =
            textures
                .diffuse_radiance_ao
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Lighting: Diffuse Radiance + AO Texture View"),
                    ..Default::default()
                });

        Self {
            lit_hdr,
            diffuse_radiance_ao,
        }
    }
}

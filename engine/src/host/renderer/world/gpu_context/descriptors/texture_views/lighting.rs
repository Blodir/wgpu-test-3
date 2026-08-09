use crate::host::renderer::world::gpu_context::resources::LightingTextures;

pub(crate) struct LightingTextureViews {
    pub(crate) lit_hdr: wgpu::TextureView,
    pub(crate) diffuse_radiance_ao: wgpu::TextureView,
    pub(crate) diffuse_radiance_ao_mips: Vec<wgpu::TextureView>,
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
        let diffuse_radiance_ao_mips = (0..textures.mip_level_count)
            .map(|mip_level| {
                textures
                    .diffuse_radiance_ao
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!(
                            "Lighting: Diffuse Radiance + AO Mip {mip_level} Texture View"
                        )),
                        format: None,
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
            lit_hdr,
            diffuse_radiance_ao,
            diffuse_radiance_ao_mips,
        }
    }
}

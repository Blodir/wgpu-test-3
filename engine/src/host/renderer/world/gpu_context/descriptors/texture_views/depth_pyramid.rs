use crate::host::renderer::world::gpu_context::resources::textures::DepthPyramidTextures;

pub(crate) struct DepthPyramidTextureViews {
    pub all_mips: wgpu::TextureView,
    pub mips: Vec<wgpu::TextureView>,
}

impl DepthPyramidTextureViews {
    pub fn new(textures: &DepthPyramidTextures) -> Self {
        let all_mips = textures.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth Pyramid All Mips"),
            mip_level_count: Some(textures.mip_level_count),
            ..Default::default()
        });
        let mips = (0..textures.mip_level_count)
            .map(|mip_level| {
                textures.texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Depth Pyramid Mip"),
                    base_mip_level: mip_level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self { all_mips, mips }
    }
}

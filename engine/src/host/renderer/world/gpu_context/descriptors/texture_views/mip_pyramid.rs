use crate::host::renderer::world::gpu_context::resources::MipPyramidTextures;

pub struct MipPyramidTextureViews {
    pub diffuse_radiance_ao_pyramid: wgpu::TextureView,
    pub diffuse_radiance_ao_mips: Vec<wgpu::TextureView>,
    pub depth_pyramid: wgpu::TextureView,
    pub depth_mips: Vec<wgpu::TextureView>,
    pub normal_pyramid: wgpu::TextureView,
    pub normal_mips: Vec<wgpu::TextureView>,
}
impl MipPyramidTextureViews {
    pub fn new(textures: &MipPyramidTextures) -> Self {
        let mip_level_count = textures.mip_level_count;
        let diffuse_radiance_ao_pyramid =
            textures
                .diffuse_radiance_ao_pyramid
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Diffuse Radiance + AO Pyramid Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });
        let diffuse_radiance_ao_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures
                    .diffuse_radiance_ao_pyramid
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!(
                            "Diffuse Radiance + AO Pyramid Mip {mip_level} Texture View"
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

        let depth_pyramid = textures
            .depth_pyramid
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Depth Pyramid Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let depth_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures
                    .depth_pyramid
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!("Depth Pyramid Mip {mip_level} Texture View")),
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

        let normal_pyramid = textures
            .normal_pyramid
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Normal Pyramid Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let normal_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures
                    .normal_pyramid
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!("Normal Pyramid Mip {mip_level} Texture View")),
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
            diffuse_radiance_ao_pyramid,
            depth_pyramid,
            normal_pyramid,
            diffuse_radiance_ao_mips,
            depth_mips,
            normal_mips,
        }
    }
}

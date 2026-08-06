use crate::host::renderer::world::gpu_context::resources::SsrTextures;

pub(crate) struct SsrTextureViews {
    pub(crate) reflections: wgpu::TextureView,
    pub(crate) scene_color: wgpu::TextureView,
}

impl SsrTextureViews {
    pub(crate) fn new(textures: &SsrTextures) -> Self {
        let reflections = textures
            .reflections
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("SSR: Reflections Texture View"),
                ..Default::default()
            });
        let scene_color = textures
            .scene_color
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("SSR: Scene Color Texture View"),
                ..Default::default()
            });

        Self {
            reflections,
            scene_color,
        }
    }
}

use crate::host::{
    renderer::rw_texture::RWTextureView, world::gpu_context::resources::SsrTextures,
};

pub(crate) struct SsrTextureViews {
    pub(crate) reflections: wgpu::TextureView,
    pub(crate) scene_color: wgpu::TextureView,
    pub(crate) ssr_history: RWTextureView,
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
        let ssr_history = RWTextureView::new(
            &textures.ssr_history,
            &wgpu::TextureViewDescriptor {
                label: Some("SSR: History Texture View"),
                ..Default::default()
            },
        );

        Self {
            reflections,
            scene_color,
            ssr_history,
        }
    }
}

use crate::host::{
    renderer::rw_texture::RWTextureView, world::gpu_context::resources::TaaResolveTextures,
};

pub(crate) struct TaaResolveTextureViews {
    pub(crate) history: RWTextureView,
}

impl TaaResolveTextureViews {
    pub(crate) fn new(textures: &TaaResolveTextures) -> Self {
        let history = RWTextureView::new(
            &textures.history,
            &wgpu::TextureViewDescriptor {
                label: Some("TAA Resolve: History Texture View"),
                ..Default::default()
            },
        );

        Self { history }
    }
}

use crate::host::renderer::{
    rw_texture::RWTextureView, world::gpu_context::resources::ReprojectTextures,
};

pub struct ReprojectTextureViews {
    pub near_field_irradiance: RWTextureView,
    pub bent_ao: RWTextureView,
    pub depth_history: RWTextureView,
    pub normal_history: RWTextureView,
}
impl ReprojectTextureViews {
    pub fn new(textures: &ReprojectTextures) -> Self {
        let near_field_irradiance = RWTextureView::new(
            &textures.near_field_irradiance,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Near Field Irradiance Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        let bent_ao = RWTextureView::new(
            &textures.bent_ao,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Bent Normals + AO Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        let depth_history = RWTextureView::new(
            &textures.depth_history,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Depth History Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        let normal_history = RWTextureView::new(
            &textures.normal_history,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Normal History Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        Self {
            near_field_irradiance,
            bent_ao,
            depth_history,
            normal_history,
        }
    }
}

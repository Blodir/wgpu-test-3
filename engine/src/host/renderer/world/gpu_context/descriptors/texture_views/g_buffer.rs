use crate::host::renderer::world::gpu_context::resources::GBufferTextures;

pub struct GBufferTextureViews {
    pub albedo_ao: wgpu::TextureView,
    pub normal_roughness: wgpu::TextureView,
    pub emissive_metallic: wgpu::TextureView,
    pub motion_vectors: wgpu::TextureView,
    pub depth: wgpu::TextureView,
}
impl GBufferTextureViews {
    pub fn new(textures: &GBufferTextures) -> Self {
        let albedo_ao = textures
            .albedo_ao
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("GBuffer: Albedo + AO Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let normal_roughness =
            textures
                .normal_roughness
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("GBuffer: Normal + Roughness Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });
        let emissive_metallic =
            textures
                .emissive_metallic
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("GBuffer: Emissive + Metallic Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });
        let motion_vectors = textures
            .motion_vectors
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("GBuffer: Motion Vectors Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let depth = textures
            .depth
            .create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            albedo_ao,
            normal_roughness,
            emissive_metallic,
            motion_vectors,
            depth,
        }
    }
}

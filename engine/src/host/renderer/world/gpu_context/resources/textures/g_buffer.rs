use crate::host::wgpu_context::WgpuContext;

pub struct GBufferTextures {
    pub albedo_ao: wgpu::Texture,
    pub normal_roughness: wgpu::Texture,
    pub emissive_metallic: wgpu::Texture,
    pub motion_vectors: wgpu::Texture,
    pub depth: wgpu::Texture,
}

impl GBufferTextures {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let albedo_ao = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Albedo + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let normal_roughness = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Normal + Roughness Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let emissive_metallic = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Emissive + Metallic Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let motion_vectors = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Motion Vectors Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let depth = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Depth Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            albedo_ao,
            normal_roughness,
            emissive_metallic,
            motion_vectors,
            depth,
        }
    }
}

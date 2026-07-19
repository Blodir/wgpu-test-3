use crate::host::wgpu_context::WgpuContext;

pub(crate) struct LightingTextures {
    pub(crate) lit_hdr: wgpu::Texture,
    pub(crate) diffuse_radiance_ao: wgpu::Texture,
}

impl LightingTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let lit_hdr = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Lighting: Lit HDR Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let diffuse_radiance_ao = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Lighting: Diffuse Radiance + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            lit_hdr,
            diffuse_radiance_ao,
        }
    }
}

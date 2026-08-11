use crate::host::{renderer::rw_texture::RWTexture, wgpu_context::WgpuContext};

pub(crate) struct TaaResolveTextures {
    pub(crate) history: RWTexture,
}

impl TaaResolveTextures {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let history = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("TAA Resolve: History Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context,
        );

        Self { history }
    }
}

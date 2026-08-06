use crate::host::wgpu_context::WgpuContext;

pub(crate) struct SsrTextures {
    pub(crate) reflections: wgpu::Texture,
    pub(crate) scene_color: wgpu::Texture,
}

impl SsrTextures {
    pub(crate) const SCALE_DIVISOR: u32 = 2;

    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let reflections_size = wgpu::Extent3d {
            width: (surface_config.width / Self::SCALE_DIVISOR).max(1),
            height: (surface_config.height / Self::SCALE_DIVISOR).max(1),
            depth_or_array_layers: 1,
        };
        let full_size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let reflections = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("SSR: Reflections Texture"),
                size: reflections_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let scene_color = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("SSR: Scene Color Texture"),
                size: full_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            reflections,
            scene_color,
        }
    }
}

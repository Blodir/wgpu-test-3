use crate::host::wgpu_context::WgpuContext;

pub(crate) struct DepthPyramidTextures {
    pub texture: wgpu::Texture,
    pub mip_level_count: u32,
}

impl DepthPyramidTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let width = surface_config.width.max(1);
        let height = surface_config.height.max(1);
        let mip_level_count = width.max(height).ilog2() + 1;
        let texture = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth Pyramid"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
        Self {
            texture,
            mip_level_count,
        }
    }
}

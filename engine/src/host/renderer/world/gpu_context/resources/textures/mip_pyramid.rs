use crate::host::wgpu_context::WgpuContext;

pub struct MipPyramidTextures {
    pub depth_pyramid: wgpu::Texture,
    pub normal_pyramid: wgpu::Texture,
    pub mip_level_count: u32,
}

impl MipPyramidTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let mip_level_count = surface_config.width.max(surface_config.height).ilog2() + 1;
        let depth_pyramid = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("Depth Pyramid Texture"),
                size,
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let normal_pyramid = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("Normal Pyramid Texture"),
                size,
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            depth_pyramid,
            normal_pyramid,
            mip_level_count,
        }
    }
}

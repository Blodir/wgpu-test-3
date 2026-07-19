use crate::host::wgpu_context::WgpuContext;

use super::HbgiTextures;

pub struct GiBlurTextures {
    pub bent_ao: wgpu::Texture,
    pub near_field_irradiance: wgpu::Texture,
}

impl GiBlurTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let width = (surface_config.width / HbgiTextures::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / HbgiTextures::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let bent_ao = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GI Blur: Bent Normal + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let near_field_irradiance =
            wgpu_context
                .device
                .create_texture(&wgpu::wgt::TextureDescriptor {
                    label: Some("GI Blur: Near Field Irradiance Texture"),
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
            bent_ao,
            near_field_irradiance,
        }
    }
}

use crate::host::wgpu_context::WgpuContext;

use super::HbgiTextures;

pub struct HbgiSvgfTextures {
    pub bent_ao_a: wgpu::Texture,
    pub bent_ao_b: wgpu::Texture,
    pub irradiance_variance_a: wgpu::Texture,
    pub irradiance_variance_b: wgpu::Texture,
}

impl HbgiSvgfTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let width = (surface_config.width / HbgiTextures::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / HbgiTextures::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let make_texture = |label: &'static str| {
            wgpu_context
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
        };

        Self {
            bent_ao_a: make_texture("HBGI SVGF: Bent Normal + AO Texture A"),
            bent_ao_b: make_texture("HBGI SVGF: Bent Normal + AO Texture B"),
            irradiance_variance_a: make_texture("HBGI SVGF: Irradiance + Variance Texture A"),
            irradiance_variance_b: make_texture("HBGI SVGF: Irradiance + Variance Texture B"),
        }
    }
}

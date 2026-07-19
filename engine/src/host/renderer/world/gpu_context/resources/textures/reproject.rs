use crate::host::{renderer::rw_texture::RWTexture, wgpu_context::WgpuContext};

use super::HbgiTextures;

pub struct ReprojectTextures {
    pub near_field_irradiance: RWTexture,
    pub bent_ao: RWTexture,
    pub depth_history: RWTexture,
    pub normal_history: RWTexture,
}

impl ReprojectTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let width = (surface_config.width / HbgiTextures::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / HbgiTextures::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let near_field_irradiance = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Near Field Irradiance Texture"),
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
        let bent_ao = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Bent Normals + AO Texture"),
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
        let depth_history = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Depth History Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context,
        );
        let normal_history = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Normal History Texture"),
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

        Self {
            near_field_irradiance,
            bent_ao,
            depth_history,
            normal_history,
        }
    }
}

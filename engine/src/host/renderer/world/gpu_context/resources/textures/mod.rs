mod g_buffer;
mod gi_blur;
mod hbgi;
mod lighting;
mod mip_pyramid;
mod reproject;
mod sun_shadow;

pub(crate) use g_buffer::GBufferTextures;
pub(crate) use gi_blur::GiBlurTextures;
pub(crate) use hbgi::HbgiTextures;
pub(crate) use lighting::LightingTextures;
pub(crate) use mip_pyramid::MipPyramidTextures;
pub(crate) use reproject::ReprojectTextures;
pub(crate) use sun_shadow::SunShadowTexture2;

use crate::host::wgpu_context::WgpuContext;

pub(crate) struct Textures {
    pub(crate) gbuffer: GBufferTextures,
    pub(crate) hbgi: HbgiTextures,
    pub(crate) gi_blur: GiBlurTextures,
    pub(crate) reproject: ReprojectTextures,
    pub(crate) pyramids: MipPyramidTextures,
    pub(crate) sky: wgpu::Texture,
    pub(crate) sun_shadow: SunShadowTexture2,
    pub(crate) lighting_target: LightingTextures,
}

impl Textures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let gbuffer = GBufferTextures::new(wgpu_context, surface_config);
        let hbgi = HbgiTextures::new(wgpu_context, surface_config);
        let gi_blur = GiBlurTextures::new(wgpu_context, surface_config);
        let reproject = ReprojectTextures::new(wgpu_context, surface_config);
        let pyramids = MipPyramidTextures::new(wgpu_context, surface_config);
        let sun_shadow = SunShadowTexture2::new(wgpu_context);
        let lighting_target = LightingTextures::new(wgpu_context, surface_config);

        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let sky = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Sky Target Texture"),
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
            gbuffer,
            hbgi,
            gi_blur,
            reproject,
            pyramids,
            sky,
            sun_shadow,
            lighting_target,
        }
    }
}

pub mod g_buffer;
pub mod gi_blur;
pub mod hbgi;
pub mod lighting;
pub mod mip_pyramid;
pub mod reproject;
pub mod sun_shadow;

pub(crate) use g_buffer::GBufferTextureViews;
pub(crate) use gi_blur::GiBlurTextureViews;
pub(crate) use hbgi::HbgiTextureViews;
pub(crate) use lighting::LightingTextureViews;
pub(crate) use mip_pyramid::MipPyramidTextureViews;
pub(crate) use reproject::ReprojectTextureViews;
pub(crate) use sun_shadow::SunShadowTextureViews;

use crate::host::world::gpu_context::Textures;

pub(crate) struct TextureViews {
    pub(crate) gbuffer: GBufferTextureViews,
    pub(crate) hbgi: HbgiTextureViews,
    pub(crate) gi_blur: GiBlurTextureViews,
    pub(crate) reproject: ReprojectTextureViews,
    pub(crate) pyramids: MipPyramidTextureViews,
    pub(crate) sky: wgpu::TextureView,
    pub(crate) sun_shadow: SunShadowTextureViews,
    pub(crate) lighting_target: LightingTextureViews,
}
impl TextureViews {
    pub fn new(textures: &Textures) -> Self {
        let gbuffer = GBufferTextureViews::new(&textures.gbuffer);
        let hbgi = HbgiTextureViews::new(&textures.hbgi);
        let gi_blur = GiBlurTextureViews::new(&textures.gi_blur);
        let reproject = ReprojectTextureViews::new(&textures.reproject);
        let pyramids = MipPyramidTextureViews::new(&textures.pyramids);
        let sun_shadow = SunShadowTextureViews::new(&textures.sun_shadow.0);
        let lighting_target = LightingTextureViews::new(&textures.lighting_target);

        let sky = textures.sky.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sky Target Texture View"),
            ..Default::default()
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

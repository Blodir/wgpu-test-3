pub mod g_buffer;
pub mod hbgi;
pub mod hbgi_svgf;
pub mod lighting;
pub mod mip_pyramid;
pub mod reproject;
pub mod ssr;
pub mod sun_shadow;

pub(crate) use g_buffer::GBufferTextureViews;
pub(crate) use hbgi::HbgiTextureViews;
pub(crate) use hbgi_svgf::HbgiSvgfTextureViews;
pub(crate) use lighting::LightingTextureViews;
pub(crate) use mip_pyramid::MipPyramidTextureViews;
pub(crate) use reproject::ReprojectTextureViews;
pub(crate) use ssr::SsrTextureViews;
pub(crate) use sun_shadow::SunShadowTextureViews;

use crate::host::world::gpu_context::Textures;

pub(crate) struct TextureViews {
    pub(crate) gbuffer: GBufferTextureViews,
    pub(crate) hbgi: HbgiTextureViews,
    pub(crate) hbgi_svgf: HbgiSvgfTextureViews,
    pub(crate) reproject: ReprojectTextureViews,
    pub(crate) pyramids: MipPyramidTextureViews,
    pub(crate) ssr: SsrTextureViews,
    pub(crate) sky: wgpu::TextureView,
    pub(crate) sun_shadow: SunShadowTextureViews,
    pub(crate) lighting_target: LightingTextureViews,
}
impl TextureViews {
    pub fn new(textures: &Textures) -> Self {
        let gbuffer = GBufferTextureViews::new(&textures.gbuffer);
        let hbgi = HbgiTextureViews::new(&textures.hbgi);
        let hbgi_svgf = HbgiSvgfTextureViews::new(&textures.hbgi_svgf);
        let reproject = ReprojectTextureViews::new(&textures.reproject);
        let pyramids = MipPyramidTextureViews::new(&textures.pyramids);
        let ssr = SsrTextureViews::new(&textures.ssr);
        let sun_shadow = SunShadowTextureViews::new(&textures.sun_shadow.0);
        let lighting_target = LightingTextureViews::new(&textures.lighting_target);

        let sky = textures.sky.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sky Target Texture View"),
            ..Default::default()
        });
        Self {
            gbuffer,
            hbgi,
            hbgi_svgf,
            reproject,
            pyramids,
            ssr,
            sky,
            sun_shadow,
            lighting_target,
        }
    }
}

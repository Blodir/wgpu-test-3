pub mod depth_pyramid;
pub mod external;
pub mod g_buffer;
pub mod ssrt;
pub mod sun_shadow;

pub(crate) use depth_pyramid::DepthPyramidTextureViews;
pub(crate) use external::ExternalTextureViews;
pub(crate) use g_buffer::GBufferTextureViews;
pub(crate) use ssrt::SsrtTextureViews;
pub(crate) use sun_shadow::SunShadowTextureViews;

use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
use crate::host::world::gpu_context::Textures;

pub(crate) struct TextureViews {
    pub(crate) external: ExternalTextureViews,
    pub(crate) gbuffer: GBufferTextureViews,
    pub(crate) depth_pyramid: DepthPyramidTextureViews,
    pub(crate) ssrt: SsrtTextureViews,
    pub(crate) sun_shadow: SunShadowTextureViews,
}

impl TextureViews {
    pub fn new(
        textures: &Textures,
        render_resources: &RenderAssetStore,
        placeholders: &PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        blue_noise: TextureRenderId,
    ) -> Self {
        Self {
            external: ExternalTextureViews::new(
                render_resources,
                placeholders,
                brdf_lut,
                blue_noise,
            ),
            gbuffer: GBufferTextureViews::new(&textures.gbuffer),
            depth_pyramid: DepthPyramidTextureViews::new(&textures.depth_pyramid),
            ssrt: SsrtTextureViews::new(&textures.ssrt),
            sun_shadow: SunShadowTextureViews::new(&textures.sun_shadow.0),
        }
    }
}

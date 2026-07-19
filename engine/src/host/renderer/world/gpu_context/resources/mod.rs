pub mod buffers;
pub mod samplers;
pub mod textures;

pub(crate) use buffers::{BoneMat34, Buffers, CameraBuffers, LightsBuffers, SunBuffers};
pub(crate) use samplers::Samplers;
pub(crate) use textures::{
    GBufferTextureViews, GBufferTextures, GiBlurTextureViews, GiBlurTextures,
    HbgiTextureViews, HbgiTextures, LightingTextureViews, MipPyramidTextureViews,
    MipPyramidTextures, ReprojectTextureViews, ReprojectTextures, SunShadowTexture2,
    SunShadowTextureViews, TextureViews, Textures,
};

pub(crate) struct Resources {
    pub(crate) textures: Textures,
    pub(crate) buffers: Buffers,
    pub(crate) samplers: Samplers,
}

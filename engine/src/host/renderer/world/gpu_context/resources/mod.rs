pub mod buffers;
pub mod samplers;
pub mod textures;

pub(crate) use buffers::{Buffers, CameraBuffers, LightsBuffers, SunBuffers, SunShadowUniform};
pub(crate) use samplers::Samplers;
pub(crate) use textures::{
    GBufferTextures, HbgiSvgfTextures, HbgiTextures, LightingTextures, MipPyramidTextures,
    ReprojectTextures, SunShadowTexture2, Textures,
};

pub(crate) struct Resources {
    pub(crate) textures: Textures,
    pub(crate) buffers: Buffers,
    pub(crate) samplers: Samplers,
}

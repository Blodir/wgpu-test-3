mod deferred_lighting;
mod g_buffer;
mod hbgi;
mod hbgi_pyramid;
mod hbgi_reproject;
mod hbgi_svgf;
mod post_processing;
mod skinned_transparent;
mod skybox;
mod static_transparent;
mod sun_shadow;

const FULLSCREEN_QUAD_INDEX_COUNT: u32 = 6;

pub(crate) use deferred_lighting::render_deferred_lighting_pass;
pub(crate) use g_buffer::{render_gbuffer_skinned_opaque_pass, render_gbuffer_static_opaque_pass};
pub(crate) use hbgi::render_hbgi_pass;
pub(crate) use hbgi_pyramid::render_hbgi_pyramid_pass;
pub(crate) use hbgi_reproject::render_hbgi_reproject_pass;
pub(crate) use hbgi_svgf::render_hbgi_svgf_pass;
pub(crate) use post_processing::render_post_processing_pass;
pub(crate) use skinned_transparent::render_skinned_transparent_pass;
pub(crate) use skybox::render_skybox_pass;
pub(crate) use static_transparent::render_static_transparent_pass;
pub(crate) use sun_shadow::render_sun_shadow_pass;

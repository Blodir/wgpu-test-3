use crate::host::world::prepare::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT;

pub mod bones;
pub mod camera;
pub mod deferred_lighting;
pub mod deferred_lighting_gbuffer;
pub mod deferred_lighting_hbgi;
pub mod hbgi;
pub mod hbgi_inputs;
pub mod hbgi_pyramid;
pub mod hbgi_reproject;
pub mod hbgi_reproject_inputs;
pub mod hbgi_reproject_settings;
pub mod hbgi_svgf;
pub mod hbgi_settings;
pub mod instance_storage;
pub mod lights;
pub mod post_processing;
pub mod sun_shadow_matrix;

pub(crate) use bones::BonesBindGroups;
pub(crate) use camera::CameraBindGroup;
pub(crate) use deferred_lighting::DeferredLightingBindGroups;
pub(crate) use deferred_lighting_gbuffer::DeferredLightingGBufferBindGroup;
pub(crate) use deferred_lighting_hbgi::DeferredLightingHbgiBindGroup;
pub(crate) use hbgi::HbgiBindGroups;
pub(crate) use hbgi_inputs::HbgiInputsBindGroup;
pub(crate) use hbgi_pyramid::HbgiPyramidBindGroups;
pub(crate) use hbgi_reproject::HbgiReprojectBindGroups;
pub(crate) use hbgi_reproject_inputs::HbgiReprojectInputsBindGroup;
pub(crate) use hbgi_reproject_settings::HbgiReprojectSettingsBindGroup;
pub(crate) use hbgi_svgf::{HbgiSvgfBindGroups, HbgiSvgfInputsBindGroup, HbgiSvgfSettingsBindGroup};
pub(crate) use hbgi_settings::{HbgiSettingsBindGroup, HbgiSettingsUniform};
pub(crate) use instance_storage::InstanceStorageBindGroup;
pub(crate) use lights::LightsBindGroup;
pub(crate) use post_processing::PostProcessingBindGroup;
pub(crate) use sun_shadow_matrix::SunShadowMatrixBindGroup;

pub(crate) struct BindGroups {
    pub(crate) bones: BonesBindGroups,
    pub(crate) camera: CameraBindGroup,
    pub(crate) deferred_lighting: DeferredLightingBindGroups,
    pub(crate) hbgi: HbgiBindGroups,
    pub(crate) hbgi_reproject: HbgiReprojectBindGroups,
    pub(crate) hbgi_svgf: HbgiSvgfBindGroups,
    pub(crate) hbgi_pyramid: HbgiPyramidBindGroups,
    pub(crate) lights: LightsBindGroup,
    pub(crate) post_processing: PostProcessingBindGroup,
    pub(crate) static_instances: InstanceStorageBindGroup,
    pub(crate) sun_shadow_matrices: [SunShadowMatrixBindGroup; SUN_SHADOW_MAX_CASCADE_COUNT],
}

use crate::host::world::prepare::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT;

pub mod bones;
pub mod camera;
pub mod instance_storage;
pub mod lights;
pub mod ssrt;
pub mod ssrt_settings;
pub mod sun_shadow_matrix;

pub(crate) use bones::BonesBindGroups;
pub(crate) use camera::CameraBindGroup;
pub(crate) use instance_storage::InstanceStorageBindGroup;
pub(crate) use lights::LightsBindGroup;
pub(crate) use ssrt::SsrtBindGroups;
pub(crate) use ssrt_settings::{
    r2_quasirandom_sequence, SsrtSettingsBindGroup, SsrtSettingsUniform,
};
pub(crate) use sun_shadow_matrix::SunShadowMatrixBindGroup;

pub(crate) struct BindGroups {
    pub(crate) bones: BonesBindGroups,
    pub(crate) camera: CameraBindGroup,
    pub(crate) lights: LightsBindGroup,
    pub(crate) ssrt: SsrtBindGroups,
    pub(crate) static_instances: InstanceStorageBindGroup,
    pub(crate) sun_shadow_matrices: [SunShadowMatrixBindGroup; SUN_SHADOW_MAX_CASCADE_COUNT],
}

use super::bind_groups::{
    BonesBindGroups, CameraBindGroup, InstanceStorageBindGroup, LightsBindGroup,
    SsrtSettingsBindGroup, SunShadowMatrixBindGroup,
};
use crate::host::renderer::world::external::MaterialBindGroup;
use crate::host::wgpu_context::WgpuContext;

pub struct BGLayouts {
    pub camera: wgpu::BindGroupLayout,
    pub lights: wgpu::BindGroupLayout,
    pub sun_shadow_matrix: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub bones: wgpu::BindGroupLayout,
    pub motion_bones: wgpu::BindGroupLayout,
    pub instance_storage: wgpu::BindGroupLayout,
    pub pbr_material: wgpu::BindGroupLayout,
    pub ssrt_settings: wgpu::BindGroupLayout,
    pub a_trous_settings: wgpu::BindGroupLayout,
}
impl BGLayouts {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let device = &wgpu_context.device;
        Self {
            camera: device.create_bind_group_layout(&CameraBindGroup::desc()),
            lights: device.create_bind_group_layout(&LightsBindGroup::desc()),
            sun_shadow_matrix: device.create_bind_group_layout(&SunShadowMatrixBindGroup::desc()),
            material: device.create_bind_group_layout(&MaterialBindGroup::desc()),
            bones: device.create_bind_group_layout(&BonesBindGroups::desc()),
            motion_bones: device.create_bind_group_layout(&BonesBindGroups::motion_desc()),
            instance_storage: device.create_bind_group_layout(&InstanceStorageBindGroup::desc()),
            pbr_material: device.create_bind_group_layout(&MaterialBindGroup::desc()),
            ssrt_settings: device.create_bind_group_layout(&SsrtSettingsBindGroup::desc()),
            a_trous_settings: device.create_bind_group_layout(&SsrtSettingsBindGroup::desc()),
        }
    }
}

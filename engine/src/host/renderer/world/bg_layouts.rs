use super::{
    bindgroups::{
        bones::BonesBinding, camera::CameraBinding, instance_storage::InstanceStorageBinding,
        lights::LightsBinding, material::MaterialBinding,
        sun_shadow_matrix::SunShadowMatrixBindGroup,
    },
    gpu_context::{
        DeferredLightingGBufferBindGroup, DeferredLightingHbgiBindGroup, GBufferBindGroup,
        GiBlurBindGroup, HbgiInputsBindGroup, HbgiPyramidBindGroups, HbgiReprojectInputsBindGroup,
        HbgiReprojectSettingsBindGroup, HbgiSettingsBindGroup, PostProcessingBindGroup,
    },
};
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
    pub g_buffer: wgpu::BindGroupLayout,
    pub hbgi_settings: wgpu::BindGroupLayout,
    pub hbgi_inputs: wgpu::BindGroupLayout,
    pub deferred_lighting_gbuffer: wgpu::BindGroupLayout,
    pub deferred_lighting_hbgi: wgpu::BindGroupLayout,
    pub gi_blur: wgpu::BindGroupLayout,
    pub hbgi_reproject_inputs: wgpu::BindGroupLayout,
    pub hbgi_reproject_settings: wgpu::BindGroupLayout,
    pub hbgi_pyramid_base: wgpu::BindGroupLayout,
    pub hbgi_pyramid_downsample: wgpu::BindGroupLayout,
    pub post_processing: wgpu::BindGroupLayout,
}
impl BGLayouts {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let camera = wgpu_context
            .device
            .create_bind_group_layout(&CameraBinding::desc());
        let lights = wgpu_context
            .device
            .create_bind_group_layout(&LightsBinding::desc());
        let sun_shadow_matrix = wgpu_context
            .device
            .create_bind_group_layout(&SunShadowMatrixBindGroup::desc());
        let material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());
        let bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBinding::desc());
        let motion_bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBinding::motion_desc());
        let instance_storage = wgpu_context
            .device
            .create_bind_group_layout(&InstanceStorageBinding::desc());
        let pbr_material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());
        let g_buffer = wgpu_context
            .device
            .create_bind_group_layout(&GBufferBindGroup::desc());
        let hbgi_settings = wgpu_context
            .device
            .create_bind_group_layout(&HbgiSettingsBindGroup::desc());
        let hbgi_inputs = wgpu_context
            .device
            .create_bind_group_layout(&HbgiInputsBindGroup::desc());
        let deferred_lighting_gbuffer = wgpu_context
            .device
            .create_bind_group_layout(&DeferredLightingGBufferBindGroup::desc());
        let deferred_lighting_hbgi = wgpu_context
            .device
            .create_bind_group_layout(&DeferredLightingHbgiBindGroup::desc());
        let gi_blur = wgpu_context
            .device
            .create_bind_group_layout(&GiBlurBindGroup::desc());
        let hbgi_reproject_inputs = wgpu_context
            .device
            .create_bind_group_layout(&HbgiReprojectInputsBindGroup::desc());
        let hbgi_reproject_settings = wgpu_context
            .device
            .create_bind_group_layout(&HbgiReprojectSettingsBindGroup::desc());
        let hbgi_pyramid_base = wgpu_context
            .device
            .create_bind_group_layout(&HbgiPyramidBindGroups::base_desc());
        let hbgi_pyramid_downsample = wgpu_context
            .device
            .create_bind_group_layout(&HbgiPyramidBindGroups::downsample_desc());
        let post_processing = wgpu_context
            .device
            .create_bind_group_layout(&PostProcessingBindGroup::desc());

        Self {
            camera,
            lights,
            sun_shadow_matrix,
            material,
            bones,
            motion_bones,
            instance_storage,
            pbr_material,
            g_buffer,
            hbgi_settings,
            hbgi_inputs,
            deferred_lighting_gbuffer,
            deferred_lighting_hbgi,
            gi_blur,
            hbgi_reproject_inputs,
            hbgi_reproject_settings,
            hbgi_pyramid_base,
            hbgi_pyramid_downsample,
            post_processing,
        }
    }
}

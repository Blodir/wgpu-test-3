use super::bind_groups::{
    BonesBindGroups, CameraBindGroup, DeferredLightingGBufferBindGroup,
    DeferredLightingHbgiBindGroup, HbgiInputsBindGroup, HbgiPyramidBindGroups,
    HbgiReprojectInputsBindGroup, HbgiReprojectSettingsBindGroup, HbgiSettingsBindGroup,
    HbgiSvgfInputsBindGroup, HbgiSvgfSettingsBindGroup, InstanceStorageBindGroup, LightsBindGroup,
    PostProcessingBindGroup, SsrBindGroup, SsrCompositeBindGroup, SunShadowMatrixBindGroup,
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
    pub hbgi_settings: wgpu::BindGroupLayout,
    pub hbgi_inputs: wgpu::BindGroupLayout,
    pub deferred_lighting_gbuffer: wgpu::BindGroupLayout,
    pub deferred_lighting_hbgi: wgpu::BindGroupLayout,
    pub hbgi_reproject_inputs: wgpu::BindGroupLayout,
    pub hbgi_reproject_settings: wgpu::BindGroupLayout,
    pub hbgi_svgf_inputs: wgpu::BindGroupLayout,
    pub hbgi_svgf_settings: wgpu::BindGroupLayout,
    pub hbgi_pyramid_base: wgpu::BindGroupLayout,
    pub hbgi_pyramid_downsample: wgpu::BindGroupLayout,
    pub post_processing: wgpu::BindGroupLayout,
    pub ssr_inputs: wgpu::BindGroupLayout,
    pub ssr_composite_inputs: wgpu::BindGroupLayout,
}
impl BGLayouts {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let camera = wgpu_context
            .device
            .create_bind_group_layout(&CameraBindGroup::desc());
        let lights = wgpu_context
            .device
            .create_bind_group_layout(&LightsBindGroup::desc());
        let sun_shadow_matrix = wgpu_context
            .device
            .create_bind_group_layout(&SunShadowMatrixBindGroup::desc());
        let material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBindGroup::desc());
        let bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBindGroups::desc());
        let motion_bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBindGroups::motion_desc());
        let instance_storage = wgpu_context
            .device
            .create_bind_group_layout(&InstanceStorageBindGroup::desc());
        let pbr_material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBindGroup::desc());
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
        let hbgi_reproject_inputs = wgpu_context
            .device
            .create_bind_group_layout(&HbgiReprojectInputsBindGroup::desc());
        let hbgi_reproject_settings = wgpu_context
            .device
            .create_bind_group_layout(&HbgiReprojectSettingsBindGroup::desc());
        let hbgi_svgf_inputs = wgpu_context
            .device
            .create_bind_group_layout(&HbgiSvgfInputsBindGroup::desc());
        let hbgi_svgf_settings = wgpu_context
            .device
            .create_bind_group_layout(&HbgiSvgfSettingsBindGroup::desc());
        let hbgi_pyramid_base = wgpu_context
            .device
            .create_bind_group_layout(&HbgiPyramidBindGroups::base_desc());
        let hbgi_pyramid_downsample = wgpu_context
            .device
            .create_bind_group_layout(&HbgiPyramidBindGroups::downsample_desc());
        let post_processing = wgpu_context
            .device
            .create_bind_group_layout(&PostProcessingBindGroup::desc());
        let ssr_inputs = wgpu_context
            .device
            .create_bind_group_layout(&SsrBindGroup::desc());
        let ssr_composite_inputs = wgpu_context
            .device
            .create_bind_group_layout(&SsrCompositeBindGroup::desc());

        Self {
            camera,
            lights,
            sun_shadow_matrix,
            material,
            bones,
            motion_bones,
            instance_storage,
            pbr_material,
            hbgi_settings,
            hbgi_inputs,
            deferred_lighting_gbuffer,
            deferred_lighting_hbgi,
            hbgi_reproject_inputs,
            hbgi_reproject_settings,
            hbgi_svgf_inputs,
            hbgi_svgf_settings,
            hbgi_pyramid_base,
            hbgi_pyramid_downsample,
            post_processing,
            ssr_inputs,
            ssr_composite_inputs,
        }
    }
}

use super::super::bg_layouts::BGLayouts;
use super::{
    hbgi_reproject_inputs::HbgiReprojectInputsBindGroup,
    hbgi_reproject_settings::HbgiReprojectSettingsBindGroup,
};

pub(crate) struct HbgiReprojectBindGroups {
    pub(crate) inputs: HbgiReprojectInputsBindGroup,
    pub(crate) settings: HbgiReprojectSettingsBindGroup,
}
impl HbgiReprojectBindGroups {
    pub fn new(
        motion_vectors_view: &wgpu::TextureView,
        current_hbgi_view: &wgpu::TextureView,
        prev_hbgi_reproject_view: &wgpu::TextureView,
        current_depth_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        prev_depth_history_view: &wgpu::TextureView,
        prev_normal_history_view: &wgpu::TextureView,
        current_hbgi_irradiance_view: &wgpu::TextureView,
        prev_hbgi_irradiance_reproject_view: &wgpu::TextureView,
        settings_buffer: &wgpu::Buffer,
        layouts: &BGLayouts,
        device: &wgpu::Device,
    ) -> Self {
        let inputs = HbgiReprojectInputsBindGroup::new(
            motion_vectors_view,
            current_hbgi_view,
            prev_hbgi_reproject_view,
            current_depth_view,
            current_normal_view,
            prev_depth_history_view,
            prev_normal_history_view,
            current_hbgi_irradiance_view,
            prev_hbgi_irradiance_reproject_view,
            &layouts.hbgi_reproject_inputs,
            device,
        );
        let settings = HbgiReprojectSettingsBindGroup::new(
            settings_buffer,
            &layouts.hbgi_reproject_settings,
            device,
        );

        Self { inputs, settings }
    }
}

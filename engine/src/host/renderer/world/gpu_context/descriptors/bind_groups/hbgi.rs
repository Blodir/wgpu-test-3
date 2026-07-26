use super::super::texture_views::MipPyramidTextureViews;
use super::{hbgi_inputs::HbgiInputsBindGroup, hbgi_settings::HbgiSettingsBindGroup};

pub(crate) struct HbgiBindGroups {
    pub(crate) settings: HbgiSettingsBindGroup,
    pub(crate) inputs: HbgiInputsBindGroup,
}
impl HbgiBindGroups {
    pub fn new(
        hbgi_settings_buffer: &wgpu::Buffer,
        hbgi_settings_layout: &wgpu::BindGroupLayout,
        mip_texture_views: &MipPyramidTextureViews,
        hbgi_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        blue_noise_view: &wgpu::TextureView,
        hbgi_inputs_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        let settings =
            HbgiSettingsBindGroup::new(hbgi_settings_buffer, hbgi_settings_layout, device);
        let inputs = HbgiInputsBindGroup::new(
            mip_texture_views,
            hbgi_sampler,
            normal_sampler,
            blue_noise_view,
            hbgi_inputs_layout,
            device,
        );

        Self { settings, inputs }
    }
}

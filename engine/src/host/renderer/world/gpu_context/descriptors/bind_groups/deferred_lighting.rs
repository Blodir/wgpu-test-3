use super::{
    deferred_lighting_gbuffer::DeferredLightingGBufferBindGroup,
    deferred_lighting_hbgi::DeferredLightingHbgiBindGroup,
};
use super::super::bg_layouts::BGLayouts;

pub(crate) struct DeferredLightingBindGroups {
    pub(crate) gbuffer: DeferredLightingGBufferBindGroup,
    pub(crate) hbgi: DeferredLightingHbgiBindGroup,
}
impl DeferredLightingBindGroups {
    pub fn new(
        albedo_ao_view: &wgpu::TextureView,
        albedo_ao_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        emissive_metallic_view: &wgpu::TextureView,
        emissive_metallic_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        blurred_view: &wgpu::TextureView,
        blurred_sampler: &wgpu::Sampler,
        blurred_irradiance_view: &wgpu::TextureView,
        blurred_irradiance_sampler: &wgpu::Sampler,
        layouts: &BGLayouts,
        device: &wgpu::Device,
    ) -> Self {
        let gbuffer = DeferredLightingGBufferBindGroup::new(
            albedo_ao_view,
            albedo_ao_sampler,
            normal_roughness_view,
            normal_roughness_sampler,
            emissive_metallic_view,
            emissive_metallic_sampler,
            depth_view,
            &layouts.deferred_lighting_gbuffer,
            device,
        );
        let hbgi = DeferredLightingHbgiBindGroup::new(
            blurred_view,
            blurred_sampler,
            blurred_irradiance_view,
            blurred_irradiance_sampler,
            &layouts.deferred_lighting_hbgi,
            device,
        );

        Self { gbuffer, hbgi }
    }
}

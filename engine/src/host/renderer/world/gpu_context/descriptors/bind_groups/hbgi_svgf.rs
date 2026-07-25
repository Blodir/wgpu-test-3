pub(crate) struct HbgiSvgfInputsBindGroup(pub(crate) wgpu::BindGroup);
impl HbgiSvgfInputsBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
            label: Some("HBGI SVGF Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        input_bent_ao_view: &wgpu::TextureView,
        input_irradiance_variance_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        depth_history_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_bent_ao_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(input_irradiance_variance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(current_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(depth_history_view),
                },
            ],
            label: Some("HBGI SVGF Inputs Bind Group"),
        })
    }

    pub fn new(
        input_bent_ao_view: &wgpu::TextureView,
        input_irradiance_variance_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        depth_history_view: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self(Self::create_bind_group(
            device,
            layout,
            input_bent_ao_view,
            input_irradiance_variance_view,
            current_normal_view,
            depth_history_view,
        ))
    }
}

pub(crate) struct HbgiSvgfSettingsBindGroup(pub(crate) wgpu::BindGroup);
impl HbgiSvgfSettingsBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("HBGI SVGF Settings Bind Group Layout"),
        }
    }

    fn create_bind_group(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("HBGI SVGF Settings Bind Group"),
        })
    }

    pub fn new(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self(Self::create_bind_group(buffer, layout, device))
    }
}

use crate::host::renderer::world::gpu_context::BGLayouts;

pub(crate) struct HbgiSvgfBindGroups {
    pub(crate) initial: HbgiSvgfInputsBindGroup,
    pub(crate) history_a: HbgiSvgfInputsBindGroup,
    pub(crate) history_b: HbgiSvgfInputsBindGroup,
    pub(crate) settings: HbgiSvgfSettingsBindGroup,
}

impl HbgiSvgfBindGroups {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reproject_bent_ao_view: &wgpu::TextureView,
        reproject_irradiance_view: &wgpu::TextureView,
        reproject_depth_history_view: &wgpu::TextureView,
        svgf_bent_ao_a_view: &wgpu::TextureView,
        svgf_irradiance_variance_a_view: &wgpu::TextureView,
        svgf_bent_ao_b_view: &wgpu::TextureView,
        svgf_irradiance_variance_b_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        settings_buffer: &wgpu::Buffer,
        layouts: &BGLayouts,
        device: &wgpu::Device,
    ) -> Self {
        let initial = HbgiSvgfInputsBindGroup::new(
            reproject_bent_ao_view,
            reproject_irradiance_view,
            current_normal_view,
            reproject_depth_history_view,
            &layouts.hbgi_svgf_inputs,
            device,
        );
        let history_a = HbgiSvgfInputsBindGroup::new(
            svgf_bent_ao_a_view,
            svgf_irradiance_variance_a_view,
            current_normal_view,
            reproject_depth_history_view,
            &layouts.hbgi_svgf_inputs,
            device,
        );
        let history_b = HbgiSvgfInputsBindGroup::new(
            svgf_bent_ao_b_view,
            svgf_irradiance_variance_b_view,
            current_normal_view,
            reproject_depth_history_view,
            &layouts.hbgi_svgf_inputs,
            device,
        );
        let settings = HbgiSvgfSettingsBindGroup::new(
            settings_buffer,
            &layouts.hbgi_svgf_settings,
            device,
        );
        Self {
            initial,
            history_a,
            history_b,
            settings,
        }
    }
}

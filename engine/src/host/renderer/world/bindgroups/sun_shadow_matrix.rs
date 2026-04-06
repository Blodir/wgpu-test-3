pub struct SunShadowMatrixBindGroup {
    pub bind_group: wgpu::BindGroup,
}

impl SunShadowMatrixBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Sun Shadow Matrix Bind Group Layout"),
        }
    }

    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sun_shadow_light_view_proj_buffer: &wgpu::Buffer,
    ) -> Self {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sun_shadow_light_view_proj_buffer.as_entire_binding(),
            }],
            label: Some("Sun Shadow Matrix Bind Group"),
        });
        Self { bind_group }
    }
}

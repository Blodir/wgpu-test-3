use crate::host::renderer::world::gpu_context::resources::CameraBuffers;

pub(crate) struct CameraBindGroup(pub(crate) wgpu::BindGroup);
impl CameraBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Camera Bind Group Layout"),
        }
    }

    fn create_bind_group(
        buffers: &CameraBuffers,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.view_proj.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.position.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffers.inverse_view_proj.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: buffers.view_rotation.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: buffers.prev_view_proj.as_entire_binding(),
                },
            ],
            label: Some("Camera Bind Group"),
        })
    }

    pub fn new(
        buffers: &CameraBuffers,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self(Self::create_bind_group(buffers, layout, device))
    }
}

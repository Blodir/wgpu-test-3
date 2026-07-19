use crate::host::renderer::rw_buffer::RWBuffer;

pub(crate) struct BonesBindGroups {
    pub bones_bind_group: wgpu::BindGroup,
    pub motion_bind_group: wgpu::BindGroup,
}
impl BonesBindGroups {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Bones Bind Group Layout"),
        }
    }

    pub fn motion_desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Motion Bones Bind Group Layout"),
        }
    }

    fn create_bones_bind_group(
        bones: &RWBuffer,
        skinned_instances: &RWBuffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bones Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: bones.get_write_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skinned_instances.get_write_buf().as_entire_binding(),
                },
            ],
        })
    }

    fn create_motion_bind_group(
        bones: &RWBuffer,
        skinned_instances: &RWBuffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Motion Bones Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: bones.get_write_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bones.get_read_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: skinned_instances.get_write_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: skinned_instances.get_read_buf().as_entire_binding(),
                },
            ],
        })
    }

    pub fn new(
        bones: &RWBuffer,
        skinned_instances: &RWBuffer,
        layout: &wgpu::BindGroupLayout,
        motion_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bones_bind_group: Self::create_bones_bind_group(
                bones,
                skinned_instances,
                layout,
                device,
            ),
            motion_bind_group: Self::create_motion_bind_group(
                bones,
                skinned_instances,
                motion_layout,
                device,
            ),
        }
    }
}

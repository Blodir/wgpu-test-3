use wgpu::util::DeviceExt as _;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoneMat34 {
    pub mat: [[f32; 4]; 3],
}
impl Default for BoneMat34 {
    fn default() -> Self {
        Self {
            mat: [
                [1f32, 0f32, 0f32, 0f32],
                [0f32, 1f32, 0f32, 0f32],
                [0f32, 0f32, 1f32, 0f32],
            ]
        }
    }
}

pub struct BonesBinding {
    pub bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
}
impl BonesBinding {
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
            ],
            label: Some("Bones Bind Group Layout"),
        }
    }
    pub fn new(layout: &wgpu::BindGroupLayout, device: &wgpu::Device) -> Self {
        let data: Vec<BoneMat34> = vec![BoneMat34::default(); 1024];
        // TODO allocate extra space
        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bones SSBO"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bones Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }]
        });
        Self {
            bind_group,
            buffer: storage_buffer,
        }
    }
    pub fn update(&mut self, data: Vec<BoneMat34>, queue: &wgpu::Queue) {
        // TODO check if there's enough space?
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&data));
    }
}

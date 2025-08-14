use cgmath::{Matrix, Matrix3, Matrix4, SquareMatrix};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub m4: [[f32; 4]; 4],
    pub itr: [[f32; 3]; 3],
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            m4: Matrix4::identity().into(),
            itr: Matrix3::identity().into(),
        }
    }
}
impl Instance {
    pub fn from_transform(transform: Matrix4<f32>) -> Self {
        Instance {
            m4: transform.clone().into(),
            itr: Matrix3::new(
                transform.x.x,
                transform.x.y,
                transform.x.z,
                transform.y.x,
                transform.y.y,
                transform.y.z,
                transform.z.x,
                transform.z.y,
                transform.z.z,
            )
            .invert()
            .unwrap()
            .transpose()
            .into(),
        }
    }
}

impl Instance {
    const BASE_SHADER_LOCATION: u32 = 0;
    const ATTRIBUTES: [wgpu::VertexAttribute; 7] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: Self::BASE_SHADER_LOCATION + 0,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 4]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 2,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 12]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 16]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 4,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 19]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 5,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 22]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 6,
            format: wgpu::VertexFormat::Float32x3,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub fn from(mat4: Matrix4<f32>, itr: Matrix3<f32>) -> Self {
        Self {
            m4: mat4.into(),
            itr: itr.into(),
        }
    }
}

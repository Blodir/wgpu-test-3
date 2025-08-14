#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub tangent: [f32; 4],
    pub weights: [f32; 4],
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub normal_tex_coords: [f32; 2],
    pub occlusion_tex_coords: [f32; 2],
    pub emissive_tex_coords: [f32; 2],
    pub base_color_tex_coords: [f32; 2],
    pub metallic_roughness_tex_coords: [f32; 2],
    pub joints: [u8; 4],
    // TODO add padding for alignment
}

impl Default for Vertex {
    fn default() -> Self {
        Vertex {
            tangent: [1.0, 0.0, 0.0, 1.0],
            weights: [1.0, 0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            normal_tex_coords: [0.0, 0.0],
            occlusion_tex_coords: [0.0, 0.0],
            emissive_tex_coords: [0.0, 0.0],
            base_color_tex_coords: [0.0, 0.0],
            metallic_roughness_tex_coords: [0.0, 0.0],
            joints: [0, 0, 0, 0],
        }
    }
}

impl Vertex {
    const BASE_SHADER_LOCATION: u32 = 7;
    const OFFSET_TAN: wgpu::BufferAddress = 0;
    const OFFSET_WEI: wgpu::BufferAddress =
        Self::OFFSET_TAN + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    const OFFSET_POS: wgpu::BufferAddress =
        Self::OFFSET_WEI + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    const OFFSET_NOR: wgpu::BufferAddress =
        Self::OFFSET_POS + size_of::<[f32; 3]>() as wgpu::BufferAddress;
    const OFFSET_NTC: wgpu::BufferAddress =
        Self::OFFSET_NOR + size_of::<[f32; 3]>() as wgpu::BufferAddress;
    //const OFFSET_OCC: wgpu::BufferAddress = Self::OFFSET_NTC + size_of::<[f32; 2]>() as wgpu::BufferAddress;
    // optimization: combining normal tex coords and occlusion tex coords
    const OFFSET_EMI: wgpu::BufferAddress =
        Self::OFFSET_NTC + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    //const OFFSET_BAS: wgpu::BufferAddress = Self::OFFSET_EMI + size_of::<[f32; 2]>() as wgpu::BufferAddress;
    // optimization: combining emissive and base color tex coords
    const OFFSET_MET: wgpu::BufferAddress =
        Self::OFFSET_EMI + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    const OFFSET_JOI: wgpu::BufferAddress =
        Self::OFFSET_MET + size_of::<[f32; 2]>() as wgpu::BufferAddress;
    const ATTRIBUTES: [wgpu::VertexAttribute; 8] = [
        // 16 byte fields are first for better data alignment
        // I have not tested if this actually matters
        // at least need to add padding first for data alignment to matter
        wgpu::VertexAttribute {
            offset: Self::OFFSET_TAN,
            shader_location: Self::BASE_SHADER_LOCATION + 0,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_WEI,
            shader_location: Self::BASE_SHADER_LOCATION + 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_POS,
            shader_location: Self::BASE_SHADER_LOCATION + 2,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_NOR,
            shader_location: Self::BASE_SHADER_LOCATION + 3,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_NTC,
            shader_location: Self::BASE_SHADER_LOCATION + 4,
            // optimization: combining normal tex coords and occlusion tex coords
            format: wgpu::VertexFormat::Float32x4,
        },
        /*
        wgpu::VertexAttribute {
            offset: Self::OFFSET_OCC,
            shader_location: Self::BASE_SHADER_LOCATION + 5,
            format: wgpu::VertexFormat::Float32x2,
        },
        */
        wgpu::VertexAttribute {
            offset: Self::OFFSET_EMI,
            shader_location: Self::BASE_SHADER_LOCATION + 5,
            // optimization: combining emissive base color tex coords
            format: wgpu::VertexFormat::Float32x4,
        },
        /*
        wgpu::VertexAttribute {
            offset: Self::OFFSET_BAS,
            shader_location: Self::BASE_SHADER_LOCATION + 6,
            format: wgpu::VertexFormat::Float32x2,
        },
        */
        wgpu::VertexAttribute {
            offset: Self::OFFSET_MET,
            shader_location: Self::BASE_SHADER_LOCATION + 6,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_JOI,
            shader_location: Self::BASE_SHADER_LOCATION + 7,
            format: wgpu::VertexFormat::Uint8x4,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

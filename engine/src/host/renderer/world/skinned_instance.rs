use glam::{Mat3, Mat4};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedInstance {
    pub m4: [[f32; 4]; 4],
    pub itr: [[f32; 4]; 3],
    pub offsets: [u32; 4],
}

fn pad_mat3(mat: Mat3) -> [[f32; 4]; 3] {
    let cols = mat.to_cols_array_2d();
    [
        [cols[0][0], cols[0][1], cols[0][2], 0.0],
        [cols[1][0], cols[1][1], cols[1][2], 0.0],
        [cols[2][0], cols[2][1], cols[2][2], 0.0],
    ]
}

impl Default for SkinnedInstance {
    fn default() -> Self {
        Self {
            m4: Mat4::IDENTITY.to_cols_array_2d(),
            itr: pad_mat3(Mat3::IDENTITY),
            offsets: [0, u32::MAX, 0, 0],
        }
    }
}
impl SkinnedInstance {
    pub fn new(transform: Mat4, palette_offset: u32) -> Self {
        let m4 = transform.to_cols_array_2d();
        let itr = Mat3::from_mat4(transform).inverse().transpose();

        SkinnedInstance {
            m4,
            itr: pad_mat3(itr),
            offsets: [palette_offset, u32::MAX, 0, 0],
        }
    }

    pub fn from(mat4: Mat4, itr: Mat3, palette_offset: u32) -> Self {
        Self {
            m4: mat4.to_cols_array_2d(),
            itr: pad_mat3(itr),
            offsets: [palette_offset, u32::MAX, 0, 0],
        }
    }
}

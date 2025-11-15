use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Joint {
    pub name: Option<String>,
    pub children: Vec<u32>,
    /// row-major
    pub trs: [[f32; 4]; 4],
    /// row-major
    pub inverse_bind_matrix: [[f32; 4]; 4],
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
}

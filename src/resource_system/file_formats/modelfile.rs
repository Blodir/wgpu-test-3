use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Submesh {
    pub instances: Vec<[[f32; 4]; 4]>,
    pub index_byte_offset: u32,
    pub index_byte_length: u32,
    pub vertex_byte_offset: u32,
    pub vertex_byte_length: u32,
    pub base_vertex: u32,
    pub material: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Deformation {
    None,
    Skinned {
        skeleton: String,
        animations: Vec<String>,
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Model {
    pub submeshes: Vec<Submesh>,
    pub material_paths: Vec<String>,
    pub buffer: String,
    pub deformation: Deformation,
    pub vertex_buffer_start_offset: u32,
    pub aabb: Aabb,
}

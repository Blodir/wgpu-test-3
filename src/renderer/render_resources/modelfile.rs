use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Primitive {
    pub instances: Vec<[[f32; 4]; 4]>,
    pub index_byte_offset: u32,
    pub index_byte_length: u32,
    pub vertex_byte_offset: u32,
    pub vertex_byte_length: u32,
    pub base_vertex: u32,
    pub material: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// index buffer format is always u32
#[derive(Serialize, Deserialize, Debug)]
pub struct Model {
    pub primitives: Vec<Primitive>,
    pub material_paths: Vec<String>,
    pub buffer_path: String,
    pub skeletonfile_path: String,
    pub animations: Vec<String>,
    pub vertex_buffer_start_offset: u32,
    pub aabb: Aabb,
}

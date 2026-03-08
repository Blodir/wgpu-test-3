use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SRT(Vec3, Quat, Vec3);

impl SRT {
    pub fn new(scale: Vec3, rotation: Quat, translation: Vec3) -> Self {
        Self(scale, rotation, translation)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Node {
    pub name: Option<String>,
    pub parent: Option<u32>,
    pub transform: SRT,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Rig {
    /// sorted topologically
    pub nodes: Vec<Node>,
    pub joint_nodes: Vec<u32>,
    pub inverse_bind_matrices: Vec<Mat4>,
}

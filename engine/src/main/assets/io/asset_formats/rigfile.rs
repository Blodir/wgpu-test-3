use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SRT(Vec3, Quat, Vec3);

impl SRT {
    pub fn new(scale: Vec3, rotation: Quat, translation: Vec3) -> Self {
        Self(scale, rotation, translation)
    }

    pub fn to_scale_rotation_translation(&self) -> (Vec3, Quat, Vec3) {
        (self.0, self.1, self.2)
    }

    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.0, self.1, self.2)
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

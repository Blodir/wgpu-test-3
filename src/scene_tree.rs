use std::collections::HashMap;

use glam::{Mat4, Vec3};

use crate::renderer::render_resources::{EnvironmentMapHandle, ModelHandle};
use generational_arena::{Arena, Index};

#[derive(Clone)]
pub struct Sun {
    pub direction: [f32; 3],
    pub color: [f32; 3],
}
impl Default for Sun {
    fn default() -> Self {
        Sun {
            direction: Vec3::new(1.0, -1.0, 1.0).normalize().into(),
            color: [10.0, 10.0, 10.0],
        }
    }
}

#[derive(Clone)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub rot_x: f32, // deg
    pub rot_y: f32, // deg
}
impl Default for Camera {
    fn default() -> Self {
        let eye: Vec3 = (0.0, 0.0, 100.0).into();
        let target: Vec3 = (0.0, 0.0, 0.0).into();
        let up: Vec3 = Vec3::Y;
        let fovy = 45.0f32;
        let znear = 0.1f32;
        let zfar = 100.0f32;
        let rot_x = 0f32;
        let rot_y = 0f32;

        Self {
            eye,
            target,
            up,
            fovy,
            znear,
            zfar,
            rot_x,
            rot_y,
        }
    }
}

pub enum RenderDataType {
    Model(ModelHandle),
}

pub struct Node {
    pub parent: Option<Index>,
    pub children: Vec<Index>,
    pub transform: Mat4,
    pub render_data: RenderDataType,
}

pub struct Scene {
    pub root: Index,
    pub nodes: Arena<Node>,
    pub sun: Sun,
    pub camera: Camera,
    pub environment: EnvironmentMapHandle,
}
impl Default for Scene {
    fn default() -> Self {
        let mut nodes = Arena::new();

        let model_handle = ModelHandle("assets/local/RiggedFigure/RiggedFigure.json".to_string());

        let root_handle = nodes.insert(Node {
            parent: None,
            children: vec![],
            transform: Mat4::IDENTITY,
            render_data: RenderDataType::Model(model_handle.clone()),
        });

        Self {
            root: root_handle,
            nodes,
            sun: Sun::default(),
            camera: Camera::default(),
            environment: EnvironmentMapHandle("assets/kloofendal_overcast_puresky_8k".to_string()),
        }
    }
    /*
    fn default() -> Self {
        let mut nodes = Arena::new();

        let lantern_handle = ModelHandle("assets/local/Lantern/Lantern.json".to_string());

        let root_handle = nodes.insert(Node {
            parent: None,
            children: vec![],
            transform: Mat4::IDENTITY,
            render_data: RenderDataType::Model(lantern_handle.clone()),
        });
        let child_handle = nodes.insert(Node {
            parent: Some(root_handle),
            children: vec![],
            transform: Mat4::from_translation(Vec3::new(8.0, 0.0, 8.0)),
            render_data: RenderDataType::Model(lantern_handle),
        });
        nodes
            .get_mut(root_handle)
            .unwrap()
            .children
            .push(child_handle);

        Self {
            root: root_handle,
            nodes,
            sun: Sun::default(),
            camera: Camera::default(),
            environment: EnvironmentMapHandle("assets/kloofendal_overcast_puresky_8k".to_string()),
        }
    }
    */
}

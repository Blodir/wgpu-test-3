use cgmath::{InnerSpace as _, Matrix4, SquareMatrix, Vector3};
use wgpu::SurfaceConfiguration;

use crate::render_engine::render_resources::{EnvironmentMapHandle, ModelHandle};

pub struct NodeHandle(u32);

pub struct Sun {
    pub direction: [f32; 3],
    pub color: [f32; 3],
}
impl Default for Sun {
    fn default() -> Self {
        Sun {
            direction: Vector3::new(1.0, -1.0, 1.0).normalize().into(),
            color: [10.0, 10.0, 10.0],
        }
    }
}

pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub rot_x: cgmath::Deg<f32>,
    pub rot_y: cgmath::Deg<f32>,
}
impl Default for Camera {
    fn default() -> Self {
        let eye: cgmath::Point3<f32> = (0.0, 0.0, 100.0).into();
        let target: cgmath::Point3<f32> = (0.0, 0.0, 0.0).into();
        let up: cgmath::Vector3<f32> = cgmath::Vector3::unit_y();
        let fovy = 45.0f32;
        let znear = 0.1f32;
        let zfar = 100.0f32;
        let rot_x = cgmath::Deg(0f32);
        let rot_y = cgmath::Deg(0f32);

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
    pub parent: Option<NodeHandle>,
    pub children: Vec<NodeHandle>,
    pub transform: Matrix4<f32>,
    pub render_data: RenderDataType,
}

pub struct Scene {
    pub root: Node,
    pub sun: Sun,
    pub camera: Camera,
    pub environment: EnvironmentMapHandle,
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            root: Node {
                parent: None,
                children: vec![],
                transform: Matrix4::identity(),
                render_data: RenderDataType::Model(ModelHandle(
                    "assets/local/Lantern/Lantern.json".to_string(),
                )),
            },
            sun: Sun::default(),
            camera: Camera::default(),
            environment: EnvironmentMapHandle("assets/kloofendal_overcast_puresky_8k".to_string()),
        }
    }
}

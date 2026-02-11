use std::{cell::RefCell, rc::Rc, u32};

use glam::{Mat4, Quat, Vec3};

use crate::game::sim::GameTrait;

use super::assets::registry::{ModelHandle, RegistryExt as _, ResourceRegistry, TextureHandle};
use generational_arena::{Arena, Index};

use super::{animator::{self, AnimationGraph, Animator}, camera::Camera};

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct SceneNodeId(pub Index);
impl Into<Index> for SceneNodeId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Clone, Debug)]
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

pub struct StaticModel {
    pub handle: ModelHandle,
    pub last_visible_frame: RefCell<u32>, // temp using interior mutability because cba
}

pub struct AnimatedModel {
    pub model: ModelHandle,
    pub animator: Animator,
    pub last_visible_frame: RefCell<u32>,
}

pub enum RenderDataType {
    Model(StaticModel),
    AnimatedModel(AnimatedModel),
    None,
}

#[derive(Debug)]
pub struct Environment {
    pub sun: Sun,
    pub prefiltered: TextureHandle,
    pub di: TextureHandle,
    pub brdf: TextureHandle,
}
impl Environment {
    pub fn init(resource_registry: &Rc<RefCell<ResourceRegistry>>) -> Self {
        Self {
            sun: Sun::default(),
            prefiltered: resource_registry.request_texture("assets/kloofendal_overcast_puresky_8k.prefiltered.dds", true),
            di: resource_registry.request_texture("assets/kloofendal_overcast_puresky_8k.di.dds", true),
            brdf: resource_registry.request_texture("assets/brdf_lut.png", false),
        }
    }
}

pub struct Node {
    pub parent: Option<SceneNodeId>,
    pub children: Vec<SceneNodeId>,
    pub transform: Mat4,
    pub transform_last_mut: u32,
    pub render_data: RenderDataType,
}
impl Node {
    pub fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    pub fn get_transform_mut(&mut self, frame_index: u32) -> &mut Mat4 {
        self.transform_last_mut = frame_index;
        &mut self.transform
    }
}

pub struct Scene {
    pub root: SceneNodeId,
    pub nodes: Arena<Node>,
    pub camera: Camera,
    pub environment: Environment,
    pub global_time_sec: f32,
}
impl Scene {
    pub fn update(&mut self, resource_registry: &Rc<RefCell<ResourceRegistry>>, animation_graphs: &Vec<AnimationGraph>, node: SceneNodeId, dt: f32, game: &impl GameTrait) {
        game.update(self, resource_registry, animation_graphs, node, dt);
        let node = self.nodes.get_mut(node.into()).unwrap();
        for child_idx in node.children.clone() {
            self.update(resource_registry, animation_graphs, child_idx, dt, game);
        }
    }
}

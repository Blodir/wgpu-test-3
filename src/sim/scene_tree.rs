use std::sync::Arc;

use glam::{Mat4, Vec3};

use crate::resource_manager::{registry::{ModelHandle, TextureHandle}, resource_manager::ResourceManager};
use generational_arena::{Arena, Index};

use super::{animator::{self, AnimationGraph, Animator}, camera::Camera};

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

pub struct AnimatedModel {
    pub model: ModelHandle,
    pub animator: Animator,
}

pub enum RenderDataType {
    Model(ModelHandle),
    AnimatedModel(AnimatedModel),
    None,
}

pub struct Environment {
    pub sun: Sun,
    pub prefiltered: TextureHandle,
    pub di: TextureHandle,
    pub brdf: TextureHandle,
}
impl Environment {
    pub fn init(resource_manager: &Arc<ResourceManager>) -> Self {
        Self {
            sun: Sun::default(),
            prefiltered: resource_manager.request_texture("assets/kloofendal_overcast_puresky_8k.prefiltered.dds", true),
            di: resource_manager.request_texture("assets/kloofendal_overcast_puresky_8k.di.dds", true),
            brdf: resource_manager.request_texture("assets/brdf_lut.png", false),
        }
    }
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
    pub camera: Camera,
    pub environment: Environment,
    pub global_time_sec: f32,
}
impl Scene {
    pub fn update(&mut self, resource_manager: &Arc<ResourceManager>, animation_graphs: &Vec<AnimationGraph>, node: Index, dt: f32) {
        let node = &mut self.nodes[node];
        // TODO remove this after testing
        if (self.global_time_sec % 16.0).abs() < dt {
            self.environment.prefiltered = resource_manager.request_texture("assets/kloofendal_overcast_puresky_8k.prefiltered.dds", true);
            self.environment.di = resource_manager.request_texture("assets/kloofendal_overcast_puresky_8k.di.dds", true);
        }
        if (self.global_time_sec % 16.0 - 8.0).abs() < dt {
            self.environment.prefiltered = resource_manager.request_texture("assets/steinbach_field_4k.prefiltered.dds", true);
            self.environment.di = resource_manager.request_texture("assets/steinbach_field_4k.di.dds", true);
        }
        match &mut node.render_data {
            RenderDataType::None => (),
            RenderDataType::Model(model_handle) => (),
            RenderDataType::AnimatedModel(animated_model) => {
                // TODO remove this after testing
                // automatically transition for fun

                let cycle_duration = 16.0;
                let phase = self.global_time_sec % cycle_duration;
                let a = cycle_duration / 4.0;
                if (phase - 0.0).abs() < dt {
                    // look -> walk
                    if let animator::AnimatorState::State(state) = animated_model.animator.get_current_state() {
                        match animated_model.animator.transition(0) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                } else if (phase - a).abs() < dt {
                    // walk -> run
                    if let animator::AnimatorState::State(state) = animated_model.animator.get_current_state() {
                        match animated_model.animator.transition(2) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                } else if (phase - (2.0 * a)).abs() < dt {
                    // run -> walk
                    if let animator::AnimatorState::State(state) = animated_model.animator.get_current_state() {
                        match animated_model.animator.transition(3) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                } else if (phase - (3.0 * a)).abs() < dt {
                    // walk -> look
                    if let animator::AnimatorState::State(state) = animated_model.animator.get_current_state() {
                        match animated_model.animator.transition(1) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                }

                animated_model.animator.update(animation_graphs, dt);
            },
        }
        for child_idx in node.children.clone() {
            self.update(resource_manager, animation_graphs, child_idx, dt);
        }
    }
}

pub fn build_test_animation_blending(resource_manager: &Arc<ResourceManager>) -> (Scene, Vec<AnimationGraph>) {
    let mut nodes = Arena::new();

    let animation_graph = AnimationGraph {
        states: vec![
            animator::State { clip_idx: 0, time_wrap: animator::TimeWrapMode::Repeat, boundary_mode: animator::BoundaryMode::Closed, speed: 1.0 },
            animator::State { clip_idx: 1, time_wrap: animator::TimeWrapMode::Repeat, boundary_mode: animator::BoundaryMode::Closed, speed: 1.0 },
            animator::State { clip_idx: 2, time_wrap: animator::TimeWrapMode::Repeat, boundary_mode: animator::BoundaryMode::Closed, speed: 1.0 },
        ],
        transitions: vec![
            animator::Transition { blend_time: 0.5, to: 1 }, // look -> walk
            animator::Transition { blend_time: 0.5, to: 0 }, // walk -> look
            animator::Transition { blend_time: 0.5, to: 2 }, // walk -> run
            animator::Transition { blend_time: 0.5, to: 1 }, // run -> walk
        ],
    };
    let animation_graphs = vec![animation_graph];

    let model_handle = resource_manager.request_model("assets/local/Fox/Fox.json");

    let mut children = vec![];
    let grid_size = 50;
    for i in 0..grid_size {
        for j in 0..grid_size {
            let spacing = 200.0;
            let x = i as f32 * spacing - ((grid_size as f32 * spacing) / 2.0);
            let z = j as f32 * spacing - ((grid_size as f32 * spacing) / 2.0);
            let transform = Mat4::from_translation(Vec3::new(x, 0.0, z));
            let render_data = RenderDataType::AnimatedModel(AnimatedModel { model: model_handle.clone(), animator: Animator::new(0, 0) });
            let child = nodes.insert(Node {
                parent: None,
                children: vec![],
                transform,
                render_data,
            });
            children.push(child);
        }
    }

    let root_handle = nodes.insert(Node {
        parent: None,
        children,
        transform: Mat4::IDENTITY,
        render_data: RenderDataType::None,
    });

    let environment = Environment {
        sun: Sun::default(),
        prefiltered: resource_manager.request_texture("assets/kloofendal_overcast_puresky_8k.prefiltered.dds", true),
        di: resource_manager.request_texture("assets/kloofendal_overcast_puresky_8k.di.dds", true),
        brdf: resource_manager.request_texture("assets/brdf_lut.png", false),
    };

    let scene = Scene {
        root: root_handle,
        nodes,
        environment,
        camera: Camera::default(),
        global_time_sec: 0.0
    };

    (scene, animation_graphs)
}

use std::{collections::HashMap, sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};
use generational_arena::Index;
use glam::Mat4;

use crate::{resource_manager::{registry::{ModelId, TextureId}, resource_manager::ResourceManager}, sim::{animator::{AnimationGraph, BoundaryMode, TimeWrapMode}, scene_tree::{Camera, Environment, RenderDataType, Scene, Sun}}};

pub fn accumulate_model_instances(
    scene: &Scene,
    animation_graphs: &Vec<AnimationGraph>,
    models: &mut HashMap<ModelId, HashMap<Index, ModelInstance>>,
    base_transform: &Mat4,
    node_handle: Index,
) {
    let node = scene.nodes.get(node_handle).unwrap();
    let (model_handle, maybe_animation_snapshot) = match &node.render_data {
        RenderDataType::Model(model_handle) => (model_handle, None),
        RenderDataType::AnimatedModel(animated_model) => (&animated_model.model, Some(animated_model.animator.build_snapshot(animation_graphs))),
    };
    let v = models
        .entry(model_handle.id())
        .or_insert_with(HashMap::new);
    let transform = node.transform * base_transform;
    let inst = ModelInstance {
        transform, animation: maybe_animation_snapshot
    };
    v.insert(node_handle, inst);
    for child in &node.children {
        accumulate_model_instances(scene, animation_graphs, models, &transform, *child);
    }
}

pub struct AnimationStateSnapshot {
    pub clip_idx: u8,
    pub time_wrap: TimeWrapMode,
    pub boundary_mode: BoundaryMode,
    /// time in seconds since the transition into this state started
    pub animation_time: f32,
}

pub struct AnimationTransitionSnapshot {
    pub from_clip_idx: u8,
    pub to_clip_idx: u8,
    pub blend_time: f32,
    /// time in seconds since the transition to the previous state started
    pub from_time: f32,
    /// time in seconds since this transition started
    pub to_time: f32,
    pub from_time_wrap: TimeWrapMode,
    pub to_time_wrap: TimeWrapMode,
}

pub enum AnimationSnapshot {
    AnimationStateSnapshot(AnimationStateSnapshot),
    AnimationTransitionSnapshot(AnimationTransitionSnapshot),
}


pub struct ModelInstance {
    pub transform: Mat4,
    pub animation: Option<AnimationSnapshot>,
}

pub struct EnvironmentSnapshot {
    pub sun: Sun,
    pub prefiltered: TextureId,
    pub di: TextureId,
    pub brdf: TextureId,
}
impl EnvironmentSnapshot {
    pub fn from(environment: &Environment) -> Self {
        Self {
            sun: environment.sun.clone(),
            prefiltered: environment.prefiltered.id(),
            di: environment.di.id(),
            brdf: environment.brdf.id(),
        }
    }
}

pub struct RenderSnapshot {
    pub model_instances: HashMap<ModelId, HashMap<Index, ModelInstance>>,
    pub environment: EnvironmentSnapshot,
    pub camera: Camera,
}
impl RenderSnapshot {
    pub fn build(scene: &Scene, animation_graphs: &Vec<AnimationGraph>) -> Self {
        let mut model_instances = HashMap::<ModelId, HashMap<Index, ModelInstance>>::new();
        accumulate_model_instances(scene, animation_graphs, &mut model_instances, &Mat4::IDENTITY, scene.root);

        let environment = EnvironmentSnapshot::from(&scene.environment);
        let camera = scene.camera.clone();
        Self {
            model_instances,
            environment,
            camera,
        }
    }

    pub fn init(resource_manager: &Arc<ResourceManager>) -> Self {
        Self {
            model_instances: HashMap::new(),
            environment: EnvironmentSnapshot::from(&Environment::init(resource_manager)),
            camera: Camera::default(),
        }
    }
}

pub type SnapshotGuard = Guard<Arc<SnapshotPair>>;

#[derive(Clone)]
pub struct SnapshotPair {
    pub prev: Arc<RenderSnapshot>,
    pub prev_timestamp: Instant,
    pub curr: Arc<RenderSnapshot>,
    pub curr_timestamp: Instant,
    pub gen: u64, // optional: monotonic generation
}

pub struct SnapshotHandoff {
    pair: ArcSwap<SnapshotPair>,
}

impl SnapshotHandoff {
    pub fn new(init: RenderSnapshot) -> Self {
        let init = Arc::new(init);
        let pair = SnapshotPair {
            prev: init.clone(),
            prev_timestamp: Instant::now(),
            curr: init,
            curr_timestamp: Instant::now(),
            gen: 0,
        };
        Self {
            pair: ArcSwap::from(Arc::new(pair)),
        }
    }

    /// Producer: publish a new current; previous becomes the old current.
    pub fn publish(&self, snap: RenderSnapshot) {
        let old = self.pair.load(); // coherent view
        let next = SnapshotPair {
            prev: old.curr.clone(),
            prev_timestamp: old.curr_timestamp,
            curr: Arc::new(snap),
            curr_timestamp: Instant::now(),
            gen: old.gen + 1,
        };
        self.pair.store(Arc::new(next)); // atomic pointer swap
    }

    /// Consumer: single atomic load returns a coherent (prev,curr) pair.
    pub fn load(&self) -> SnapshotGuard {
        self.pair.load()
    }
}

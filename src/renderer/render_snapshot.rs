use std::{collections::HashMap, sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};
use generational_arena::Index;
use glam::Mat4;

use crate::scene_tree::{Camera, RenderDataType, Scene, Sun};

use super::render_resources::{EnvironmentMapHandle, ModelHandle};

pub fn accumulate_model_transforms(
    scene: &Scene,
    models: &mut HashMap<ModelHandle, HashMap<Index, ModelInstance>>,
    base_transform: &Mat4,
    node_handle: Index,
) {
    let node = scene.nodes.get(node_handle).unwrap();
    let RenderDataType::Model(model_handle) = &node.render_data;
    let v = models
        .entry(model_handle.clone())
        .or_insert_with(HashMap::new);
    let transform = node.transform * base_transform;
    let duration = 2.0;
    let animation = Some(AnimationInstance {
        clip_idx: 1,
        clip_time: scene.global_time_sec % duration / duration,
    });
    let inst = ModelInstance {
        transform, animation
    };
    v.insert(node_handle, inst);
    for child in &node.children {
        accumulate_model_transforms(scene, models, &transform, *child);
    }
}

pub struct AnimationInstance {
    pub clip_idx: u32,
    /// 0..1
    pub clip_time: f32,
}

pub struct ModelInstance {
    pub transform: Mat4,
    pub animation: Option<AnimationInstance>,
}

pub struct RenderSnapshot {
    pub model_instances: HashMap<ModelHandle, HashMap<Index, ModelInstance>>,
    pub environment_map: EnvironmentMapHandle,
    pub camera: Camera,
    pub sun: Option<Sun>,
}
impl RenderSnapshot {
    pub fn build(scene: &Scene) -> Self {
        let mut model_instances = HashMap::<ModelHandle, HashMap<Index, ModelInstance>>::new();
        accumulate_model_transforms(scene, &mut model_instances, &Mat4::IDENTITY, scene.root);

        // TODO dirty check
        let environment_map = scene.environment.clone();
        let camera = scene.camera.clone();
        let sun = Some(scene.sun.clone());
        Self {
            model_instances,
            environment_map,
            camera,
            sun,
        }
    }
}
impl Default for RenderSnapshot {
    fn default() -> Self {
        Self {
            model_instances: HashMap::new(),
            environment_map: EnvironmentMapHandle(
                "assets/kloofendal_overcast_puresky_8k".to_string(),
            ),
            camera: Camera::default(),
            sun: None,
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
    pub fn new() -> Self {
        let init = Arc::new(RenderSnapshot::default());
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

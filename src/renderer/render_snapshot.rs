use std::{collections::HashMap, sync::Arc};

use arc_swap::{ArcSwap, Guard};
use generational_arena::Index;
use glam::Mat4;

use crate::scene_tree::{Camera, RenderDataType, Scene, Sun};

use super::render_resources::{EnvironmentMapHandle, ModelHandle};

pub fn accumulate_model_transforms(
    scene: &Scene,
    models: &mut HashMap<ModelHandle, HashMap<Index, Mat4>>,
    base_transform: &Mat4,
    node_handle: Index,
) {
    let node = scene.nodes.get(node_handle).unwrap();
    let RenderDataType::Model(model_handle) = &node.render_data;
    let v = models
        .entry(model_handle.clone())
        .or_insert_with(HashMap::new);
    let transform = node.transform * base_transform;
    v.insert(node_handle, transform);
    for child in &node.children {
        accumulate_model_transforms(scene, models, &transform, *child);
    }
}

pub struct RenderSnapshot {
    pub model_transforms: HashMap<ModelHandle, HashMap<Index, Mat4>>,
    pub environment_map: EnvironmentMapHandle,
    pub camera: Option<Camera>,
    pub sun: Option<Sun>,
}
impl RenderSnapshot {
    pub fn build(scene: &Scene) -> Self {
        let mut model_transforms = HashMap::<ModelHandle, HashMap<Index, Mat4>>::new();
        accumulate_model_transforms(scene, &mut model_transforms, &Mat4::IDENTITY, scene.root);

        // TODO dirty check
        let environment_map = scene.environment.clone();
        let camera = Some(scene.camera.clone());
        let sun = Some(scene.sun.clone());
        Self {
            model_transforms,
            environment_map,
            camera,
            sun,
        }
    }
}
impl Default for RenderSnapshot {
    fn default() -> Self {
        Self {
            model_transforms: HashMap::new(),
            environment_map: EnvironmentMapHandle(
                "assets/kloofendal_overcast_puresky_8k".to_string(),
            ),
            camera: None,
            sun: None,
        }
    }
}

pub type SnapshotGuard = Guard<Arc<RenderSnapshot>>;

pub struct SnapshotHandoff {
    current: ArcSwap<RenderSnapshot>,
    previous: ArcSwap<RenderSnapshot>,
}
impl SnapshotHandoff {
    pub fn new() -> Self {
        Self {
            current: ArcSwap::from_pointee(RenderSnapshot::default()),
            previous: ArcSwap::from_pointee(RenderSnapshot::default()),
        }
    }

    pub fn publish(&self, snap: RenderSnapshot) {
        // this swap probably doesn't have to be atomic, since it's fine if the render thread
        // reads (current, current) for one frame
        self.previous.store(self.current.load_full());
        self.current.store(Arc::new(snap));
    }

    pub fn load(&self) -> (SnapshotGuard, SnapshotGuard) {
        (self.previous.load(), self.current.load())
    }
}

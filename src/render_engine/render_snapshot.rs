use std::collections::HashMap;

use generational_arena::Index;
use glam::Mat4;

use crate::scene_tree::{Camera, RenderDataType, Scene, Sun};

use super::render_resources::{EnvironmentMapHandle, ModelHandle};

pub fn accumulate_model_transforms(
    scene: &Scene,
    models: &mut HashMap<ModelHandle, Vec<Mat4>>,
    base_transform: &Mat4,
    node_handle: Index,
) {
    let node = scene.nodes.get(node_handle).unwrap();
    let RenderDataType::Model(model_handle) = &node.render_data;
    let v = models.entry(model_handle.clone()).or_insert_with(Vec::new);
    let transform = node.transform * base_transform;
    v.push(transform);
    for child in &node.children {
        accumulate_model_transforms(scene, models, &transform, *child);
    }
}

pub struct RenderSnapshot {
    pub model_transforms: HashMap<ModelHandle, Vec<Mat4>>,
    pub environment_map: EnvironmentMapHandle,
    pub camera: Option<Camera>,
    pub sun: Option<Sun>,
}
impl RenderSnapshot {
    pub fn build(scene: &Scene) -> Self {
        let mut model_transforms = HashMap::<ModelHandle, Vec<Mat4>>::new();
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

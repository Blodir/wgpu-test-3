use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc, sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};
use generational_arena::Index;
use glam::{Mat4, Quat, Vec3};

use crate::{renderer::pose_storage::TRS, resource_system::{game_resources::{self, GameResources}, registry::{GameState, ModelId, RenderState, ResourceRegistry, TextureId}, render_resources::{MaterialRenderId, MeshRenderId, ModelRenderId, TextureRenderId}, resource_manager::ResourceManager}, sim::{animator::{AnimationGraph, BoundaryMode, TimeWrapMode}, camera::{frustum_intersects_aabb_world, Camera, Frustum}, scene_tree::{Environment, RenderDataType, Scene, SceneNodeId, Sun}}};

pub fn accumulate_instance_snapshots(
    scene: &Scene,
    animation_graphs: &Vec<AnimationGraph>,
    instances: &mut HashMap<SceneNodeId, ModelInstanceSnapshot>,
    base_transform: &Mat4,
    node_id: SceneNodeId,
    frustum: &Frustum,
    resource_registry: &Rc<RefCell<ResourceRegistry>>,
    game_resources: &GameResources,
    frame_index: u32,
) {
    let node = scene.nodes.get(node_id.into()).unwrap();
    let transform = node.transform * base_transform;
    let (model_handle, last_visible_frame, maybe_animation_snapshot) = match &node.render_data {
        RenderDataType::None => {
            for child in &node.children {
                accumulate_instance_snapshots(scene, animation_graphs, instances, &transform, *child, frustum, resource_registry, game_resources, frame_index);
            }
            return;
        },
        RenderDataType::Model(static_model) => (&static_model.handle, *static_model.last_visible_frame.borrow(), None),
        RenderDataType::AnimatedModel(animated_model) => (&animated_model.model, *animated_model.last_visible_frame.borrow(), Some(animated_model.animator.build_snapshot())),
    };

    let reg = resource_registry.borrow();
    let entry = reg.get(model_handle);
    if let (
        GameState::Ready(model_game_id),
    ) = (
        &entry.game_state,
    ) {
        let model_game = game_resources.models.get(*model_game_id).unwrap();
        // render everything that was visible on the previous frame to reduce popping when the camera moves fast
        let last_frame_visible = frame_index.wrapping_sub(last_visible_frame) <= 1;
        let intersect = frustum_intersects_aabb_world(frustum, &model_game.aabb, &transform);
        if last_frame_visible || intersect {
            if intersect {
                match &scene.nodes.get(node_id.into()).unwrap().render_data {
                    RenderDataType::Model(static_model) => static_model.last_visible_frame.replace(frame_index),
                    RenderDataType::AnimatedModel(animated_model) => animated_model.last_visible_frame.replace(frame_index),
                    RenderDataType::None => 0u32,
                };
            }
            let mut submesh_transforms = vec![];
            for submesh in &model_game.submesh_instances {
                let mut sub = vec![];
                for submesh_instance in submesh {
                    let (s, r, t) = (transform * submesh_instance).to_scale_rotation_translation();
                    sub.push(TRS { t, r, s });
                }
                submesh_transforms.push(sub);
            }
            if !submesh_transforms.is_empty() {
                let inst = ModelInstanceSnapshot {
                    submesh_transforms,
                    animation: maybe_animation_snapshot,
                };
                instances.insert(node_id, inst);
            }
        }
    }

    for child in &node.children {
        accumulate_instance_snapshots(scene, animation_graphs, instances, &transform, *child, frustum, resource_registry, game_resources, frame_index);
    }
}

pub struct CameraSnapshot {
    pub position: Vec3,
    pub rotation: Quat,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}
impl Default for CameraSnapshot {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fovy: 45.0f32,
            znear: 0.1f32,
            zfar: 100.0f32,
        }
    }
}

/*
pub struct AnimationStateSnapshot {
    pub clip_id: AnimationClipRenderId,
    pub time_wrap: TimeWrapMode,
    pub boundary_mode: BoundaryMode,
    /// time in seconds since the transition into this state started
    pub animation_time: f32,
}

pub struct AnimationTransitionSnapshot {
    pub from_clip_id: AnimationClipRenderId,
    pub to_clip_id: AnimationClipRenderId,
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
 */

pub struct AnimationSnapshot(pub u64);

pub struct ModelInstanceSnapshot {
    pub submesh_transforms: Vec<Vec<TRS>>,
    pub animation: Option<AnimationSnapshot>,
}

#[derive(Debug)]
pub struct EnvironmentMapSnapshot {
    pub prefiltered: TextureRenderId,
    pub di: TextureRenderId,
    pub brdf: TextureRenderId,
}

pub struct EnvironmentSnapshot {
    pub sun: Sun,
    pub environment_map: Option<EnvironmentMapSnapshot>,
}
impl EnvironmentSnapshot {
    pub fn from(environment: &Environment, resource_registry: &Rc<RefCell<ResourceRegistry>>) -> Self {
        if let (
            RenderState::Ready(prefiltered_render_id),
            RenderState::Ready(di_render_id),
            RenderState::Ready(brdf_render_id),
        ) = (
            &resource_registry.borrow().get(&environment.prefiltered).render_state,
            &resource_registry.borrow().get(&environment.di).render_state,
            &resource_registry.borrow().get(&environment.brdf).render_state,
        ) {
            Self {
                sun: environment.sun.clone(),
                environment_map: Some(EnvironmentMapSnapshot {
                    prefiltered: TextureRenderId(*prefiltered_render_id),
                    di: TextureRenderId(*di_render_id),
                    brdf: TextureRenderId(*brdf_render_id),
                })
            }
        } else {
            Self {
                sun: environment.sun.clone(),
                environment_map: None,
            }
        }
    }
}

pub struct SubmeshBatch {
    pub instances: Vec<SceneNodeId>,
    pub submesh_idx: usize,
}

pub struct MeshBatch {
    pub model_id: ModelRenderId,
    pub submesh_range: std::ops::Range<usize>, // indexes into both draws and instance ranges arrays
}

pub struct MaterialBatch {
    pub material_id: MaterialRenderId,
    pub mesh_range: std::ops::Range<usize>,
}

#[derive(Default)]
pub struct SkinnedMeshDrawSnapshot {
    pub submesh_batches: Vec<SubmeshBatch>,
    pub material_batches: Vec<MaterialBatch>,
    pub mesh_batches: Vec<MeshBatch>,
}
impl SkinnedMeshDrawSnapshot {
    fn build(
        scene: &Scene,
        instances: &HashMap<SceneNodeId, ModelInstanceSnapshot>,
        resource_registry: &Rc<RefCell<ResourceRegistry>>,
        game_resources: &GameResources,
    ) -> Self {
        let reg = resource_registry.borrow();
        let mut materials: HashMap<MaterialRenderId, HashMap<ModelRenderId, Vec<Vec<SceneNodeId>>>> = HashMap::new();

        // correct rendered nodes in hashmaps
        for (node_id, snap) in instances {
            let node = scene.nodes.get((*node_id).into()).unwrap();
            let model_handle = match &node.render_data {
                RenderDataType::Model(static_model) => &static_model.handle,
                RenderDataType::AnimatedModel(animated_model) => &animated_model.model,
                RenderDataType::None => panic!(),
            };
            if let (
                GameState::Ready(model_game_id),
                RenderState::Ready(model_render_id),
            ) = (
                &reg.get(&model_handle).game_state,
                &reg.get(&model_handle).render_state,
            ) {
                let model_game_data = game_resources.models.get(*model_game_id).unwrap();

                for submesh_idx in 0..model_game_data.manifest.primitives.len() {
                    let submesh = &model_game_data.manifest.primitives[submesh_idx];
                    let mat_handle = &model_game_data.materials[submesh.material as usize];
                    if let RenderState::Ready(mat_render_id) = resource_registry.borrow().get(&mat_handle).render_state {
                        let models = materials.entry(MaterialRenderId(mat_render_id)).or_insert(HashMap::new());
                        let submeshes = models.entry(ModelRenderId(*model_render_id)).or_insert(vec![vec![]; model_game_data.manifest.primitives.len()]);
                        submeshes[submesh_idx].push(*node_id);
                    }
                }
            }
        }

        // build batches from hashmaps
        let mut material_batches: Vec<MaterialBatch> = vec![];
        let mut mesh_batches: Vec<MeshBatch> = vec![];
        let mut submesh_batches: Vec<SubmeshBatch> = vec![];

        for (mat_render_id, models) in materials.iter_mut() {
            let mat_batch = MaterialBatch {
                material_id: *mat_render_id,
                mesh_range: mesh_batches.len()..models.len(),
            };
            material_batches.push(mat_batch);
            for (model_render_id, submeshes) in models.iter_mut() {
                let mesh_batch = MeshBatch {
                    model_id: *model_render_id,
                    submesh_range: submesh_batches.len()..submeshes.len(),
                };
                mesh_batches.push(mesh_batch);
                let mut i = 0;
                for instances in submeshes.drain(..) {
                    let submesh_batch = SubmeshBatch {
                        instances,
                        submesh_idx: i,
                    };
                    submesh_batches.push(submesh_batch);

                    i += 1;
                }
            }
        }

        Self {
            submesh_batches,
            material_batches,
            mesh_batches,
        }
    }
}

pub struct RenderSnapshot {
    pub skinned_draw_snapshot: SkinnedMeshDrawSnapshot,
    pub model_instances: HashMap<SceneNodeId, ModelInstanceSnapshot>,
    pub environment: EnvironmentSnapshot,
    pub camera: CameraSnapshot,
}
impl RenderSnapshot {
    pub fn build(scene: &mut Scene, resource_registry: &Rc<RefCell<ResourceRegistry>>, animation_graphs: &Vec<AnimationGraph>, game_resources: &GameResources, frame_index: u32) -> Self {
        let mut model_instances = HashMap::<SceneNodeId, ModelInstanceSnapshot>::new();
        let frustum = scene.camera.build_frustum();
        accumulate_instance_snapshots(scene, animation_graphs, &mut model_instances, &Mat4::IDENTITY, scene.root, &frustum, resource_registry, game_resources, frame_index);
        let skinned_draw_snapshot = SkinnedMeshDrawSnapshot::build(scene, &model_instances, resource_registry, game_resources);

        let environment = EnvironmentSnapshot::from(&scene.environment, resource_registry);
        let camera = scene.camera.build_snapshot();
        Self {
            model_instances,
            skinned_draw_snapshot,
            environment,
            camera,
        }
    }

    pub fn init() -> Self {
        Self {
            model_instances: HashMap::new(),
            environment: EnvironmentSnapshot { sun: Sun::default(), environment_map: None },
            camera: Camera::default().build_snapshot(),
            skinned_draw_snapshot: SkinnedMeshDrawSnapshot::default(),
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

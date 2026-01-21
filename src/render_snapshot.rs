use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc, sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};
use generational_arena::Index;
use glam::{Mat4, Quat, Vec3};

use crate::{resource_system::{registry::{ModelId, ResourceRegistry, TextureId}, render_resources::{AnimationClipRenderId, MaterialRenderId, MeshRenderId, ModelRenderId, SkeletonRenderId, TextureRenderId}, resource_manager::ResourceManager}, sim::{animator::{AnimationGraph, BoundaryMode, TimeWrapMode}, camera::{frustum_intersects_aabb_world, Camera, Frustum}, scene_tree::{Environment, RenderDataType, Scene, SceneNodeId, Sun}}};

pub fn accumulate_model_instances(
    scene: &Scene,
    animation_graphs: &Vec<AnimationGraph>,
    models: &mut HashMap<ModelRenderId, HashMap<SceneNodeId, ModelInstance>>,
    base_transform: &Mat4,
    node_handle: Index,
    frustum: &Frustum,
    resource_registry: &Rc<RefCell<ResourceRegistry>>,
) {
    let node = scene.nodes.get(node_handle).unwrap();
    let transform = node.transform * base_transform;
    let (model_handle, maybe_animation_snapshot) = match &node.render_data {
        RenderDataType::None => {
            for child in &node.children {
                accumulate_model_instances(scene, animation_graphs, models, &transform, *child, frustum, resource_registry);
            }
            return;
        },
        RenderDataType::Model(model_handle) => (model_handle, None),
        RenderDataType::AnimatedModel(animated_model) => (&animated_model.model, Some(animated_model.animator.build_snapshot(animation_graphs))),
    };

    let v = models
        .entry(model_handle.id())
        .or_insert_with(HashMap::new);

    match resource_registry.registry.lock().unwrap().get(model_handle).game_state {
        crate::resource_system::registry::GameState::Ready(index) => {
            if let Some(model) = resource_registry.cpu.models.lock().unwrap().get(index) {
                if frustum_intersects_aabb_world(frustum, &model.manifest.aabb, &transform) {
                    let inst = ModelInstance {
                        transform, animation: maybe_animation_snapshot
                    };
                    v.insert(node_handle, inst);
                }
            }
        },
        _ => (),
    }
    for child in &node.children {
        accumulate_model_instances(scene, animation_graphs, models, &transform, *child, frustum, resource_registry);
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

pub struct ModelInstance {
    pub transform: Mat4,
    pub animation: Option<AnimationSnapshot>,
    pub skeleton: SkeletonRenderId,
}

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
    pub fn from(environment: &Environment) -> Self {
        Self {
            sun: environment.sun.clone(),
            prefiltered: environment.prefiltered.id(),
            di: environment.di.id(),
            brdf: environment.brdf.id(),
        }
    }
}

pub struct SubmeshDraw {
    pub index_range: Range<u32>,
    pub base_vertex: i32,
    pub node: SceneNodeId,
}

pub struct MeshBatch {
    pub mesh: MeshRenderId,
    pub vertex_buffer_start_offset: u64,
    pub draw_range: std::ops::Range<usize>, // indexes into both draws and instance ranges arrays
}

pub struct MaterialBatch {
    pub material: MaterialRenderId,
    pub mesh_range: std::ops::Range<usize>,
}

pub struct SkinnedMeshDrawSnapshot {
    pub draws: Vec<SubmeshDraw>,
    pub material_batches: Vec<MaterialBatch>,
    pub mesh_batches: Vec<MeshBatch>,
}

pub struct RenderSnapshot {
    pub skinned_draw_snapshot: SkinnedMeshDrawSnapshot,
    pub model_instances: HashMap<SceneNodeId, ModelInstance>,
    pub environment: EnvironmentSnapshot,
    pub camera: CameraSnapshot,
}
impl RenderSnapshot {
    pub fn build(scene: &Scene, resource_registry: &Rc<RefCell<ResourceRegistry>>, animation_graphs: &Vec<AnimationGraph>) -> Self {
        let mut model_instances = HashMap::<ModelRenderId, HashMap<SceneNodeId, ModelInstance>>::new();
        let frustum = scene.camera.build_frustum();
        accumulate_model_instances(scene, animation_graphs, &mut model_instances, &Mat4::IDENTITY, scene.root, &frustum, resource_registry);

        let environment = EnvironmentSnapshot::from(&scene.environment);
        let camera = scene.camera.build_snapshot();
        Self {
            model_instances,
            environment,
            camera,
        }
    }

    pub fn init() -> Self {
        Self {
            model_instances: HashMap::new(),
            environment: EnvironmentSnapshot { sun: Sun::default(), environment_map: None },
            camera: Camera::default().build_snapshot(),
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

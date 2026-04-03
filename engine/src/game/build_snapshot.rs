use std::{cell::RefCell, collections::HashMap, rc::Rc};

use glam::{Mat4, Quat, Vec3};

use crate::game::assets::registry::{GameState, RenderState, ResourceRegistry};
use crate::game::assets::store::GameAssetStore;
use crate::host::assets::io::asset_formats::{materialfile, modelfile, rigfile::SRT};
use crate::{
    game::{
        animator::AnimationGraph,
        camera::{frustum_intersects_aabb_world, Frustum},
        scene_tree::{Environment, RenderDataType, Scene, SceneNodeId, Sun},
    },
    host::assets::store::{MaterialRenderId, ModelRenderId, TextureRenderId},
    host::world::pipelines::MeshPipelineKind,
};

pub fn accumulate_instance_snapshots(
    scene: &Scene,
    animation_graphs: &Vec<AnimationGraph>,
    skinned_instances: &mut Vec<SkinnedInstanceSnapshot>,
    static_instances: &mut Vec<StaticInstanceSnapshot>,
    point_lights: &mut Vec<PointLightSnapshot>,
    base_transform: &Mat4,
    node_id: SceneNodeId,
    frustum: &Frustum,
    resource_registry: &Rc<RefCell<ResourceRegistry>>,
    game_resources: &GameAssetStore,
    tick_index: u32,
) {
    let node = scene.nodes.get(node_id.into()).unwrap();
    let transform = node.get_transform() * base_transform;
    let (model_handle, last_visible_frame, maybe_animation_snapshot) = match &node.render_data {
        RenderDataType::None => {
            for child in &node.children {
                accumulate_instance_snapshots(
                    scene,
                    animation_graphs,
                    skinned_instances,
                    static_instances,
                    point_lights,
                    &transform,
                    *child,
                    frustum,
                    resource_registry,
                    game_resources,
                    tick_index,
                );
            }
            return;
        }
        RenderDataType::Model(static_model) => (
            &static_model.handle,
            *static_model.last_visible_frame.borrow(),
            None,
        ),
        RenderDataType::AnimatedModel(animated_model) => (
            &animated_model.model,
            *animated_model.last_visible_frame.borrow(),
            Some(animated_model.animator.build_snapshot()),
        ),
        RenderDataType::PointLight(light) => {
            let (_scale, _rotation, translation) = transform.to_scale_rotation_translation();
            point_lights.push(PointLightSnapshot {
                position: translation,
                color: light.color,
                intensity: light.intensity,
                range: light.range,
            });
            for child in &node.children {
                accumulate_instance_snapshots(
                    scene,
                    animation_graphs,
                    skinned_instances,
                    static_instances,
                    point_lights,
                    &transform,
                    *child,
                    frustum,
                    resource_registry,
                    game_resources,
                    tick_index,
                );
            }
            return;
        }
    };

    let reg = resource_registry.borrow();
    let entry = reg.get(model_handle);
    if let (GameState::Ready(model_game_id),) = (&entry.game_state,) {
        let model_game = game_resources.models.get(*model_game_id).unwrap();
        // render everything that was visible on the previous frame to reduce popping when the camera moves fast
        let last_frame_visible = tick_index.wrapping_sub(last_visible_frame) <= 1;
        let intersect = frustum_intersects_aabb_world(frustum, &model_game.aabb, &transform);
        if last_frame_visible || intersect {
            if intersect {
                match &scene.nodes.get(node_id.into()).unwrap().render_data {
                    RenderDataType::Model(static_model) => {
                        static_model.last_visible_frame.replace(tick_index)
                    }
                    RenderDataType::AnimatedModel(animated_model) => {
                        animated_model.last_visible_frame.replace(tick_index)
                    }
                    RenderDataType::PointLight(_point_light) => 0u32,
                    RenderDataType::None => 0u32,
                };
            }
            let (s, r, t) = transform.to_scale_rotation_translation();
            let model_transform = SRT::new(s, r, t);
            match model_game.manifest.deformation {
                modelfile::Deformation::None => {
                    let inst = StaticInstanceSnapshot {
                        node_id,
                        model_transform,
                        animation: maybe_animation_snapshot,
                        dirty: node.transform_last_mut == tick_index,
                        prev_index: None,
                    };
                    static_instances.push(inst);
                }
                modelfile::Deformation::Skinned => {
                    let inst = SkinnedInstanceSnapshot {
                        node_id,
                        model_transform,
                        animation: maybe_animation_snapshot,
                        dirty: node.transform_last_mut == tick_index,
                        prev_index: None,
                    };
                    skinned_instances.push(inst);
                }
            }
        }
    }

    for child in &node.children {
        accumulate_instance_snapshots(
            scene,
            animation_graphs,
            skinned_instances,
            static_instances,
            point_lights,
            &transform,
            *child,
            frustum,
            resource_registry,
            game_resources,
            tick_index,
        );
    }
}

#[derive(Clone, Copy)]
pub struct PointLightSnapshot {
    pub position: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

#[derive(Clone, Copy)]
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

#[derive(Debug)]
pub struct EnvironmentMapSnapshot {
    pub prefiltered: TextureRenderId,
    pub di: TextureRenderId,
}

pub struct LightsSnapshot {
    pub sun: Sun,
    pub environment_map_intensity: f32,
    pub point_lights: Vec<PointLightSnapshot>,
    pub environment_map: Option<EnvironmentMapSnapshot>,
}
impl LightsSnapshot {
    pub fn from(
        environment: &Environment,
        resource_registry: &Rc<RefCell<ResourceRegistry>>,
        point_lights: Vec<PointLightSnapshot>,
    ) -> Self {
        if let (RenderState::Ready(prefiltered_render_id), RenderState::Ready(di_render_id)) = (
            &resource_registry
                .borrow()
                .get(&environment.prefiltered)
                .render_state,
            &resource_registry.borrow().get(&environment.di).render_state,
        ) {
            Self {
                sun: environment.sun.clone(),
                environment_map_intensity: environment.environment_map_intensity,
                point_lights,
                environment_map: Some(EnvironmentMapSnapshot {
                    prefiltered: TextureRenderId(*prefiltered_render_id),
                    di: TextureRenderId(*di_render_id),
                }),
            }
        } else {
            Self {
                sun: environment.sun.clone(),
                environment_map_intensity: environment.environment_map_intensity,
                point_lights,
                environment_map: None,
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct AnimationSnapshot(pub u64);

pub struct SkinnedInstanceSnapshot {
    pub node_id: SceneNodeId,
    pub model_transform: SRT,
    pub animation: Option<AnimationSnapshot>,
    pub dirty: bool,
    pub prev_index: Option<u32>,
}

pub struct StaticInstanceSnapshot {
    pub node_id: SceneNodeId,
    pub model_transform: SRT,
    pub animation: Option<AnimationSnapshot>,
    pub dirty: bool,
    pub prev_index: Option<u32>,
}

pub struct SubmeshBatch {
    pub instances: Vec<u32>, // indexes into skinned_instances/static_instances
    pub submesh_idx: usize,
}

pub struct MeshBatch {
    pub model_id: ModelRenderId,
    pub submesh_range: std::ops::Range<usize>, // indexes into both submesh and instance ranges arrays
}

pub struct MaterialBatch {
    pub material_id: MaterialRenderId,
    pub mesh_range: std::ops::Range<usize>,
}

#[derive(Default)]
pub struct PassBatches {
    pub submesh_batches: Vec<SubmeshBatch>,
    pub material_batches: Vec<MaterialBatch>, // indexes into mesh batches
    pub mesh_batches: Vec<MeshBatch>,         // indexes into submesh batches
    pub skinned_batch: std::ops::Range<usize>, // indexes into material batches
    pub static_batch: std::ops::Range<usize>, // indexes into material batches
}

#[derive(Default)]
pub struct MeshDrawSnapshot {
    pub opaque_batch: PassBatches,
    pub transparent_batch: PassBatches,
    pub skinned_instances: Vec<SkinnedInstanceSnapshot>,
    pub static_instances: Vec<StaticInstanceSnapshot>,
}
impl MeshDrawSnapshot {
    fn build_pass_batches(
        pipelines: &mut HashMap<
            MeshPipelineKind,
            HashMap<MaterialRenderId, HashMap<ModelRenderId, Vec<Vec<u32>>>>,
        >,
    ) -> PassBatches {
        let mut mat_offset = 0usize;
        let mut skinned_batch = 0..0;
        let mut static_batch = 0..0;
        let mut material_batches: Vec<MaterialBatch> = vec![];
        let mut mesh_batches: Vec<MeshBatch> = vec![];
        let mut submesh_batches: Vec<SubmeshBatch> = vec![];

        for (pipeline_kind, materials) in pipelines.iter_mut() {
            for (mat_render_id, models) in materials.iter_mut() {
                let mesh_batches_len = mesh_batches.len();
                let mat_batch = MaterialBatch {
                    material_id: *mat_render_id,
                    mesh_range: mesh_batches_len..mesh_batches_len + models.len(),
                };
                material_batches.push(mat_batch);
                for (model_render_id, submeshes) in models.iter_mut() {
                    let submesh_batches_len = submesh_batches.len();
                    let mesh_batch = MeshBatch {
                        model_id: *model_render_id,
                        submesh_range: submesh_batches_len..submesh_batches_len + submeshes.len(),
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
            match pipeline_kind {
                MeshPipelineKind::StaticPbr => {
                    static_batch = mat_offset..material_batches.len();
                    mat_offset = material_batches.len();
                }
                MeshPipelineKind::SkinnedPbr => {
                    skinned_batch = mat_offset..material_batches.len();
                    mat_offset = material_batches.len();
                }
            }
        }

        PassBatches {
            submesh_batches,
            material_batches,
            mesh_batches,
            skinned_batch,
            static_batch,
        }
    }

    pub fn link_previous(&mut self, prev: &MeshDrawSnapshot) {
        let mut prev_skinned_by_node = HashMap::new();
        for (idx, inst) in prev.skinned_instances.iter().enumerate() {
            prev_skinned_by_node.insert(inst.node_id, idx as u32);
        }
        for inst in &mut self.skinned_instances {
            inst.prev_index = prev_skinned_by_node.get(&inst.node_id).copied();
        }

        let mut prev_static_by_node = HashMap::new();
        for (idx, inst) in prev.static_instances.iter().enumerate() {
            prev_static_by_node.insert(inst.node_id, idx as u32);
        }
        for inst in &mut self.static_instances {
            inst.prev_index = prev_static_by_node.get(&inst.node_id).copied();
        }
    }

    fn build(
        scene: &Scene,
        resource_registry: &Rc<RefCell<ResourceRegistry>>,
        game_resources: &GameAssetStore,
        animation_graphs: &Vec<AnimationGraph>,
        tick_index: u32,
    ) -> (Self, Vec<PointLightSnapshot>) {
        let mut skinned_instances = Vec::<SkinnedInstanceSnapshot>::new();
        let mut static_instances = Vec::<StaticInstanceSnapshot>::new();
        let mut point_lights = Vec::<PointLightSnapshot>::new();
        let frustum = scene.camera.build_frustum();
        accumulate_instance_snapshots(
            scene,
            animation_graphs,
            &mut skinned_instances,
            &mut static_instances,
            &mut point_lights,
            &Mat4::IDENTITY,
            scene.root,
            &frustum,
            resource_registry,
            game_resources,
            tick_index,
        );

        let reg = resource_registry.borrow();
        let mut opaque_pipelines: HashMap<
            MeshPipelineKind,
            HashMap<MaterialRenderId, HashMap<ModelRenderId, Vec<Vec<u32>>>>,
        > = HashMap::new();
        let mut transparent_pipelines: HashMap<
            MeshPipelineKind,
            HashMap<MaterialRenderId, HashMap<ModelRenderId, Vec<Vec<u32>>>>,
        > = HashMap::new();

        // collect rendered nodes in hashmaps
        for (instance_idx, inst) in skinned_instances.iter().enumerate() {
            let node = scene.nodes.get(inst.node_id.into()).unwrap();
            let model_handle = match &node.render_data {
                RenderDataType::Model(_static_model) => panic!(),
                RenderDataType::AnimatedModel(animated_model) => &animated_model.model,
                RenderDataType::PointLight(_point_light) => panic!(),
                RenderDataType::None => panic!(),
            };
            if let (GameState::Ready(model_game_id), RenderState::Ready(model_render_id)) = (
                &reg.get(&model_handle).game_state,
                &reg.get(&model_handle).render_state,
            ) {
                let model_game_data = game_resources.models.get(*model_game_id).unwrap();

                for submesh_idx in 0..model_game_data.manifest.submeshes.len() {
                    let submesh = &model_game_data.manifest.submeshes[submesh_idx];
                    let mat_handle = &submesh
                        .material
                        .as_ref()
                        .map(|m| &model_game_data.materials[*m as usize])
                        .unwrap_or(&game_resources.placeholders.material);
                    if let RenderState::Ready(mat_render_id) = reg.get(&mat_handle).render_state {
                        let is_transparent = match &reg.get(&mat_handle).game_state {
                            GameState::Ready(mat_game_id) => game_resources
                                .materials
                                .get(*mat_game_id)
                                .map(|mat_game_data| {
                                    matches!(
                                        mat_game_data.manifest.alpha_mode,
                                        materialfile::AlphaMode::Blend
                                    )
                                })
                                .unwrap_or(false),
                            _ => false,
                        };
                        let pipelines = if is_transparent {
                            &mut transparent_pipelines
                        } else {
                            &mut opaque_pipelines
                        };
                        let materials = pipelines
                            .entry(MeshPipelineKind::SkinnedPbr)
                            .or_insert(HashMap::new());
                        let models = materials
                            .entry(MaterialRenderId(mat_render_id))
                            .or_insert(HashMap::new());
                        let submeshes = models
                            .entry(ModelRenderId(*model_render_id))
                            .or_insert(vec![vec![]; model_game_data.manifest.submeshes.len()]);
                        submeshes[submesh_idx].push(instance_idx as u32);
                    }
                }
            }
        }

        for (instance_idx, inst) in static_instances.iter().enumerate() {
            let node = scene.nodes.get(inst.node_id.into()).unwrap();
            let model_handle = match &node.render_data {
                RenderDataType::Model(static_model) => &static_model.handle,
                RenderDataType::AnimatedModel(animated_model) => &animated_model.model,
                RenderDataType::PointLight(_point_light) => panic!(),
                RenderDataType::None => panic!(),
            };
            if let (GameState::Ready(model_game_id), RenderState::Ready(model_render_id)) = (
                &reg.get(&model_handle).game_state,
                &reg.get(&model_handle).render_state,
            ) {
                let model_game_data = game_resources.models.get(*model_game_id).unwrap();

                for submesh_idx in 0..model_game_data.manifest.submeshes.len() {
                    let submesh = &model_game_data.manifest.submeshes[submesh_idx];
                    let mat_handle = submesh
                        .material
                        .as_ref()
                        .map(|m| &model_game_data.materials[*m as usize])
                        .unwrap_or(&game_resources.placeholders.material);
                    if let RenderState::Ready(mat_render_id) = reg.get(&mat_handle).render_state {
                        let is_transparent = match &reg.get(&mat_handle).game_state {
                            GameState::Ready(mat_game_id) => game_resources
                                .materials
                                .get(*mat_game_id)
                                .map(|mat_game_data| {
                                    matches!(
                                        mat_game_data.manifest.alpha_mode,
                                        materialfile::AlphaMode::Blend
                                    )
                                })
                                .unwrap_or(false),
                            _ => false,
                        };
                        let pipelines = if is_transparent {
                            &mut transparent_pipelines
                        } else {
                            &mut opaque_pipelines
                        };
                        let materials = pipelines
                            .entry(MeshPipelineKind::StaticPbr)
                            .or_insert(HashMap::new());
                        let models = materials
                            .entry(MaterialRenderId(mat_render_id))
                            .or_insert(HashMap::new());
                        let submeshes = models
                            .entry(ModelRenderId(*model_render_id))
                            .or_insert(vec![vec![]; model_game_data.manifest.submeshes.len()]);
                        submeshes[submesh_idx].push(instance_idx as u32);
                    }
                }
            }
        }

        let opaque_batch = Self::build_pass_batches(&mut opaque_pipelines);
        let transparent_batch = Self::build_pass_batches(&mut transparent_pipelines);

        (
            Self {
                opaque_batch,
                transparent_batch,
                skinned_instances,
                static_instances,
            },
            point_lights,
        )
    }
}

pub struct FixedSnapshot {
    pub mesh_draw_snapshot: MeshDrawSnapshot,
    pub lights: LightsSnapshot,
}
impl FixedSnapshot {
    pub fn build(
        scene: &mut Scene,
        resource_registry: &Rc<RefCell<ResourceRegistry>>,
        animation_graphs: &Vec<AnimationGraph>,
        game_resources: &GameAssetStore,
        tick_index: u32,
    ) -> Self {
        let (mesh_draw_snapshot, point_lights) = MeshDrawSnapshot::build(
            scene,
            resource_registry,
            game_resources,
            animation_graphs,
            tick_index,
        );

        let environment = LightsSnapshot::from(&scene.environment, resource_registry, point_lights);
        Self {
            mesh_draw_snapshot,
            lights: environment,
        }
    }

    pub fn init() -> Self {
        Self {
            lights: LightsSnapshot {
                sun: Sun::default(),
                environment_map_intensity: 1.0,
                point_lights: vec![],
                environment_map: None,
            },
            mesh_draw_snapshot: MeshDrawSnapshot::default(),
        }
    }

    pub(crate) fn link_previous(&mut self, prev: &FixedSnapshot) {
        self.mesh_draw_snapshot
            .link_previous(&prev.mesh_draw_snapshot);
    }
}

use std::{cell::RefCell, rc::Rc, sync::Arc};

use generational_arena::{Arena, Index};
use glam::Mat4;

use super::{animation::{self, AnimationClip}, file_formats::{animationfile, materialfile, modelfile, skeletonfile::{self, Skeleton}}, registry::{AnimationClipHandle, AnimationClipId, AnimationHandle, AnimationId, MaterialHandle, MaterialId, MeshHandle, ModelId, RenderState, ResourceRegistry, SkeletonHandle, SkeletonId, TextureHandle}, render_resources::{MaterialRenderId, MeshRenderId, SubMesh, TextureRenderId}};

use super::registry::RegistryExt;

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct ModelGameId(Index);
impl Into<Index> for ModelGameId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct MaterialGameId(Index);
impl Into<Index> for MaterialGameId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct AnimationClipGameId(pub Index);
impl Into<Index> for AnimationClipGameId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct AnimationGameId(pub Index);
impl Into<Index> for AnimationGameId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct SkeletonGameId(pub Index);
impl Into<Index> for SkeletonGameId {
    fn into(self) -> Index {
        self.0
    }
}

pub struct ModelGameData {
    pub manifest: modelfile::Model,
    pub mesh: MeshHandle,
    pub submesh_instances: Vec<Vec<Mat4>>,
    pub animation_clips: Vec<AnimationClipHandle>,
    pub skeleton: SkeletonHandle,
    pub materials: Vec<MaterialHandle>,
    pub aabb: modelfile::Aabb,
}

pub struct MaterialGameData {
    pub manifest: materialfile::Material,
    pub normal_texture: Option<TextureHandle>,
    pub occlusion_texture: Option<TextureHandle>,
    pub emissive_texture: Option<TextureHandle>,
    pub base_color_texture: Option<TextureHandle>,
    pub metallic_roughness_texture: Option<TextureHandle>,
}

pub struct AnimationClipGameData {
    pub manifest: animationfile::AnimationClip,
    pub animation: AnimationHandle,
}

pub enum CreateGameResourceRequest {
    Model { id: ModelId, manifest: modelfile::Model },
    Material { id: MaterialId, manifest: materialfile::Material },
    AnimationClip { id: AnimationClipId, manifest: animationfile::AnimationClip },
    Skeleton { id: SkeletonId,  manifest: skeletonfile::Skeleton },
    Animation { id: AnimationId, anim: animation::AnimationClip },
}

pub enum CreateGameResourceResponse {
    Model {
        id: ModelId,
        game_id: ModelGameId,
        mesh: MeshRenderId,
        submeshes: Vec<SubMesh>,
        vertex_buffer_start_offset: u32,
    },
    Material {
        id: MaterialId,
        game_id: MaterialGameId,
        manifest: materialfile::Material,
        normal_texture: Option<TextureRenderId>,
        occlusion_texture: Option<TextureRenderId>,
        emissive_texture: Option<TextureRenderId>,
        base_color_texture: Option<TextureRenderId>,
        metallic_roughness_texture: Option<TextureRenderId>,
    },
    AnimationClip {
        id: AnimationClipId,
        game_id: AnimationClipGameId,
    },
    Skeleton {
        id: SkeletonId,
        game_id: SkeletonGameId,
    },
    Animation {
        id: AnimationId,
        game_id: AnimationGameId,
    },
}

pub enum StagedData {
    Model(ModelId, ModelGameData),
    Material(MaterialId, MaterialGameData),
}

pub struct GameResources {
    pub models: Arena<ModelGameData>,
    pub materials: Arena<MaterialGameData>,
    pub animation_clips: Arena<AnimationClipGameData>,
    pub animations: Arena<Arc<AnimationClip>>,
    pub skeletons: Arena<Arc<Skeleton>>,
    pub staging: Vec<StagedData>,
    pub req_rx: crossbeam::channel::Receiver<CreateGameResourceRequest>,
    pub res_tx: crossbeam::channel::Sender<CreateGameResourceResponse>,
}
impl GameResources {
    pub fn new(
        req_rx: crossbeam::channel::Receiver<CreateGameResourceRequest>,
        res_tx: crossbeam::channel::Sender<CreateGameResourceResponse>,
    ) -> Self {
        Self {
            models: Arena::new(),
            materials: Arena::new(),
            animation_clips: Arena::new(),
            animations: Arena::new(),
            skeletons: Arena::new(),
            staging: vec![],
            req_rx,
            res_tx,
        }
    }

    pub fn process_requests(
        &mut self,
        registry: &Rc<RefCell<ResourceRegistry>>
    ) {
        for req in self.req_rx.try_iter() {
            match req {
                CreateGameResourceRequest::Model { id, manifest } => {
                    let data = ModelGameData {
                        mesh: registry.request_mesh(&manifest.buffer_path),
                        skeleton: registry.request_skeleton(&manifest.skeletonfile_path),
                        animation_clips: manifest.animations.iter().map(|a| registry.request_animation_clip(a)).collect(),
                        materials: manifest.material_paths.iter().map(|a| registry.request_material(a)).collect(),
                        aabb: manifest.aabb.clone(),
                        submesh_instances: manifest.primitives.iter().map(|prim| prim.instances.iter().map(|m| Mat4::from_cols_array_2d(m)).collect()).collect(),
                        manifest,
                    };
                    self.staging.push(StagedData::Model(id, data));
                },
                CreateGameResourceRequest::Material { id, manifest } => {
                    let normal_texture = manifest.normal_texture.as_ref().map(|sampled_texture| registry.request_texture(&sampled_texture.source, false));
                    let occlusion_texture = manifest.occlusion_texture.as_ref().map(|sampled_texture| registry.request_texture(&sampled_texture.source, false));
                    let emissive_texture = manifest.emissive_texture.as_ref().map(|sampled_texture| registry.request_texture(&sampled_texture.source, true));
                    let base_color_texture = manifest.base_color_texture.as_ref().map(|sampled_texture| registry.request_texture(&sampled_texture.source, true));
                    let metallic_roughness_texture = manifest.metallic_roughness_texture.as_ref().map(|sampled_texture| registry.request_texture(&sampled_texture.source, false));
                    let data = MaterialGameData {
                        manifest,
                        normal_texture,
                        occlusion_texture,
                        emissive_texture,
                        base_color_texture,
                        metallic_roughness_texture,
                    };
                    self.staging.push(StagedData::Material(id, data));
                },
                CreateGameResourceRequest::AnimationClip { id, manifest } => {
                    let animation = registry.request_animation(&manifest.binary_path, &manifest);
                    let data = AnimationClipGameData { manifest, animation };
                    let game_id = self.animation_clips.insert(data);
                    let res = CreateGameResourceResponse::AnimationClip {
                        id,
                        game_id: AnimationClipGameId(game_id),
                    };
                    self.res_tx.send(res);
                }
                CreateGameResourceRequest::Skeleton { id, manifest } => {
                    let game_id = self.skeletons.insert(Arc::new(manifest));
                    let res = CreateGameResourceResponse::Skeleton { id, game_id: SkeletonGameId(game_id) };
                    self.res_tx.send(res);
                },
                CreateGameResourceRequest::Animation { id, anim } => {
                    let game_id = self.animations.insert(Arc::new(anim));
                    let res = CreateGameResourceResponse::Animation { id, game_id: AnimationGameId(game_id) };
                    self.res_tx.send(res);
                },
            }
        }

        let mut staging = vec![];
        'staging_loop: for data in self.staging.drain(..) {
            match data {
                StagedData::Model(id, model_game_data) => {
                    let reg = registry.borrow_mut();
                    let mesh_render_id = if let RenderState::Ready(index) = reg.get(&model_game_data.mesh).render_state {
                        MeshRenderId(index)
                    } else {
                        staging.push(StagedData::Model(id, model_game_data));
                        continue 'staging_loop;
                    };

                    let mut material_render_ids = vec![];
                    for mat_handle in &model_game_data.materials {
                        if let RenderState::Ready(index) = reg.get(mat_handle).render_state {
                            material_render_ids.push(MaterialRenderId(index));
                        } else {
                            staging.push(StagedData::Model(id, model_game_data));
                            continue 'staging_loop;
                        };
                    }

                    let vertex_buffer_start_offset = model_game_data.manifest.vertex_buffer_start_offset;
                    let submeshes = model_game_data.manifest.primitives.iter().map(|prim| {
                        SubMesh {
                            index_range: prim.index_byte_offset / 4..prim.index_byte_offset / 4 + prim.index_byte_length / 4,
                            base_vertex: prim.base_vertex,
                            material: material_render_ids[prim.material as usize],
                        }
                    }).collect();
                    let game_id = self.models.insert(model_game_data);
                    let res = CreateGameResourceResponse::Model {
                        id,
                        game_id: ModelGameId(game_id),
                        mesh: mesh_render_id,
                        submeshes,
                        vertex_buffer_start_offset,
                    };
                    if self.res_tx.send(res).is_err() {
                        todo!();
                    };
                },
                StagedData::Material(id, material_game_data) => {
                    let reg = registry.borrow_mut();

                    let normal_texture = if let Some(tex) = material_game_data.normal_texture.as_ref() {
                        if let RenderState::Ready(index) = reg.get(tex).render_state {
                            Some(TextureRenderId(index))
                        } else {
                            staging.push(StagedData::Material(id, material_game_data));
                            continue 'staging_loop;
                        }
                    } else { None };

                    let occlusion_texture = if let Some(tex) = material_game_data.occlusion_texture.as_ref() {
                        if let RenderState::Ready(index) = reg.get(tex).render_state {
                            Some(TextureRenderId(index))
                        } else {
                            staging.push(StagedData::Material(id, material_game_data));
                            continue 'staging_loop;
                        }
                    } else { None };

                    let emissive_texture = if let Some(tex) = material_game_data.emissive_texture.as_ref() {
                        if let RenderState::Ready(index) = reg.get(tex).render_state {
                            Some(TextureRenderId(index))
                        } else {
                            staging.push(StagedData::Material(id, material_game_data));
                            continue 'staging_loop;
                        }
                    } else { None };

                    let base_color_texture = if let Some(tex) = material_game_data.base_color_texture.as_ref() {
                        if let RenderState::Ready(index) = reg.get(tex).render_state {
                            Some(TextureRenderId(index))
                        } else {
                            staging.push(StagedData::Material(id, material_game_data));
                            continue 'staging_loop;
                        }
                    } else { None };

                    let metallic_roughness_texture = if let Some(tex) = material_game_data.metallic_roughness_texture.as_ref() {
                        if let RenderState::Ready(index) = reg.get(tex).render_state {
                            Some(TextureRenderId(index))
                        } else {
                            staging.push(StagedData::Material(id, material_game_data));
                            continue 'staging_loop;
                        }
                    } else { None };

                    let manifest = material_game_data.manifest.clone();
                    let game_id = self.materials.insert(material_game_data);
                    let res = CreateGameResourceResponse::Material {
                        id,
                        game_id: MaterialGameId(game_id),
                        manifest,
                        normal_texture,
                        occlusion_texture,
                        emissive_texture,
                        base_color_texture,
                        metallic_roughness_texture,
                    };
                    self.res_tx.send(res);
                },
            }
        }
        self.staging = staging;
    }
}

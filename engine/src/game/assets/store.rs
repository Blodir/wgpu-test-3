use std::{cell::RefCell, rc::Rc, sync::Arc};

use generational_arena::{Arena, Index};
use glam::Mat4;

use super::runtime_formats::animation::{self, AnimationClip};
use crate::resource_system::file_formats::{animationfile, materialfile, modelfile, skeletonfile::{self, Skeleton}};
use crate::resource_system::render_resources::{MaterialRenderId, MeshRenderId, SubMesh, TextureRenderId};
use super::{registry::{AnimationClipHandle, AnimationClipId, AnimationHandle, AnimationId, MaterialHandle, MaterialId, MeshHandle, ModelId,
        RenderState, ResourceRegistry, SkeletonHandle, SkeletonId, TextureHandle}};

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

pub enum DeformationData {
    None,
    Skinned {
        skeleton: SkeletonHandle,
        animation_clips: Vec<AnimationClipHandle>,
    }
}

pub struct ModelGameData {
    pub manifest: modelfile::Model,
    pub mesh: MeshHandle,
    pub submesh_instances: Vec<Vec<Mat4>>,
    pub deformation: DeformationData,
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

pub struct Placeholders {
    pub material: MaterialHandle,
}

pub struct GameAssetStore {
    pub placeholders: Placeholders,
    pub models: Arena<ModelGameData>,
    pub materials: Arena<MaterialGameData>,
    pub animation_clips: Arena<AnimationClipGameData>,
    pub animations: Arena<Arc<AnimationClip>>,
    pub skeletons: Arena<Arc<Skeleton>>,
    pub staging: Vec<StagedData>,
    pub req_rx: crossbeam::channel::Receiver<CreateGameResourceRequest>,
    pub res_tx: crossbeam::channel::Sender<CreateGameResourceResponse>,
}
impl GameAssetStore {
    pub fn new(
        req_rx: crossbeam::channel::Receiver<CreateGameResourceRequest>,
        res_tx: crossbeam::channel::Sender<CreateGameResourceResponse>,
        registry: &Rc<RefCell<ResourceRegistry>>,
    ) -> Self {
        let placeholders = Placeholders { material: registry.request_material(None) };
        Self {
            placeholders,
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
                        mesh: registry.request_mesh(&manifest.buffer),
                        deformation: match manifest.deformation {
                            modelfile::Deformation::None => DeformationData::None,
                            modelfile::Deformation::Skinned { ref skeleton, ref animations } => {
                                DeformationData::Skinned {
                                    skeleton: registry.request_skeleton(skeleton),
                                    animation_clips: animations.iter().map(|a| registry.request_animation_clip(a)).collect(),
                                }
                            },
                        },
                        materials: manifest.material_paths.iter().map(|a| registry.request_material(Some(a))).collect(),
                        aabb: manifest.aabb.clone(),
                        submesh_instances: manifest.submeshes.iter().map(|prim| prim.instances.iter().map(|m| Mat4::from_cols_array_2d(m)).collect()).collect(),
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
                    let mut submeshes = vec![];
                    for submesh in &model_game_data.manifest.submeshes {
                        let material = if let Some(mat_idx) = submesh.material {
                            material_render_ids[mat_idx as usize]
                        } else {
                            if let RenderState::Ready(index) = reg.get(&self.placeholders.material).render_state {
                                MaterialRenderId(index)
                            } else {
                                staging.push(StagedData::Model(id, model_game_data));
                                continue 'staging_loop;
                            }
                        };
                        submeshes.push(
                            SubMesh {
                                index_range: submesh.index_byte_offset / 4..submesh.index_byte_offset / 4 + submesh.index_byte_length / 4,
                                base_vertex: submesh.base_vertex,
                                material,
                            }
                        )
                    }
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

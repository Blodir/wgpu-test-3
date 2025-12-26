use std::{ops::Range, sync::Mutex};

use generational_arena::Arena;

use super::{animation, file_formats::{animationfile, materialfile, modelfile, skeletonfile}, registry::{AnimationClipHandle, AnimationHandle, MaterialHandle, MeshHandle, SkeletonHandle, TextureHandle}, texture::TextureLoadData};

pub struct SubMesh {
    pub instances: Vec<[[f32; 4]; 4]>,
    pub index_range: Range<u32>,
    pub base_vertex: u32,
    pub material: MaterialHandle,
}

pub struct ModelCpuData {
    pub manifest: modelfile::Model,
    pub mesh: MeshHandle,
    pub submeshes: Vec<SubMesh>,
    pub animations: Vec<AnimationClipHandle>,
    pub skeleton: SkeletonHandle,
}

pub struct MeshCpuData {
    pub index_vertex_data: Vec<u8>,
}

pub struct MaterialCpuData {
    pub manifest: materialfile::Material,
    pub normal_texture: Option<TextureHandle>,
    pub occlusion_texture: Option<TextureHandle>,
    pub emissive_texture: Option<TextureHandle>,
    pub base_color_texture: Option<TextureHandle>,
    pub metallic_roughness_texture: Option<TextureHandle>,
}

pub struct SkeletonCpuData {
    pub manifest: skeletonfile::Skeleton,
}

pub struct AnimationClipCpuData {
    pub manifest: animationfile::AnimationClip,
    pub animation: AnimationHandle,
}

pub type AnimationCpuData = animation::AnimationClip;

pub type TextureCpuData = TextureLoadData;

pub struct CpuResources {
    pub models: Mutex<Arena<ModelCpuData>>,
    pub meshes: Mutex<Arena<MeshCpuData>>,
    pub materials: Mutex<Arena<MaterialCpuData>>,
    pub skeletons: Mutex<Arena<SkeletonCpuData>>,
    pub animation_clips: Mutex<Arena<AnimationClipCpuData>>,
    pub animations: Mutex<Arena<AnimationCpuData>>,
    pub textures: Mutex<Arena<TextureCpuData>>,
}
impl CpuResources {
    pub fn new() -> Self {
        Self {
            models: Mutex::new(Arena::new()),
            meshes: Mutex::new(Arena::new()),
            materials: Mutex::new(Arena::new()),
            skeletons: Mutex::new(Arena::new()),
            animation_clips: Mutex::new(Arena::new()),
            animations: Mutex::new(Arena::new()),
            textures: Mutex::new(Arena::new()),
        }
    }
}

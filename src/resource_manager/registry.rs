use std::{collections::HashMap, marker::PhantomData, sync::{Arc, Weak}};

use generational_arena::{Arena, Index};

use super::resource_manager::ResourceManager;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ResourceKind {
    Model,
    Mesh,
    Material,
    Skeleton,
    AnimationClip,
    Animation,
    Texture,
}

pub trait ResourceTag {
    const KIND: ResourceKind;
}

/// Non-owning reference to a registry entry, the entry is not guaranteed to be present
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct HandleId<T: ResourceTag> {
    idx: generational_arena::Index,
    _marker: PhantomData<T>,
}

/// Owned/refcounted reference to a registry entry
#[derive(Debug)]
pub struct Handle<T: ResourceTag> {
    idx: generational_arena::Index,
    manager: Weak<ResourceManager>,
    _marker: PhantomData<T>,
}
impl<T: ResourceTag> Handle<T> {
    pub fn new(idx: generational_arena::Index, resource_manager_arc: &Arc<ResourceManager>) -> Self {
        Self {
            idx,
            manager: Arc::downgrade(resource_manager_arc),
            _marker: PhantomData,
        }
    }
    pub fn id(&self) -> HandleId<T> {
        HandleId {
            idx: self.idx,
            _marker: PhantomData,
        }
    }
}

impl<T: ResourceTag> Clone for Handle<T> {
    fn clone(&self) -> Self {
        if let Some(manager) = self.manager.upgrade() {
            manager.inc_ref(self.idx, T::KIND);
        }
        Self {
            idx: self.idx,
            manager: self.manager.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: ResourceTag> Drop for Handle<T> {
    fn drop(&mut self) {
        if let Some(manager) = self.manager.upgrade() {
            manager.dec_ref(self.idx, T::KIND);
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Model;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Mesh;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Material;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Skeleton;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _AnimationClip;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Animation;
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct _Texture;

impl ResourceTag for _Model { const KIND: ResourceKind = ResourceKind::Model; }
impl ResourceTag for _Mesh { const KIND: ResourceKind = ResourceKind::Mesh; }
impl ResourceTag for _Material { const KIND: ResourceKind = ResourceKind::Material; }
impl ResourceTag for _Skeleton { const KIND: ResourceKind = ResourceKind::Skeleton; }
impl ResourceTag for _AnimationClip { const KIND: ResourceKind = ResourceKind::AnimationClip; }
impl ResourceTag for _Animation { const KIND: ResourceKind = ResourceKind::Animation; }
impl ResourceTag for _Texture { const KIND: ResourceKind = ResourceKind::Texture; }

pub type ModelHandle = Handle<_Model>;
pub type MeshHandle = Handle<_Mesh>;
pub type MaterialHandle = Handle<_Material>;
pub type SkeletonHandle = Handle<_Skeleton>;
pub type AnimationClipHandle = Handle<_AnimationClip>;
pub type AnimationHandle = Handle<_Animation>;
pub type TextureHandle = Handle<_Texture>;

pub type ModelId = HandleId<_Model>;
pub type MeshId = HandleId<_Mesh>;
pub type MaterialId = HandleId<_Material>;
pub type SkeletonId = HandleId<_Skeleton>;
pub type AnimationClipId = HandleId<_AnimationClip>;
pub type AnimationId = HandleId<_Animation>;
pub type TextureId = HandleId<_Texture>;

pub enum CpuState {
    Absent, Loading, Ready(Index)
}

pub enum GpuState {
    Absent, Queued, Uploading(Index), Ready(Index)
}

pub struct Entry {
    pub kind: ResourceKind,
    pub ref_count: u32,
    pub cpu_state: CpuState,
    pub gpu_state: GpuState,
}
impl Entry {
    pub fn new(kind: ResourceKind) -> Self {
        Self {
            kind,
            ref_count: 0,
            cpu_state: CpuState::Absent,
            gpu_state: GpuState::Absent,
        }
    }
}

pub struct ResourceRegistry {
    pub entries: Arena<Entry>,
    pub by_path: HashMap<String, Index>,
}
impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            entries: Arena::new(),
            by_path: HashMap::new(),
        }
    }

    pub fn get<T: ResourceTag>(&self, handle: &Handle<T>) -> &Entry {
        self.entries.get(handle.idx).unwrap()
    }

    pub fn get_id<T: ResourceTag>(&self, id: &HandleId<T>) -> Option<&Entry> {
        self.entries.get(id.idx)
    }
}

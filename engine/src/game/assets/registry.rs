use std::{cell::RefCell, collections::HashMap, marker::PhantomData, rc::{Rc, Weak}};

use generational_arena::{Arena, Index};

use crate::main::assets::io::asset_formats::animationfile;
use crate::main::assets::store::{MaterialRenderId, MeshRenderId, ModelRenderId, TextureRenderId};
use super::{store::{AnimationClipGameId, AnimationGameId, MaterialGameId, ModelGameId, SkeletonGameId}};

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
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct HandleId<T: ResourceTag> {
    idx: generational_arena::Index,
    _marker: PhantomData<T>,
}

/// Owned/refcounted reference to a registry entry
#[derive(Debug)]
pub struct Handle<T: ResourceTag> {
    idx: generational_arena::Index,
    registry: Weak<RefCell<ResourceRegistry>>,
    _marker: PhantomData<T>,
}
impl<T: ResourceTag> Handle<T> {
    pub fn new(idx: generational_arena::Index, registry: &Rc<RefCell<ResourceRegistry>>) -> Self {
        Self {
            idx,
            registry: Rc::downgrade(registry),
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
        if let Some(registry) = self.registry.upgrade() {
            registry.inc_ref(self.idx, T::KIND);
        }
        Self {
            idx: self.idx,
            registry: self.registry.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: ResourceTag> Drop for Handle<T> {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            registry.dec_ref(self.idx, T::KIND);
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

pub enum GameState {
    Absent, Loading, Ready(Index)
}

pub enum RenderState {
    Absent, Queued, Uploading(Index), Ready(Index)
}

pub enum ResourceRequest {
    LoadModel { id: ModelId, path: String },
    LoadMesh { id: MeshId, path: String },
    LoadMaterial { id: MaterialId, path: Option<String> },
    LoadSkeleton { id: SkeletonId, path: String },
    LoadAnimationClip { id: AnimationClipId, path: String },
    LoadAnimation { id: AnimationId, path: String, header: animationfile::AnimationClip },
    LoadTexture { id: TextureId, path: String, srgb: bool },
}

pub enum ResourceResult {
    ModelResult { id: ModelId, game_id: ModelGameId, render_id: ModelRenderId },
    MeshResult { id: MeshId, render_id: MeshRenderId },
    SkeletonResult { id: SkeletonId, game_id: SkeletonGameId },
    AnimationResult { id: AnimationId, game_id: AnimationGameId },
    AnimationClipResult { id: AnimationClipId, game_id: AnimationClipGameId },
    TextureResult { id: TextureId, render_id: TextureRenderId },
    MaterialResult { id: MaterialId, game_id: MaterialGameId, render_id: MaterialRenderId },
}

pub struct Entry {
    pub kind: ResourceKind,
    pub ref_count: u32,
    pub game_state: GameState,
    pub render_state: RenderState,
}
impl Entry {
    pub fn new(kind: ResourceKind) -> Self {
        Self {
            kind,
            ref_count: 0,
            game_state: GameState::Absent,
            render_state: RenderState::Absent,
        }
    }
}

pub struct ResourceRegistry {
    pub entries: Arena<Entry>,
    pub by_path: HashMap<String, Index>,
    pub req_tx: crossbeam::channel::Sender<ResourceRequest>,
    pub res_rx: crossbeam::channel::Receiver<ResourceResult>,
}
impl ResourceRegistry {
    pub fn new(
        req_tx: crossbeam::channel::Sender<ResourceRequest>,
        res_rx: crossbeam::channel::Receiver<ResourceResult>,
    ) -> Self {
        Self {
            entries: Arena::new(),
            by_path: HashMap::new(),
            req_tx,
            res_rx,
        }
    }

    pub fn get<T: ResourceTag>(&self, handle: &Handle<T>) -> &Entry {
        self.entries.get(handle.idx).unwrap()
    }

    pub fn get_id<T: ResourceTag>(&self, id: &HandleId<T>) -> Option<&Entry> {
        self.entries.get(id.idx)
    }

    fn resource_request(
        &mut self,
        request: ResourceRequest,
    ) {
        if self.req_tx.send(request).is_err() {
            todo!()
        }
    }
}

pub trait RegistryExt {
    fn inc_ref(&self, idx: Index, kind: ResourceKind);
    fn dec_ref(&self, idx: Index, kind: ResourceKind);
    fn request_model(&self, path: &str) -> ModelHandle;
    fn request_mesh(&self, path: &str) -> MeshHandle;
    fn request_material(&self, path: Option<&str>) -> MaterialHandle;
    fn request_skeleton(&self, path: &str) -> SkeletonHandle;
    fn request_animation_clip(&self, path: &str) -> AnimationClipHandle;
    fn request_animation(&self, path: &str, header: &animationfile::AnimationClip) -> AnimationHandle;
    fn request_texture(&self, path: &str, srgb: bool) -> TextureHandle;
    fn process_responses(&self);
}

impl RegistryExt for Rc<RefCell<ResourceRegistry>> {
    fn inc_ref(&self, idx: Index, kind: ResourceKind) {
        let mut reg = self.borrow_mut();
        let Some(entry) = reg.entries.get_mut(idx) else {
            debug_assert!(false, "inc_ref on stale handle");
            return;
        };
        debug_assert_eq!(entry.kind, kind);
        entry.ref_count += 1;
    }
    fn dec_ref(&self, idx: Index, kind: ResourceKind) {
        let mut reg = self.borrow_mut();
        let entry = match reg.entries.get_mut(idx) {
            Some(e) => e,
            None => return,
        };

        debug_assert_eq!(entry.kind, kind);

        entry.ref_count = entry.ref_count.checked_sub(1)
            .expect("refcount underflow");
    }

    fn request_model(
        &self,
        path: &str,
    ) -> ModelHandle {
        let mut reg = self.borrow_mut();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return ModelHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Model,
                ref_count: 1u32,
                game_state: GameState::Loading,
                render_state: RenderState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        let handle = ModelHandle::new(idx, self);
        reg.resource_request(ResourceRequest::LoadModel { id: handle.id(), path: path.to_string() });
        handle
    }

    fn request_mesh(
        &self,
        path: &str,
    ) -> MeshHandle {
        let mut reg = self.borrow_mut();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return MeshHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Mesh,
                ref_count: 1u32,
                game_state: GameState::Loading,
                render_state: RenderState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        let handle = MeshHandle::new(idx, self);
        reg.resource_request(ResourceRequest::LoadMesh { id: handle.id(), path: path.to_string() });
        handle
    }

    fn request_material(
        &self,
        path: Option<&str>,
    ) -> MaterialHandle {
        let mut reg = self.borrow_mut();
        if let Some(path) = path {
            if let Some(&idx) = reg.by_path.get(path) {
                let entry = reg.entries.get_mut(idx).unwrap();
                entry.ref_count += 1;
                return MaterialHandle::new(idx, self);
            }

            let idx = reg.entries.insert(
                Entry {
                    kind: ResourceKind::Material,
                    ref_count: 1u32,
                    game_state: GameState::Loading,
                    render_state: RenderState::Absent,
                }
            );
            reg.by_path.insert(path.to_string(), idx);

            let handle = MaterialHandle::new(idx, self);
            reg.resource_request(ResourceRequest::LoadMaterial { id: handle.id(), path: Some(path.to_string()) });
            handle
        } else {
            let idx = reg.entries.insert(
                Entry {
                    kind: ResourceKind::Material,
                    ref_count: 1u32,
                    game_state: GameState::Loading,
                    render_state: RenderState::Absent,
                }
            );
            let handle = MaterialHandle::new(idx, self);
            reg.resource_request(ResourceRequest::LoadMaterial { id: handle.id(), path: None });
            handle
        }
    }

    fn request_skeleton(
        &self,
        path: &str,
    ) -> SkeletonHandle {
        let mut reg = self.borrow_mut();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return SkeletonHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Skeleton,
                ref_count: 1u32,
                game_state: GameState::Loading,
                render_state: RenderState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        let handle = SkeletonHandle::new(idx, self);
        reg.resource_request(ResourceRequest::LoadSkeleton { id: handle.id(), path: path.to_string() });
        handle
    }

    fn request_animation_clip(
        &self,
        path: &str,
    ) -> AnimationClipHandle {
        let mut reg = self.borrow_mut();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return AnimationClipHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::AnimationClip,
                ref_count: 1u32,
                game_state: GameState::Loading,
                render_state: RenderState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        let handle = AnimationClipHandle::new(idx, self);
        reg.resource_request(ResourceRequest::LoadAnimationClip { id: handle.id(), path: path.to_string() });
        handle
    }

    fn request_animation(
        &self,
        path: &str,
        header: &animationfile::AnimationClip,
    ) -> AnimationHandle {
        let mut reg = self.borrow_mut();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return AnimationHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Animation,
                ref_count: 1u32,
                game_state: GameState::Loading,
                render_state: RenderState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        let handle = AnimationHandle::new(idx, self);
        reg.resource_request(ResourceRequest::LoadAnimation { id: handle.id(), path: path.to_string(), header: header.clone() });
        handle
    }

    fn request_texture(
        &self,
        path: &str,
        srgb: bool,
    ) -> TextureHandle {
        let mut reg = self.borrow_mut();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return TextureHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Texture,
                ref_count: 1u32,
                game_state: GameState::Loading,
                render_state: RenderState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        let handle = TextureHandle::new(idx, self);
        reg.resource_request(ResourceRequest::LoadTexture { id: handle.id(), path: path.to_string(), srgb });
        handle
    }

    fn process_responses(&self) {
        let mut reg = self.borrow_mut();
        while let Ok(msg) = reg.res_rx.try_recv() {
            match msg {
                ResourceResult::ModelResult { id, game_id, render_id } => {
                    let entry = reg.entries.get_mut(id.idx).unwrap();
                    entry.game_state = GameState::Ready(game_id.into());
                    entry.render_state = RenderState::Ready(render_id.into());
                }
                ResourceResult::MeshResult { id, render_id } => {
                    let entry = reg.entries.get_mut(id.idx).unwrap();
                    entry.render_state = RenderState::Ready(render_id.into());
                },
                ResourceResult::SkeletonResult { id, game_id } => {
                    let entry = reg.entries.get_mut(id.idx).unwrap();
                    entry.game_state = GameState::Ready(game_id.into());
                },
                ResourceResult::AnimationResult { id, game_id } => {
                    let entry = reg.entries.get_mut(id.idx).unwrap();
                    entry.game_state = GameState::Ready(game_id.into());
                },
                ResourceResult::AnimationClipResult { id, game_id } => {
                    let entry = reg.entries.get_mut(id.idx).unwrap();
                    entry.game_state = GameState::Ready(game_id.into());
                },
                ResourceResult::TextureResult { id, render_id } => {
                    let entry = reg.entries.get_mut(id.idx).unwrap();
                    entry.render_state = RenderState::Ready(render_id.into());
                },
                ResourceResult::MaterialResult { id, game_id, render_id } => {
                    let entry = reg.entries.get_mut(id.idx).unwrap();
                    entry.game_state = GameState::Ready(game_id.into());
                    entry.render_state = RenderState::Ready(render_id.into());
                },
            }
        }
    }
}

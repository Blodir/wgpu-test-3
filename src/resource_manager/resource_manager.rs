use std::{collections::{HashMap, VecDeque}, ops::Range, sync::{Arc, Mutex}};

use generational_arena::{Arena, Index};

use crate::renderer::{pipelines::model::vertex::Vertex, render_resources::modelfile, wgpu_context::{self, WgpuContext}};

#[derive(Debug, PartialEq, Eq)]
enum ResourceKind {
    Model,
    Material,
    Skeleton,
    AnimationClip,
    Texture,
}

struct ModelHandle(Index);
struct MeshHandle(Index);
struct MaterialHandle(Index);

pub struct SkeletonHandle {
    idx: generational_arena::Index,
    manager: std::sync::Weak<ResourceManager>,
}
impl Drop for SkeletonHandle {
    fn drop(&mut self) {
        if let Some(manager) = self.manager.upgrade() {
            manager.dec_ref(self.idx, ResourceKind::Skeleton);
        }
    }
}
impl Clone for SkeletonHandle {
    fn clone(&self) -> Self {
        if let Some(manager) = self.manager.upgrade() {
            manager.inc_ref(self.idx, ResourceKind::Skeleton);
        }

        Self {
            idx: self.idx,
            manager: self.manager.clone(),
        }
    }
}

struct AnimationClipHandle(Index);
struct TextureHandle(Index);

enum CpuState {
    Absent, Loading, Ready(Index)
}

enum GpuState {
    Absent, Queued, Uploading(Index), Ready(Index)
}

struct Entry {
    kind: ResourceKind,
    ref_count: u32,
    cpu_state: CpuState,
    gpu_state: GpuState,
}
impl Entry {
    pub fn new(kind: ResourceKind) -> Self {
        todo!()
    }
}

struct ResourceRegistry {
    pub entries: Arena<Entry>,
    pub by_path: HashMap<String, Index>,
}

struct Mesh {
    pub index_vertex_data: Vec<u8>,
}

struct SubMesh {
    pub instances: Vec<[[f32; 4]; 4]>,
    pub index_range: Range<u32>,
    pub base_vertex: u32,
    pub material: MaterialHandle,
}

struct ModelCpuData {
    manifest: modelfile::Model,
    mesh: MeshHandle,
    submeshes: Vec<SubMesh>,
    animations: Vec<AnimationClipHandle>,
    skeleton: SkeletonHandle,
}

struct ModelGpuData {
    vertex_index_buffer: wgpu::Buffer,
}

struct CpuResources {
    pub models: Mutex<Arena<ModelCpuData>>,
}

struct GpuResources {
    pub models: Mutex<Arena<ModelGpuData>>,
}

enum IoRequest {
    LoadModel { id: Index, path: String },
    // etc.
}

enum IoResponse {
    ModelLoaded { id: Index, model: modelfile::Model },
    Error { path: String, message: String },
    // etc.
}

fn load_model_from_disk(path: &str) -> Result<modelfile::Model, Box<dyn std::error::Error>> {
    let json_file = std::fs::File::open(path)?;
    let json_reader = std::io::BufReader::new(json_file);
    let model: modelfile::Model = serde_json::from_reader(json_reader)?;
    Ok(model)
}

fn io_worker_loop(
    rx: crossbeam::channel::Receiver<IoRequest>,
    tx: crossbeam::channel::Sender<IoResponse>,
) {
    while let Ok(req) = rx.recv() {
        let result = match req {
            IoRequest::LoadModel { id, path } => load_model_from_disk(&path)
                .map_or_else(|e| IoResponse::Error { path: path.clone(), message: e.to_string() }, |model| IoResponse::ModelLoaded { id, model }),
        };

        // ignore send errors on shutdown
        let _ = tx.send(result);
    }
}

struct IoManager {
    pub req_tx: crossbeam::channel::Sender<IoRequest>,
    pub res_rx: crossbeam::channel::Receiver<IoResponse>,
    workers: Vec<std::thread::JoinHandle<()>>,
}
impl IoManager {
    pub fn new() -> Self {
        let (req_tx, req_rx) = crossbeam::channel::unbounded();
        let (res_tx, res_rx) = crossbeam::channel::unbounded();

        let workers = (0..2)
            .map(|_| {
                let rx = req_rx.clone();
                let tx = res_tx.clone();
                std::thread::spawn(move || {
                    io_worker_loop(rx, tx);
                })
            })
            .collect();

        Self {
            req_tx, res_rx, workers
        }
    }

}

pub struct ResourceManager {
    registry: Mutex<ResourceRegistry>,
    gpu: GpuResources,
    cpu: CpuResources,
    io: IoManager,
    upload_queue: Mutex<VecDeque<Index>>,
}
impl ResourceManager {
    pub fn new() -> Self {
        // start io thread
        todo!()
    }

    fn inc_ref(&self, idx: Index, kind: ResourceKind) {
        let mut reg = self.registry.lock().unwrap();
        let Some(entry) = reg.entries.get_mut(idx) else {
            debug_assert!(false, "inc_ref on stale handle");
            return;
        };
        debug_assert_eq!(entry.kind, kind);
        entry.ref_count += 1;
    }

    fn dec_ref(&self, idx: Index, kind: ResourceKind) {
        let mut reg = self.registry.lock().unwrap();

        let entry = match reg.entries.get_mut(idx) {
            Some(e) => e,
            None => return,
        };

        debug_assert_eq!(entry.kind, kind);

        entry.ref_count = entry.ref_count.checked_sub(1)
            .expect("refcount underflow");
    }

    fn process_io_responses(
        self: &std::sync::Arc<Self>,
    ) {
        let mut reg = self.registry.lock().unwrap();
        while !self.io.res_rx.is_empty() {
            let res = match self.io.res_rx.recv() {
                Ok(r) => r,
                Err(err) => {
                    println!("Recv Error: {}", err);
                    break
                },
            };
            match res {
                IoResponse::ModelLoaded { id, model } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data = ModelCpuData {
                        mesh: ResourceManager::request_mesh(self, &model.buffer_path),
                        submeshes: model.primitives.iter().map(|prim| {
                            let index_start = prim.index_byte_offset / 4;
                            let index_count = prim.index_byte_length / 4;
                            SubMesh {
                                instances: prim.instances.clone(),
                                index_range: index_start..index_start + index_count,
                                base_vertex: prim.base_vertex,
                                material: ResourceManager::request_material(self, &model.material_paths[prim.material as usize]),
                            }
                        }).collect(),
                        animations: model.animations.iter().map(|anim| ResourceManager::request_animation_clip(self, anim)).collect(),
                        skeleton: ResourceManager::request_skeleton(self, &model.skeletonfile_path),
                        manifest: model,
                    };
                    let cpu_idx = self.cpu.models.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::Error { path, message } => {
                    println!("IO Error: path: {}, message: {}", path, message);
                },
            }
        }
    }

    fn process_upload_queue(
        self: &std::sync::Arc<Self>,
        wgpu_context: &WgpuContext,
    ) {
        let mut reg = self.registry.lock().unwrap();
        let mut upload_queue = self.upload_queue.lock().unwrap();
        while let Some(id) = upload_queue.pop_front() {
            let entry = reg.entries.get_mut(id).unwrap();

            if entry.ref_count == 0 {
                continue; // cancelled
            }

            match entry.kind {
                ResourceKind::Model => {
                    if let CpuState::Ready(c) = entry.cpu_state {
                        let m = self.cpu.models.lock().unwrap().get(c).unwrap();
                        // probs remove this, model shouldnt have gpu data
                        todo!();
                    }
                },
                ResourceKind::Material => todo!(),
                ResourceKind::Skeleton => todo!(),
                ResourceKind::AnimationClip => todo!(),
                ResourceKind::Texture => todo!(),
            }
        }
        todo!();
    }

    fn run_gc(
        self: &std::sync::Arc<Self>,
    ) {
        // for each entry with ref count 0
        // should there be a vec that keeps track of refcount 0s?
        // TODO eviction
        // during eviction remember to clean CpuResources arena etc.
        todo!();
    }

    fn make_io_request(&self, req: IoRequest) {
        if self.io.req_tx.send(req).is_err() {
            todo!()
        }
    }

    pub fn request_model(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> ModelHandle {
        // make io request
        todo!()
    }

    pub fn request_mesh(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> MeshHandle {
        todo!()
    }

    pub fn request_material(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> MaterialHandle {
        todo!()
    }

    pub fn request_skeleton(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> SkeletonHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return SkeletonHandle { idx, manager: Arc::downgrade(self) };
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Skeleton,
                ref_count: 0u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadModel { id: idx, path: path.to_string() });

        SkeletonHandle { idx, manager: Arc::downgrade(self) }
    }

    pub fn request_animation_clip(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> AnimationClipHandle {
        todo!()
    }
    pub fn request_texture(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> TextureHandle {
        todo!()
    }
}

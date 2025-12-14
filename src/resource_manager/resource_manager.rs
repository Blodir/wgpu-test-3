use std::sync::mpsc::{Receiver, Sender};

type ResourceId = usize;

enum ResourceKind {
    Model,
    Material,
    Skeleton,
    AnimationClip,
    Texture,
}

struct ModelHandle(ResourceId);
struct MaterialHandle(ResourceId);
struct SkeletonHandle(ResourceId);
struct AnimationClipHandle(ResourceId);
struct TextureHandle(ResourceId);

enum CpuState {
    Absent, Loading, Ready(u32)
}

enum GpuState {
    Absent, Loading, Ready(u32)
}

struct Entry {
    kind: ResourceKind,
    ref_count: u32,
    cpu_state: CpuState,
    gpu_state: GpuState,
}

struct ResourceRegistry {}
struct GpuResources {}
struct CpuResources {}

enum IoRequest {
    LoadModel(ModelHandle),
    // etc.
}
enum IoResponse {
    ModelLoaded(ModelHandle),
    // etc.
}
struct IoManager {
    tx: Sender<IoRequest>,
    rx: Receiver<IoResponse>,
}

pub struct ResourceManager {
    registry: ResourceRegistry,
    gpu: GpuResources,
    cpu: CpuResources,
    io: IoManager,
}
impl ResourceManager {
    pub fn new() -> Self {
        // start io thread
        todo!()
    }
    fn process_io_responses() {
        // for each response in io.tx
        // set cpu resource
        // update registry entry
        todo!()
    }
    pub fn request_model() -> ModelHandle {
        // make io request
        todo!()
    }
    pub fn request_material() -> MaterialHandle {
        todo!()
    }
    pub fn request_skeleton() -> SkeletonHandle {
        todo!()
    }
    pub fn request_animation_clip() -> AnimationClipHandle {
        todo!()
    }
    pub fn request_texture() -> TextureHandle {
        todo!()
    }
}

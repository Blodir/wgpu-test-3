use std::collections::HashMap;

type NodeId = u64;
type RenderDataId = u64;
type ResourceId = String;

// actual vertex buffer layout that gets uploaded to the gpu
struct Vertex {}
enum VertexIndices {
    //U8(Vec<u8>), wgpu does not allow u8s while gltf does (i think?)
    U16(Vec<u16>),
    U32(Vec<u32>),
}

struct VertexData {
    vertices: Vec<Vertex>,
    indices: VertexIndices,
}

struct MeshResource {
    vertex_data: ResourceId,
    material_data: ResourceId,
}

enum ResourceType {
}

struct ResourcePool {
    data: HashMap<ResourceId, ResourceType>
}

// Static data that should not be cloned each sim step
struct RenderDataRepository {
    data: HashMap<RenderDataId, RenderData>
}

struct RenderData {
}

struct Transform {}

// SCENE GRAPH NODES
pub struct Node {
    parent: NodeId,
    children: Vec<NodeId>,
    transform: Transform,
    render_data: RenderDataId,
}


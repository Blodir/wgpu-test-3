use std::{collections::{HashMap, VecDeque}, fs::File, io::Read as _, ops::Range, sync::{Arc, Mutex}};

use ddsfile::{Caps2, Dds};
use generational_arena::{Arena, Index};
use glam::{Quat, Vec3};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::renderer::{pipelines::model::vertex::Vertex, render_resources::{animation, animationfile, dds, materialfile, modelfile, skeletonfile}, wgpu_context::{self, WgpuContext}};

#[derive(Debug, PartialEq, Eq)]
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

pub struct Handle<T: ResourceTag> {
    idx: generational_arena::Index,
    manager: std::sync::Weak<ResourceManager>,
    _marker: std::marker::PhantomData<T>,
}
impl<T: ResourceTag> Handle<T> {
    pub fn new(idx: generational_arena::Index, resource_manager_arc: &std::sync::Arc<ResourceManager>) -> Self {
        Self {
            idx,
            manager: Arc::downgrade(resource_manager_arc),
            _marker: std::marker::PhantomData,
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
            _marker: std::marker::PhantomData,
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

pub struct _Model;
pub struct _Mesh;
pub struct _Material;
pub struct _Skeleton;
pub struct _AnimationClip;
pub struct _Animation;
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
        Self {
            kind,
            ref_count: 0,
            cpu_state: CpuState::Absent,
            gpu_state: GpuState::Absent,
        }
    }
}

struct ResourceRegistry {
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

struct MeshCpuData {
    pub index_vertex_data: Vec<u8>,
}

struct MaterialCpuData {
    pub manifest: materialfile::Material,
    pub normal_texture: Option<TextureHandle>,
    pub occlusion_texture: Option<TextureHandle>,
    pub emissive_texture: Option<TextureHandle>,
    pub base_color_texture: Option<TextureHandle>,
    pub metallic_roughness_texture: Option<TextureHandle>,
}

struct SkeletonCpuData {
    manifest: skeletonfile::Skeleton,
}

struct AnimationClipCpuData {
    manifest: animationfile::AnimationClip,
    animation: AnimationHandle,
}

type AnimationCpuData = animation::AnimationClip;

type TextureCpuData = TextureLoadData;

struct CpuResources {
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

struct MeshGpuData {
    pub buffer: wgpu::Buffer,
}

struct TextureGpuData {
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
}

struct GpuResources {
    pub meshes: Mutex<Arena<MeshGpuData>>,
    pub textures: Mutex<Arena<TextureGpuData>>,
}
impl GpuResources {
    pub fn new() -> Self {
        Self {
            meshes: Mutex::new(Arena::new()),
            textures: Mutex::new(Arena::new()),
        }
    }
}

struct TextureLoadData {
    data: Vec<u8>,
    base_width: u32,
    base_height: u32,
    mips: u32,
    layers: u32,
    format: wgpu::TextureFormat,
}

enum IoRequest {
    LoadModel { id: Index, path: String },
    LoadMesh { id: Index, path: String },
    LoadMaterial { id: Index, path: String },
    LoadSkeleton { id: Index, path: String },
    LoadAnimationClip { id: Index, path: String },
    LoadAnimation { id: Index, path: String, header: animationfile::AnimationClip },
    LoadTexture { id: Index, path: String, srgb: bool },
}

enum IoResponse {
    ModelLoaded { id: Index, model: modelfile::Model },
    MeshLoaded { id: Index, data: Vec<u8> },
    MaterialLoaded { id: Index, material: materialfile::Material },
    SkeletonLoaded { id: Index, skeleton: skeletonfile::Skeleton },
    AnimationClipLoaded { id: Index, clip: animationfile::AnimationClip },
    AnimationLoaded { id: Index, parsed_clip: animation::AnimationClip },
    TextureLoaded { id: Index, data: TextureLoadData },
    Error { path: String, message: String },
}

fn load_json<T>(path: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: serde::de::DeserializeOwned,
{
    let json_file = std::fs::File::open(path)?;
    let json_reader = std::io::BufReader::new(json_file);
    let model: T = serde_json::from_reader(json_reader)?;
    Ok(model)
}

fn load_bin(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    Ok(bytes)
}

fn load_dds(bytes: &mut [u8]) -> TextureLoadData {
    let dds = Dds::read(&mut &bytes[..]).unwrap();

    let format = dds::dds_format_to_wgpu(
        dds.get_dxgi_format()
            .expect("Dds doesn't have a DXGI format."),
    );
    let is_cubemap = dds
        .header
        .caps2
        .contains(Caps2::CUBEMAP);
    let base_width = dds.get_width();
    let base_height = dds.get_height();
    let mips = dds.get_num_mipmap_levels();
    let layers = if is_cubemap { 6 } else { dds.get_num_array_layers() };

    TextureLoadData {
        data: dds.data,
        base_width,
        base_height,
        mips,
        layers,
        format,
    }
}

fn load_png(bytes: &mut [u8], srgb: bool) -> TextureLoadData {
    let img: image::DynamicImage = image::load_from_memory(&bytes).unwrap();
    let dimensions = image::GenericImageView::dimensions(&img);
    let (remapped, format): (Vec<u8>, wgpu::TextureFormat) = match (&img, srgb) {
        (image::DynamicImage::ImageRgb32F(_), false) => (
            bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba32Float,
        ),
        (image::DynamicImage::ImageRgba32F(_), false) => (
            bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba32Float,
        ),
        (_, true) => (
            bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ),
        (_, false) => (
            bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
    };
    let base_width = dimensions.0;
    let base_height = dimensions.1;
    let mips = 1;
    let layers = 1;

    TextureLoadData {
        data: remapped,
        base_width,
        base_height,
        mips,
        layers,
        format,
    }
}

fn load_texture(path: &str, srgb: bool) -> Result<TextureLoadData, Box<dyn std::error::Error>> {
    let mut file = File::open(path).unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();

    if bytes.starts_with(b"DDS ") {
        Ok(load_dds(&mut bytes))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Ok(load_png(&mut bytes, srgb))
    } else {
        Err("invalid texture format".into())
    }
}

pub fn load_animation(
    path: &str,
    header: animationfile::AnimationClip,
) -> Result<animation::AnimationClip, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;

    let read_f32_ref = |r: &animationfile::BinRef| -> Box<[f32]> {
        let count = r.count as usize;
        let mut output = vec![0f32; count];
        let stride = 4;

        for i in 0..count {
            let idx = r.offset as usize + i * stride;
            output[i] = bytemuck::cast::<[u8; 4], f32>(bytes[idx..idx + 4].try_into().unwrap());
        }

        output.into_boxed_slice()
    };
    let read_vec3_ref = |r: &animationfile::BinRef| -> Box<[Vec3]> {
        let count = r.count as usize;
        let mut output = vec![];
        let stride = 12;

        for i in 0..count {
            let idx = r.offset as usize + i * stride;
            output.push(
                Vec3::from_array([
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx..idx + 4].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 4..idx + 8].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 8..idx + 12].try_into().unwrap()),
                ])
            );
        }

        output.into_boxed_slice()
    };
    let read_quat_ref = |r: &animationfile::BinRef| -> Box<[Quat]> {
        let count = r.count as usize;
        let mut output = vec![];
        let stride = 16;

        for i in 0..count {
            let idx = r.offset as usize + i * stride;
            output.push(
                Quat::from_array([
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx..idx + 4].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 4..idx + 8].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 8..idx + 12].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 12..idx + 16].try_into().unwrap()),
                ])
            );
        }

        output.into_boxed_slice()
    };

    let duration = header.duration;
    let primitive_groups = header.primitive_groups;
    let tracks: Vec<animation::Track> = header.tracks.iter().map(|track| {
        let target = track.target;
        let shared_times = track.shared_times.as_ref().map(read_f32_ref);
        let translation = track.translation.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_vec3_ref(&s.values);
            animation::Channel::<Vec3> {
                interpolation, times, values
            }
        });
        let rotation = track.rotation.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_quat_ref(&s.values);
            animation::Channel::<Quat> {
                interpolation, times, values
            }
        });
        let scale = track.scale.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_vec3_ref(&s.values);
            animation::Channel::<Vec3> {
                interpolation, times, values
            }
        });
        animation::Track {
            target, shared_times, translation, rotation, scale
        }
    }).collect();

    Ok(animation::AnimationClip {
        duration,
        tracks,
        primitive_groups,
    })
}

fn io_worker_loop(
    rx: crossbeam::channel::Receiver<IoRequest>,
    tx: crossbeam::channel::Sender<IoResponse>,
) {
    while let Ok(req) = rx.recv() {
        let result = match req {
            IoRequest::LoadModel { id, path } => load_json::<modelfile::Model>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |model| IoResponse::ModelLoaded { id, model },
                ),
            IoRequest::LoadMesh { id, path } => load_bin(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::MeshLoaded { id, data },
                ),
            IoRequest::LoadMaterial { id, path } => load_json::<materialfile::Material>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |material| IoResponse::MaterialLoaded { id, material },
                ),
            IoRequest::LoadSkeleton { id, path } => load_json::<skeletonfile::Skeleton>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |skeleton| IoResponse::SkeletonLoaded { id, skeleton },
                ),
            IoRequest::LoadAnimationClip { id, path } => load_json::<animationfile::AnimationClip>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |clip| IoResponse::AnimationClipLoaded { id, clip },
                ),
            IoRequest::LoadAnimation { id, path, header } => load_animation(&path, header)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::AnimationLoaded { id, parsed_clip: data },
                ),
            IoRequest::LoadTexture { id, path, srgb } => load_texture(&path, srgb)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::TextureLoaded { id, data },
                ),
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
        Self {
            registry: Mutex::new(ResourceRegistry::new()),
            gpu: GpuResources::new(),
            cpu: CpuResources::new(),
            io: IoManager::new(),
            upload_queue: Mutex::new(VecDeque::new()),
        }
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

    pub fn process_io_responses(
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
                IoResponse::MeshLoaded { id, data } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data = MeshCpuData { index_vertex_data: data };
                    let cpu_idx = self.cpu.meshes.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::MaterialLoaded { id, material } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let normal_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let occlusion_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let emissive_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let base_color_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let metallic_roughness_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let cpu_data = MaterialCpuData { manifest: material, normal_texture, occlusion_texture, emissive_texture, base_color_texture, metallic_roughness_texture };
                    let cpu_idx = self.cpu.materials.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::SkeletonLoaded { id, skeleton } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data = SkeletonCpuData { manifest: skeleton };
                    let cpu_idx = self.cpu.skeletons.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::AnimationClipLoaded { id, clip } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let animation = self.request_animation(&clip.binary_path, &clip);
                    let cpu_data = AnimationClipCpuData { manifest: clip, animation };
                    let cpu_idx = self.cpu.animation_clips.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::AnimationLoaded { id, parsed_clip } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data: AnimationCpuData = parsed_clip;
                    let cpu_idx = self.cpu.animations.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::TextureLoaded { id, data } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data: TextureCpuData = data;
                    let cpu_idx = self.cpu.textures.lock().unwrap().insert(cpu_data);
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

    pub fn process_upload_queue(
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
                ResourceKind::Mesh => {
                    if let CpuState::Ready(cpu_idx) = entry.cpu_state {
                        let mut meshes_cpu = self.cpu.meshes.lock().unwrap();
                        let buffer = {
                            let mesh_cpu = meshes_cpu.get(cpu_idx).unwrap();
                            wgpu_context.device.create_buffer_init(&BufferInitDescriptor {
                                label: Some("Index/vertex buffer"),
                                contents: bytemuck::cast_slice(&mesh_cpu.index_vertex_data),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
                            })
                        };
                        let mesh_gpu = MeshGpuData { buffer };
                        let mut meshes_gpu = self.gpu.meshes.lock().unwrap();
                        let gpu_idx = meshes_gpu.insert(mesh_gpu);
                        entry.gpu_state = GpuState::Ready(gpu_idx);
                        entry.cpu_state = CpuState::Absent;
                        meshes_cpu.remove(cpu_idx);
                    } else {
                        println!("Warning: no cpu data for mesh in upload queue");
                    }
                },
                ResourceKind::Texture => {
                    if let CpuState::Ready(cpu_idx) = entry.cpu_state {
                        let mut textures_cpu = self.cpu.textures.lock().unwrap();
                        let texture_cpu = textures_cpu.get(cpu_idx).unwrap();
                        // TODO move this function away from dds module, also it should probs just take the cpudata directly
                        let texture = dds::upload_texture(
                            &texture_cpu.data,
                            texture_cpu.base_width,
                            texture_cpu.base_height,
                            texture_cpu.mips,
                            texture_cpu.layers,
                            texture_cpu.format,
                            &wgpu_context.device,
                            &wgpu_context.queue
                        );
                        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                        let texture_gpu = TextureGpuData { texture, texture_view };
                        let mut textures_gpu = self.gpu.textures.lock().unwrap();
                        let gpu_idx = textures_gpu.insert(texture_gpu);
                        entry.gpu_state = GpuState::Ready(gpu_idx);
                        entry.cpu_state = CpuState::Absent;
                        textures_cpu.remove(cpu_idx);
                    } else {
                        println!("Warning: no cpu data for texture in upload queue");
                    }
                }
                _ => println!("Warning: tried to upload an unsupported resource!"),
            }
        }
        todo!();
    }

    pub fn run_gc(
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
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return ModelHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Model,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadModel { id: idx, path: path.to_string() });

        ModelHandle::new(idx, self)
    }

    pub fn request_mesh(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> MeshHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return MeshHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Mesh,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadMesh { id: idx, path: path.to_string() });

        MeshHandle::new(idx, self)
    }

    pub fn request_material(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> MaterialHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return MaterialHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Material,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadMaterial { id: idx, path: path.to_string() });

        MaterialHandle::new(idx, self)
    }

    pub fn request_skeleton(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> SkeletonHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return SkeletonHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Skeleton,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadModel { id: idx, path: path.to_string() });

        SkeletonHandle::new(idx, self)
    }

    pub fn request_animation_clip(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> AnimationClipHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return AnimationClipHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::AnimationClip,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadAnimationClip { id: idx, path: path.to_string() });

        AnimationClipHandle::new(idx, self)
    }

    fn request_animation(
        self: &std::sync::Arc<Self>,
        path: &str,
        header: &animationfile::AnimationClip,
    ) -> AnimationHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return AnimationHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Animation,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        // TODO cloning header sounds pretty bad
        self.make_io_request(IoRequest::LoadAnimation { id: idx, path: path.to_string(), header: header.clone() });

        AnimationHandle::new(idx, self)
    }

    pub fn request_texture(
        self: &std::sync::Arc<Self>,
        path: &str,
        srgb: bool,
    ) -> TextureHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return TextureHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Texture,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadTexture { id: idx, path: path.to_string(), srgb });

        TextureHandle::new(idx, self)
    }
}

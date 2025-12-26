use std::{collections::VecDeque, sync::Mutex};

use generational_arena::Index;
use wgpu::util::{BufferInitDescriptor, DeviceExt as _};

use crate::renderer::{wgpu_context::WgpuContext, Renderer};

use super::{cpu_resources::{AnimationClipCpuData, AnimationCpuData, CpuResources, MaterialCpuData, MeshCpuData, ModelCpuData, SkeletonCpuData, SubMesh, TextureCpuData}, file_formats::{animationfile, dds}, gpu_resources::{GpuResources, MeshGpuData, TextureGpuData}, io_manager::{IoManager, IoRequest, IoResponse}, registry::{AnimationClipHandle, AnimationHandle, CpuState, Entry, GpuState, MaterialHandle, MeshHandle, ModelHandle, ResourceKind, ResourceRegistry, SkeletonHandle, TextureHandle}, texture::upload_texture};

pub struct ResourceManager {
    pub registry: Mutex<ResourceRegistry>,
    pub gpu: GpuResources,
    pub cpu: CpuResources,
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

    pub fn inc_ref(&self, idx: Index, kind: ResourceKind) {
        let mut reg = self.registry.lock().unwrap();
        let Some(entry) = reg.entries.get_mut(idx) else {
            debug_assert!(false, "inc_ref on stale handle");
            return;
        };
        debug_assert_eq!(entry.kind, kind);
        entry.ref_count += 1;
    }

    pub fn dec_ref(&self, idx: Index, kind: ResourceKind) {
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
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_idx = self.cpu.models.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::MeshLoaded { id, data } => {
                    let cpu_data = MeshCpuData { index_vertex_data: data };
                    let cpu_idx = self.cpu.meshes.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::MaterialLoaded { id, material } => {
                    let normal_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let occlusion_texture = material.occlusion_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let emissive_texture = material.emissive_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let base_color_texture = material.base_color_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let metallic_roughness_texture = material.metallic_roughness_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let cpu_data = MaterialCpuData { manifest: material, normal_texture, occlusion_texture, emissive_texture, base_color_texture, metallic_roughness_texture };
                    let cpu_idx = self.cpu.materials.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::SkeletonLoaded { id, skeleton } => {
                    let cpu_data = SkeletonCpuData { manifest: skeleton };
                    let cpu_idx = self.cpu.skeletons.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::AnimationClipLoaded { id, clip } => {
                    let animation = self.request_animation(&clip.binary_path, &clip);
                    let cpu_data = AnimationClipCpuData { manifest: clip, animation };
                    let cpu_idx = self.cpu.animation_clips.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::AnimationLoaded { id, parsed_clip } => {
                    let cpu_data: AnimationCpuData = parsed_clip;
                    let cpu_idx = self.cpu.animations.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::TextureLoaded { id, data } => {
                    let cpu_data: TextureCpuData = data;
                    let cpu_idx = self.cpu.textures.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
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
        renderer: &mut Renderer,
        wgpu_context: &WgpuContext,
    ) {
        let mut upload_queue = self.upload_queue.lock().unwrap();
        let mut meshes_cpu = self.cpu.meshes.lock().unwrap();
        let mut textures_cpu = self.cpu.textures.lock().unwrap();
        let mut materials_gpu = self.gpu.materials.lock().unwrap();
        let mut queue_next_frame: Vec<Index> = vec![];
        'upload_queue: while let Some(id) = upload_queue.pop_front() {
            let (entry_kind, entry_cpu_idx) = {
                let mut reg = self.registry.lock().unwrap();
                let entry = reg.entries.get(id).unwrap();

                if entry.ref_count == 0 {
                    continue 'upload_queue; // cancelled
                }

                let entry_kind = entry.kind;
                let entry_cpu_idx = if let CpuState::Ready(cpu_idx) = entry.cpu_state {
                    cpu_idx
                } else {
                    println!("Warning: no cpu data for entry in upload queue");
                    continue 'upload_queue;
                };
                (entry_kind, entry_cpu_idx)
            };

            match entry_kind {
                ResourceKind::Mesh => {
                    let buffer = {
                        let mesh_cpu = meshes_cpu.get(entry_cpu_idx).unwrap();
                        wgpu_context.device.create_buffer_init(&BufferInitDescriptor {
                            label: Some("Index/vertex buffer"),
                            contents: bytemuck::cast_slice(&mesh_cpu.index_vertex_data),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
                        })
                    };
                    let mesh_gpu = MeshGpuData { buffer };
                    let mut meshes_gpu = self.gpu.meshes.lock().unwrap();
                    let gpu_idx = meshes_gpu.insert(mesh_gpu);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.gpu_state = GpuState::Ready(gpu_idx);
                    entry.cpu_state = CpuState::Absent;
                    meshes_cpu.remove(entry_cpu_idx);
                },
                ResourceKind::Texture => {
                    let texture_cpu = textures_cpu.get(entry_cpu_idx).unwrap();
                    // TODO move this function away from dds module, also it should probs just take the cpudata directly
                    let texture = upload_texture(
                        &texture_cpu,
                        &wgpu_context,
                    );
                    // TODO probably should have a is_cubemap flag?
                    let texture_view = if texture_cpu.layers == 6 {
                        texture.create_view(&wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::Cube),
                            ..Default::default()
                        })
                    } else {
                        texture.create_view(&wgpu::TextureViewDescriptor::default())
                    };
                    let texture_gpu = TextureGpuData { texture, texture_view };
                    let mut textures_gpu = self.gpu.textures.lock().unwrap();
                    let gpu_idx = textures_gpu.insert(texture_gpu);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.gpu_state = GpuState::Ready(gpu_idx);
                    entry.cpu_state = CpuState::Absent;
                    textures_cpu.remove(entry_cpu_idx);
                },
                ResourceKind::Material => {
                    let material_binding = match renderer.upload_material(entry_cpu_idx, self, wgpu_context) {
                        Ok(mat) => mat,
                        Err(_) => {
                            queue_next_frame.push(id);
                            continue 'upload_queue;
                        }
                    };
                    let gpu_idx = materials_gpu.insert(material_binding);
                    {
                        let mut reg = self.registry.lock().unwrap();
                        let entry = reg.entries.get_mut(id).unwrap();
                        entry.gpu_state = GpuState::Ready(gpu_idx);
                        entry.cpu_state = CpuState::Absent;
                    }
                    let mut materials_cpu = self.cpu.materials.lock().unwrap();
                    materials_cpu.remove(entry_cpu_idx);
                },
                _ => println!("Warning: tried to upload an unsupported resource!"),
            }
        }
        upload_queue.extend(queue_next_frame);
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

        self.make_io_request(IoRequest::LoadSkeleton { id: idx, path: path.to_string() });

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

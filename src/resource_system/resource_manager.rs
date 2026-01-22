
use generational_arena::Index;
use wgpu::util::{BufferInitDescriptor, DeviceExt as _};

use crate::renderer::{wgpu_context::WgpuContext, Renderer};

use super::{file_formats::{animationfile, dds}, game_resources::{CreateGameResourceRequest, CreateGameResourceResponse, GameResources, MaterialGameData, MaterialGameId}, io_manager::{IoManager, IoRequest, IoResponse}, registry::{GameState, RenderState, ResourceKind, ResourceRequest, ResourceResult}, render_resources::{AnimationClipRenderData, AnimationClipRenderId, AnimationRenderId, MaterialRenderId, MeshGpuData, MeshRenderId, ModelRenderData, ModelRenderId, RenderResources, SkeletonRenderId, TextureGpuData, TextureRenderId}, texture::upload_texture};

pub struct ResourceManager {
    io: IoManager,
    game_res_rx: crossbeam::channel::Receiver<CreateGameResourceResponse>,
    game_req_tx: crossbeam::channel::Sender<CreateGameResourceRequest>,
    registry_req_rx: crossbeam::channel::Receiver<ResourceRequest>,
    registry_res_tx: crossbeam::channel::Sender<ResourceResult>,
}
impl ResourceManager {
    pub fn new(
        registry_req_rx: crossbeam::channel::Receiver<ResourceRequest>,
        registry_res_tx: crossbeam::channel::Sender<ResourceResult>,
        game_res_rx: crossbeam::channel::Receiver<CreateGameResourceResponse>,
        game_req_tx: crossbeam::channel::Sender<CreateGameResourceRequest>,
    ) -> Self {
        Self {
            io: IoManager::new(),
            game_req_tx,
            game_res_rx,
            registry_req_rx,
            registry_res_tx,
        }
    }

    pub fn process_reg_requests(&mut self) {
        for msg in self.registry_req_rx.try_iter() {
            match msg {
                ResourceRequest::LoadModel { id, path } => self.make_io_request(IoRequest::LoadModel { id, path }),
                ResourceRequest::LoadMesh { id, path } => self.make_io_request(IoRequest::LoadMesh { id, path }),
                ResourceRequest::LoadMaterial { id, path } => self.make_io_request(IoRequest::LoadMaterial { id, path }),
                ResourceRequest::LoadSkeleton { id, path } => self.make_io_request(IoRequest::LoadSkeleton { id, path }),
                ResourceRequest::LoadAnimationClip { id, path } => self.make_io_request(IoRequest::LoadAnimationClip { id, path }),
                ResourceRequest::LoadAnimation { id, path, header } => self.make_io_request(IoRequest::LoadAnimation { id, path, header }),
                ResourceRequest::LoadTexture { id, path, srgb } => self.make_io_request(IoRequest::LoadTexture { id, path, srgb }),
            }
        }
    }

    pub fn process_game_responses(&self, renderer: &mut Renderer, render_resources: &mut RenderResources, wgpu_context: &WgpuContext) {
        while !self.game_res_rx.is_empty() {
            let res = match self.game_res_rx.recv() {
                Ok(r) => r,
                Err(err) => {
                    println!("Recv Error: {}", err);
                    break
                },
            };

            match res {
                CreateGameResourceResponse::Model { id, game_id, mesh, skeleton, animation_clips, submeshes, vertex_buffer_start_offset } => {
                    let model_render = ModelRenderData {
                        vertex_buffer_start_offset,
                        mesh_id: mesh,
                        submeshes,
                        skeleton,
                        anim_clips: animation_clips,
                    };
                    let render_idx = render_resources.models.insert(model_render);
                    if self.registry_res_tx.send(ResourceResult::ModelResult { id, game_id, render_id: ModelRenderId(render_idx) }).is_err() {
                        todo!();
                    }
                },
                CreateGameResourceResponse::Material { id, game_id, manifest, normal_texture, occlusion_texture, emissive_texture, base_color_texture, metallic_roughness_texture } => {
                    let material_binding = match renderer.upload_material(&manifest, &normal_texture, &occlusion_texture, &emissive_texture, &base_color_texture, &metallic_roughness_texture, render_resources, wgpu_context) {
                        Ok(mat) => mat,
                        Err(_) => {
                            todo!()
                        }
                    };
                    let render_id = render_resources.materials.insert(material_binding);
                    if self.registry_res_tx.send(ResourceResult::MaterialResult { id, game_id, render_id: MaterialRenderId(render_id) }).is_err() {
                        todo!();
                    }
                },
                CreateGameResourceResponse::AnimationClip { id, game_id, manifest, animation } => {
                    let data = AnimationClipRenderData {
                        manifest,
                        animation,
                    };
                    let render_id = render_resources.animation_clips.insert(data);
                    if self.registry_res_tx.send(ResourceResult::AnimationClipResult { id, game_id, render_id: AnimationClipRenderId(render_id) }).is_err() {
                        todo!();
                    }
                }
            }
        }

    }

    pub fn process_io_responses(
        &self,
        render_resources: &mut RenderResources,
        wgpu_context: &WgpuContext,
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
                    if self.game_req_tx.send(CreateGameResourceRequest::Model { id, manifest: model }).is_err() {
                        todo!()
                    }
                    // Request game id

                    /*
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
                    entry.game_state = GameState::Ready(cpu_idx);
                    */
                },
                IoResponse::MeshLoaded { id, data } => {
                    let buffer = wgpu_context.device.create_buffer_init(&BufferInitDescriptor {
                        label: Some("Index/vertex buffer"),
                        contents: bytemuck::cast_slice(&data),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
                    });
                    let mesh_gpu = MeshGpuData { buffer };
                    let idx = render_resources.meshes.insert(mesh_gpu);
                    if self.registry_res_tx.send(ResourceResult::MeshResult { id, render_id: MeshRenderId(idx) }).is_err() {
                        todo!();
                    };
                    /*
                    let cpu_data = MeshGameData { index_vertex_data: data };
                    let cpu_idx = self.cpu.meshes.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.game_state = GameState::Ready(cpu_idx);
                    entry.render_state = RenderState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                    */
                },
                IoResponse::MaterialLoaded { id, material } => {
                    if self.game_req_tx.send(CreateGameResourceRequest::Material { id, manifest: material }).is_err() {
                        todo!();
                    }
                    /*
                    let normal_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let occlusion_texture = material.occlusion_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let emissive_texture = material.emissive_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let base_color_texture = material.base_color_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let metallic_roughness_texture = material.metallic_roughness_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let cpu_data = MaterialGameData { manifest: material, normal_texture, occlusion_texture, emissive_texture, base_color_texture, metallic_roughness_texture };
                    let cpu_idx = self.cpu.materials.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.game_state = GameState::Ready(cpu_idx);
                    entry.render_state = RenderState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                    */
                },
                IoResponse::SkeletonLoaded { id, skeleton } => {
                    let idx = render_resources.skeletons.insert(skeleton);
                    self.registry_res_tx.send(ResourceResult::SkeletonResult { id, render_id: SkeletonRenderId(idx) });
                    /*
                    let cpu_data = SkeletonCpuData { manifest: skeleton };
                    let cpu_idx = self.cpu.skeletons.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.game_state = GameState::Ready(cpu_idx);
                    */
                },
                IoResponse::AnimationClipLoaded { id, clip } => {
                    if self.game_req_tx.send(CreateGameResourceRequest::AnimationClip { id, manifest: clip }).is_err() {
                        todo!();
                    }
                    /*
                    let animation = self.request_animation(&clip.binary_path, &clip);
                    let cpu_data = AnimationClipCpuData { manifest: clip, animation };
                    let cpu_idx = self.cpu.animation_clips.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.game_state = GameState::Ready(cpu_idx);
                    */
                },
                IoResponse::AnimationLoaded { id, parsed_clip } => {
                    let idx = render_resources.animations.insert(parsed_clip);
                    self.registry_res_tx.send(ResourceResult::AnimationResult { id, render_id: AnimationRenderId(idx) });
                    /*
                    let cpu_data: AnimationCpuData = parsed_clip;
                    let cpu_idx = self.cpu.animations.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.game_state = GameState::Ready(cpu_idx);
                    */
                },
                IoResponse::TextureLoaded { id, data } => {
                    // TODO move this function away from dds module, also it should probs just take the cpudata directly
                    let texture = upload_texture(
                        &data,
                        &wgpu_context,
                    );
                    // TODO probably should have a is_cubemap flag?
                    let texture_view = if data.layers == 6 {
                        texture.create_view(&wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::Cube),
                            ..Default::default()
                        })
                    } else {
                        texture.create_view(&wgpu::TextureViewDescriptor::default())
                    };
                    let texture_gpu = TextureGpuData { texture, texture_view };
                    let idx = render_resources.textures.insert(texture_gpu);
                    if self.registry_res_tx.send(ResourceResult::TextureResult { id, render_id: TextureRenderId(idx) }).is_err() {
                        todo!();
                    }

                    /*
                    let cpu_data: TextureGameData = data;
                    let cpu_idx = self.cpu.textures.lock().unwrap().insert(cpu_data);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.game_state = GameState::Ready(cpu_idx);
                    entry.render_state = RenderState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                    */
                },
                IoResponse::Error { path, message } => {
                    println!("IO Error: path: {}, message: {}", path, message);
                },
            }
        }
    }

    /*
    pub fn process_upload_queue(
        self: &std::sync::Arc<Self>,
        renderer: &mut Renderer,
        render_resources: &mut RenderResources,
        wgpu_context: &WgpuContext,
    ) {
        let mut upload_queue = self.upload_queue.lock().unwrap();
        let mut meshes_cpu = self.cpu.meshes.lock().unwrap();
        let mut textures_cpu = self.cpu.textures.lock().unwrap();
        let mut queue_next_frame: Vec<Index> = vec![];
        'upload_queue: while let Some(id) = upload_queue.pop_front() {
            let (entry_kind, entry_cpu_idx) = {
                let reg = self.registry.lock().unwrap();
                let entry = reg.entries.get(id).unwrap();

                if entry.ref_count == 0 {
                    continue 'upload_queue; // cancelled
                }

                let entry_kind = entry.kind;
                let entry_cpu_idx = if let GameState::Ready(cpu_idx) = entry.game_state {
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
                    let meshes_gpu = &mut render_resources.meshes;
                    let gpu_idx = meshes_gpu.insert(mesh_gpu);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.render_state = RenderState::Ready(gpu_idx);
                    entry.game_state = GameState::Absent;
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
                    let textures_gpu = &mut render_resources.textures;
                    let gpu_idx = textures_gpu.insert(texture_gpu);
                    let mut reg = self.registry.lock().unwrap();
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.render_state = RenderState::Ready(gpu_idx);
                    entry.game_state = GameState::Absent;
                    textures_cpu.remove(entry_cpu_idx);
                },
                ResourceKind::Material => {
                    let material_binding = match renderer.upload_material(entry_cpu_idx, self, render_resources, wgpu_context) {
                        Ok(mat) => mat,
                        Err(_) => {
                            queue_next_frame.push(id);
                            continue 'upload_queue;
                        }
                    };
                    let gpu_idx = render_resources.materials.insert(material_binding);
                    {
                        let mut reg = self.registry.lock().unwrap();
                        let entry = reg.entries.get_mut(id).unwrap();
                        entry.render_state = RenderState::Ready(gpu_idx);
                        entry.game_state = GameState::Absent;
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
    */

    fn make_io_request(&self, req: IoRequest) {
        if self.io.req_tx.send(req).is_err() {
            todo!()
        }
    }
}

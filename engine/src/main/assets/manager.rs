
use wgpu::util::{BufferInitDescriptor, DeviceExt as _};

use crate::{main::{world::Renderer, wgpu_context::WgpuContext}};
use super::io::asset_formats::materialfile;

use crate::game::assets::registry::{ResourceRequest, ResourceResult};
use crate::game::assets::store::{CreateGameResourceRequest, CreateGameResourceResponse};
use super::{store::{MaterialRenderId, MeshGpuData, MeshRenderId, ModelRenderData, ModelRenderId, RenderAssetStore, TextureGpuData, TextureRenderId}, texture::upload_texture};
use super::io::worker_pool::{IoWorkerPool, IoRequest, IoResponse};

pub struct RenderAssetManager {
    io: IoWorkerPool,
    game_res_rx: crossbeam::channel::Receiver<CreateGameResourceResponse>,
    game_req_tx: crossbeam::channel::Sender<CreateGameResourceRequest>,
    registry_req_rx: crossbeam::channel::Receiver<ResourceRequest>,
    registry_res_tx: crossbeam::channel::Sender<ResourceResult>,
}
impl RenderAssetManager {
    pub fn new(
        registry_req_rx: crossbeam::channel::Receiver<ResourceRequest>,
        registry_res_tx: crossbeam::channel::Sender<ResourceResult>,
        game_res_rx: crossbeam::channel::Receiver<CreateGameResourceResponse>,
        game_req_tx: crossbeam::channel::Sender<CreateGameResourceRequest>,
    ) -> Self {
        Self {
            io: IoWorkerPool::new(),
            game_req_tx,
            game_res_rx,
            registry_req_rx,
            registry_res_tx,
        }
    }

    pub fn process_reg_requests(
        &mut self,
    ) {
        for msg in self.registry_req_rx.try_iter() {
            match msg {
                ResourceRequest::LoadModel { id, path } => self.make_io_request(IoRequest::LoadModel { id, path }),
                ResourceRequest::LoadMesh { id, path } => self.make_io_request(IoRequest::LoadMesh { id, path }),
                ResourceRequest::LoadMaterial { id, path } => {
                    if let Some(path) = path {
                        self.make_io_request(IoRequest::LoadMaterial { id, path });
                    } else {
                        let manifest = materialfile::Material {
                            base_color_factor: [1.0, 1.0, 1.0, 1.0],
                            metallic_factor: 1.0,
                            roughness_factor: 1.0,
                            emissive_factor: [1.0, 1.0, 1.0],
                            normal_texture_scale: 1.0,
                            occlusion_strength: 1.0,
                            alpha_mode: materialfile::AlphaMode::Opaque,
                            alpha_cutoff: 1.0,
                            double_sided: false,
                            normal_texture: None,
                            occlusion_texture: None,
                            emissive_texture: None,
                            base_color_texture: None,
                            metallic_roughness_texture: None,
                        };
                        if self.game_req_tx.send(CreateGameResourceRequest::Material { id, manifest }).is_err() {
                            todo!();
                        }
                    }
                },
                ResourceRequest::LoadSkeleton { id, path } => self.make_io_request(IoRequest::LoadSkeleton { id, path }),
                ResourceRequest::LoadAnimationClip { id, path } => self.make_io_request(IoRequest::LoadAnimationClip { id, path }),
                ResourceRequest::LoadAnimation { id, path, header } => self.make_io_request(IoRequest::LoadAnimation { id, path, header }),
                ResourceRequest::LoadTexture { id, path, srgb } => self.make_io_request(IoRequest::LoadTexture { id, path, srgb }),
            }
        }
    }

    pub fn process_game_responses(&self, renderer: &mut Renderer, render_resources: &mut RenderAssetStore, wgpu_context: &WgpuContext) {
        while !self.game_res_rx.is_empty() {
            let res = match self.game_res_rx.recv() {
                Ok(r) => r,
                Err(err) => {
                    println!("Recv Error: {}", err);
                    break
                },
            };

            match res {
                CreateGameResourceResponse::Model { id, game_id, mesh, submeshes, vertex_buffer_start_offset } => {
                    let model_render = ModelRenderData {
                        vertex_buffer_start_offset,
                        mesh_id: mesh,
                        submeshes,
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
                CreateGameResourceResponse::AnimationClip { id, game_id } => {
                    if self.registry_res_tx.send(ResourceResult::AnimationClipResult { id, game_id }).is_err() {
                        todo!();
                    }
                }
                CreateGameResourceResponse::Skeleton { id, game_id } => {
                    if self.registry_res_tx.send(ResourceResult::SkeletonResult { id, game_id }).is_err() {
                        todo!();
                    }
                },
                CreateGameResourceResponse::Animation { id, game_id } => {
                    if self.registry_res_tx.send(ResourceResult::AnimationResult { id, game_id }).is_err() {
                        todo!();
                    }
                },
            }
        }

    }

    pub fn process_io_responses(
        &self,
        render_resources: &mut RenderAssetStore,
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
                },
                IoResponse::MaterialLoaded { id, material } => {
                    if self.game_req_tx.send(CreateGameResourceRequest::Material { id, manifest: material }).is_err() {
                        todo!();
                    }
                },
                IoResponse::SkeletonLoaded { id, skeleton } => {
                    if self.game_req_tx.send(CreateGameResourceRequest::Skeleton { id, manifest: skeleton }).is_err() {
                        todo!();
                    }
                },
                IoResponse::AnimationClipLoaded { id, clip } => {
                    if self.game_req_tx.send(CreateGameResourceRequest::AnimationClip { id, manifest: clip }).is_err() {
                        todo!();
                    }
                },
                IoResponse::AnimationLoaded { id, parsed_clip } => {
                    if self.game_req_tx.send(CreateGameResourceRequest::Animation { id, anim: parsed_clip }).is_err() {
                        todo!();
                    }
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
                },
                IoResponse::Error { path, message } => {
                    println!("IO Error: path: {}, message: {}", path, message);
                },
            }
        }
    }

    pub fn run_gc(
        self: &std::sync::Arc<Self>,
    ) {
        // registry sends evict messages?
        todo!();
    }

    fn make_io_request(&self, req: IoRequest) {
        if self.io.req_tx.send(req).is_err() {
            todo!()
        }
    }
}

use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix4, Quaternion, SquareMatrix};
use wgpu::{BindGroupLayout, VertexBufferLayout};
use winit::window::Window;
use wgpu::util::DeviceExt;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::rc::Rc;
use std::sync::Arc;
use pollster::FutureExt as _;
use crate::renderer::camera::{Camera, CameraBindGroups};
use crate::renderer::wgpu_context::WgpuContext;
use crate::renderer::depth_texture::DepthTexture;
use crate::renderer::glb::GLTFSceneRef;

use super::pipeline::{get_primitive_pipeline_config, PipelineCache, PipelineConfig, ShaderCache};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Matrix4f32 {
    data: [[f32; 4]; 4],
}

impl From<Matrix4<f32>> for Matrix4f32 {
    fn from(m4: Matrix4<f32>) -> Self {
        Matrix4f32 {
            data: m4.into()
        }
    }
}

struct PrimitiveRenderContext2 {
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_index_buffer: wgpu::Buffer,
    vertex_index_count: u32,
}

/*
* Contains the subset of the primitives of a mesh that use the same pipeline
*/
struct MeshRenderContext {
    primitives: Vec<PrimitiveRenderContext2>,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
}

struct PipelineRenderContext {
    pipeline: Rc<wgpu::RenderPipeline>,
    meshes: Vec<MeshRenderContext>,
}

type PipelineRenderContextMap = HashMap<PipelineConfig, PipelineRenderContext>;
type MeshInstancesMap = HashMap<usize, Vec<Matrix4f32>>;

fn construct_mesh_instances_map(scene: &GLTFSceneRef, node_idx: usize, mut transform: Matrix4<f32>, acc: &mut HashMap<usize, Vec<Matrix4f32>>) {
    let node = &scene.desc.nodes[node_idx];

    if let Some(v) = node.scale {
        transform = transform * Matrix4::from_nonuniform_scale(v[0] as f32, v[1] as f32, v[2] as f32);
    }
    if let Some(v) = node.rotation {
        transform = transform * Matrix4::from(Quaternion::new(v[3] as f32, v[0] as f32, v[1] as f32, v[2] as f32));
    }
    if let Some(v) = node.translation {
        transform = transform * Matrix4::from_translation(cgmath::Vector3::from(v.map(|x| x as f32)));
    }
    if let Some(m) = node.matrix {
        let m: [f32; 16] = m.map(|x| x as f32);
        let m: Matrix4<f32> = Matrix4::new(
            m[0],  m[1],  m[2],  m[3],
            m[4],  m[5],  m[6],  m[7],
            m[8],  m[9],  m[10], m[11],
            m[12], m[13], m[14], m[15]
        );
        transform = transform * m;
    }
    if let Some(mesh) = node.mesh {
        acc.entry(mesh as usize).or_insert(Vec::new()).push(Matrix4f32::from(transform.clone()));
    }
    if let Some(children) = &node.children {
        for child_idx in children {
            construct_mesh_instances_map(scene, *child_idx, transform.clone(), acc);
        }
    }
}

fn scene_to_mesh_instances(scene: &GLTFSceneRef) -> MeshInstancesMap {
    let mut map: HashMap<usize, Vec<Matrix4f32>> = HashMap::new();
    let transform = Matrix4::identity();

    // Only rendering the main scene for now
    let scene_nodes = &scene.desc.scenes[scene.desc.scene].nodes;
    for node_idx in scene_nodes {
        construct_mesh_instances_map(scene, *node_idx, transform, &mut map);
    }

    map
}

fn scene_to_pipeline_render_context_map<'scene>(
    scene: &'scene GLTFSceneRef,
    wgpu_context: &WgpuContext,
    pipeline_cache: &mut PipelineCache,
    shader_cache: &mut ShaderCache,
) -> PipelineRenderContextMap {
    let mut pipeline_render_context_map: PipelineRenderContextMap = HashMap::new();

    let global_bind_group_layouts: Vec<Vec<wgpu::BindGroupLayoutEntry>> = vec![
        // TODO get this from camera, this is just temp for testing
        vec![
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
        ],
    ];

    let mesh_instances_map = scene_to_mesh_instances(scene);

    for mesh_idx in 0..scene.desc.meshes.len() {
        let mesh = &scene.desc.meshes[mesh_idx];
        let mut mesh_context_map: HashMap<PipelineConfig, Vec<PrimitiveRenderContext2>> = HashMap::new();
        for primitive in &mesh.primitives {
            let pipeline_config = get_primitive_pipeline_config(scene, primitive);

            let mut vertex_buffers = vec![
                wgpu_context.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Vertex Buffer"),
                        contents: scene.accessor_data[primitive.attributes.position as usize].buffer_slice,
                        usage: wgpu::BufferUsages::VERTEX,
                    }
                )
            ];
            
            if let Some(n) = primitive.attributes.normal {
                vertex_buffers.push(
                    wgpu_context.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Normal Buffer"),
                            contents: scene.accessor_data[n as usize].buffer_slice,
                            usage: wgpu::BufferUsages::VERTEX,
                        }
                    )
                );
            }

            let vertex_index_buffer = wgpu_context.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: scene.accessor_data[primitive.indices as usize].buffer_slice,
                    usage: wgpu::BufferUsages::INDEX,
                }
            );
            let vertex_index_count = scene.desc.accessors[primitive.indices as usize].count;

            let primitive_render_context = PrimitiveRenderContext2 {
                vertex_buffers,
                vertex_index_count,
                vertex_index_buffer,
            };

            mesh_context_map.entry(
                pipeline_config.clone()
            ).or_insert(
                vec![]
            ).push(primitive_render_context);
        }

        let instance_data: &Vec<Matrix4f32> = mesh_instances_map.get(&mesh_idx).unwrap();

        for (pipeline_config, primitive_render_contexts) in mesh_context_map.into_iter() {
            let render_context = pipeline_render_context_map.entry(
                pipeline_config.clone()
            ).or_insert(
                PipelineRenderContext {
                    pipeline: pipeline_cache.get_pipeline(
                        &pipeline_config,
                        &global_bind_group_layouts,
                        &wgpu_context.device,
                        &wgpu_context.surface_config,
                        shader_cache
                    ),
                    meshes: vec![]
                }
            );

            let instance_buffer = wgpu_context.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX,
                }
            );
            let instance_count = instance_data.len();

            render_context.meshes.push(
                MeshRenderContext {
                    primitives: primitive_render_contexts,
                    instance_buffer,
                    instance_count: instance_count as u32,
                }
            );
        }
    }

    pipeline_render_context_map
}

fn render(
    pipeline_render_context_map: &PipelineRenderContextMap,
    global_bind_groups: Vec<&wgpu::BindGroup>,
    depth_texture: &DepthTexture,
    wgpu_context: &WgpuContext,
) -> Result<(), wgpu::SurfaceError> {
    let output = wgpu_context.surface.get_current_texture()?;
    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = wgpu_context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        for i in 0..global_bind_groups.len() {
            render_pass.set_bind_group(i as u32, global_bind_groups[i], &[]);
        }

        for ctx in pipeline_render_context_map.values() {
            render_pass.set_pipeline(&ctx.pipeline);
            for mesh_render_context in &ctx.meshes {
                for primitive_render_context in &mesh_render_context.primitives {
                    render_pass.set_vertex_buffer(0, mesh_render_context.instance_buffer.slice(..));
                    for i in 0..primitive_render_context.vertex_buffers.len() {
                        render_pass.set_vertex_buffer(i as u32 + 1, primitive_render_context.vertex_buffers[i].slice(..));
                    }
                    // TODO get index accessor component type
                    render_pass.set_index_buffer(primitive_render_context.vertex_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    render_pass.draw_indexed(0..primitive_render_context.vertex_index_count, 0, 0..mesh_render_context.instance_count);
                }
            }
        }
    }

    wgpu_context.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

pub struct Renderer<'surface> {
    wgpu_context: WgpuContext<'surface>,
    camera: Camera,
    camera_bind_groups: CameraBindGroups,
    depth_texture: DepthTexture,
    prcm: PipelineRenderContextMap,
}

impl<'surface> Renderer<'surface> {
    pub async fn new<'scene>(
        window: Arc<Window>, scene: &GLTFSceneRef<'scene>,
        pipeline_cache: &mut PipelineCache,
        shader_cache: &mut ShaderCache,
    ) -> Self {
        let wgpu_context = WgpuContext::new(window).await;
        let camera = Camera::new(&wgpu_context);
        let camera_bind_groups = CameraBindGroups::new(&camera, &wgpu_context);
        let depth_texture = DepthTexture::create_depth_texture(&wgpu_context);
        let prcm = scene_to_pipeline_render_context_map(scene, &wgpu_context, pipeline_cache, shader_cache);

        Self {
            wgpu_context,
            camera,
            camera_bind_groups,
            depth_texture,
            prcm,
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let global_bind_groups = vec![&self.camera_bind_groups.camera_bind_group];
        render(&self.prcm, global_bind_groups, &self.depth_texture, &self.wgpu_context)
    }

    pub fn reload_shaders(&mut self) {
        // TODO
        /*
        self.render_pipeline = Renderer::create_render_pipeline(
            &self.wgpu_context, &self.vertex_buffer_layouts,
            &self.camera_bind_groups.camera_bind_group_layout,
            &self.camera_bind_groups.view_invert_transpose_bind_group_layout
        );
        */
        //self.render().unwrap();
    }

    pub fn resize(&mut self, new_size: Option<winit::dpi::PhysicalSize<u32>>) {
        let new_size = new_size.unwrap_or(self.wgpu_context.window.inner_size());
        if new_size.width > 0 && new_size.height > 0 {
            self.wgpu_context.surface_config.width = new_size.width;
            self.wgpu_context.surface_config.height = new_size.height;
            self.wgpu_context.surface.configure(&self.wgpu_context.device, &self.wgpu_context.surface_config);
            self.depth_texture = DepthTexture::create_depth_texture(&self.wgpu_context);
            self.camera.aspect = self.wgpu_context.surface_config.width as f32 / self.wgpu_context.surface_config.height as f32;
        }
    }

    // update_camera_bindings should be called after mutating the camera
    pub fn get_camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    pub fn update_camera_bindings(&mut self) {
        self.camera_bind_groups = CameraBindGroups::new(&self.camera, &self.wgpu_context)
    }
}


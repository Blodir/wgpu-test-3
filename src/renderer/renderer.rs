use wgpu::{BindGroupLayout, VertexBufferLayout};
use winit::window::Window;
use wgpu::util::DeviceExt;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;
use pollster::FutureExt as _;
use crate::renderer::camera::{Camera, CameraBindGroups};
use crate::renderer::wgpu_context::WgpuContext;
use crate::renderer::depth_texture::DepthTexture;
use crate::renderer::glb::GLTFSceneRef;

use super::pipeline::{get_primitive_pipeline_config, PipelineCache, PipelineConfig, ShaderCache};

struct PrimitiveRenderContext {
    pipeline_config: PipelineConfig,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_index_buffer: wgpu::Buffer,
    vertex_index_count: u32,
    mesh_idx: usize,
}

struct PrimitiveRenderer {
    render_contexts_by_pipeline: HashMap::<PipelineConfig, Vec<PrimitiveRenderContext>>,
    global_bind_group_layouts: Vec<Vec<wgpu::BindGroupLayoutEntry>>,
}
impl PrimitiveRenderer {
    pub fn new(scene: &GLTFSceneRef, wgpu_context: &WgpuContext) -> Self {
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

        let mut render_contexts_by_pipeline = HashMap::<PipelineConfig, Vec<PrimitiveRenderContext>>::new();

        for mesh_idx in 0..scene.desc.meshes.len() {
            let mesh = &scene.desc.meshes[mesh_idx];
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

                let primitive_render_context = PrimitiveRenderContext {
                    pipeline_config,
                    vertex_buffers,
                    vertex_index_count,
                    vertex_index_buffer,
                    mesh_idx,
                };

                render_contexts_by_pipeline.entry(primitive_render_context.pipeline_config.clone()).or_insert(vec![]).push(primitive_render_context);
            }
        }

        Self {
            render_contexts_by_pipeline, global_bind_group_layouts,
        }
    }

    pub fn render(&self, wgpu_context: &WgpuContext, depth_texture: &DepthTexture, camera_bind_groups: &CameraBindGroups, pipeline_cache: &mut PipelineCache, shader_cache: &mut ShaderCache) -> Result<(), wgpu::SurfaceError> {
        let output = wgpu_context.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = wgpu_context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        let mut pipelines = vec![];
        for render_contexts in self.render_contexts_by_pipeline.values() {
            pipelines.push(
                (
                    pipeline_cache.get_pipeline(&render_contexts.first().unwrap().pipeline_config, &self.global_bind_group_layouts, &wgpu_context.device, &wgpu_context.surface_config, shader_cache),
                    render_contexts
                )
            );
        }

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

            render_pass.set_bind_group(0, &camera_bind_groups.camera_bind_group, &[]);
            for (pipeline, render_contexts) in &pipelines {
                render_pass.set_pipeline(pipeline);
                for render_context in *render_contexts {
                    for i in 0..render_context.vertex_buffers.len() {
                        render_pass.set_vertex_buffer(i as u32, render_context.vertex_buffers[i].slice(..));
                    }
                    // TODO get index accessor component type
                    render_pass.set_index_buffer(render_context.vertex_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    render_pass.draw_indexed(0..render_context.vertex_index_count, 0, 0..1);
                }
            }
        }

        wgpu_context.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub struct Renderer<'surface> {
    wgpu_context: WgpuContext<'surface>,
    camera: Camera,
    camera_bind_groups: CameraBindGroups,
    depth_texture: DepthTexture,
    child_renderer: PrimitiveRenderer,
}

impl<'surface> Renderer<'surface> {
    pub async fn new<'scene>(
        window: Arc<Window>, scene: &GLTFSceneRef<'scene>,
    ) -> Self {
        let wgpu_context = WgpuContext::new(window).await;
        let camera = Camera::new(&wgpu_context);
        let camera_bind_groups = CameraBindGroups::new(&camera, &wgpu_context);

        let child_renderer = PrimitiveRenderer::new(scene, &wgpu_context);

        let depth_texture = DepthTexture::create_depth_texture(&wgpu_context);

        Self {
            wgpu_context,
            camera,
            camera_bind_groups,
            depth_texture,
            child_renderer,
        }
    }

    pub fn render(
        &mut self, pipeline_cache: & mut PipelineCache, shader_cache: & mut ShaderCache,
    ) -> Result<(), wgpu::SurfaceError> {
        self.child_renderer.render(&self.wgpu_context, &self.depth_texture, &self.camera_bind_groups, pipeline_cache, shader_cache)
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


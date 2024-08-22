use std::sync::Arc;

use winit::window::Window;

use super::{camera::{Camera, CameraBinding, CameraUniform}, lights::{Lights, LightsBinding}, wgpu_context::WgpuContext};

pub struct DepthTexture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl DepthTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::LessEqual),
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                ..Default::default()
            }
        );

        Self { 
            texture, view, sampler
        }
    }
}

pub struct World {
    pub camera: Camera,
    pub lights: Lights,
    pub pbr_meshes: Vec<super::pbr::Mesh>,
}
pub struct WorldBinding {
    camera_binding: CameraBinding,
    lights_binding: LightsBinding,
    pbr_mesh_bindings: Vec<super::pbr::MeshBinding>,
}
impl World {
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pbr_material_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> WorldBinding {
        let camera_binding = self.camera.to_camera_uniform().upload(device, camera_bind_group_layout);
        let lights_binding = self.lights.upload(device, lights_bind_group_layout);
        let pbr_mesh_bindings = self.pbr_meshes.iter().map(|mesh| {
            mesh.upload(device, queue, pbr_material_bind_group_layout)
        }).collect();

        WorldBinding { camera_binding, lights_binding, pbr_mesh_bindings }
    }
}

pub struct Renderer<'surface> {
    wgpu_context: WgpuContext<'surface>,
    depth_texture: DepthTexture,
    pbr_material_pipeline: super::pbr::MaterialPipeline,
    world_binding: WorldBinding,
    world: World,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    lights_bind_group_layout: wgpu::BindGroupLayout,
}
impl<'surface> Renderer<'surface> {
    pub async fn new(
        window: Arc<Window>,
        pbr_meshes: Vec<super::pbr::Mesh>,
    ) -> Self {
        let wgpu_context = WgpuContext::new(window).await;
        let depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let camera_bind_group_layout = wgpu_context.device.create_bind_group_layout(&CameraUniform::desc());
        let lights_bind_group_layout = wgpu_context.device.create_bind_group_layout(&Lights::desc());
        let pbr_material_pipeline = super::pbr::MaterialPipeline::new(
            &wgpu_context.device, &wgpu_context.surface_config,
            &camera_bind_group_layout, &lights_bind_group_layout
        );

        let camera = Camera::new(&wgpu_context.surface_config);
        let lights = Lights::default();
        let world = World { camera, lights, pbr_meshes };
        let world_binding = world.upload(
            &wgpu_context.device, &wgpu_context.queue,
            &pbr_material_pipeline.material_bind_group_layout,
            &camera_bind_group_layout, &lights_bind_group_layout
        );
        
        Self { wgpu_context, depth_texture, pbr_material_pipeline, world_binding, world, camera_bind_group_layout, lights_bind_group_layout }
    }

    pub fn reload_pbr_pipeline(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.pbr_material_pipeline.rebuild_pipeline(
            &self.wgpu_context.device, &self.wgpu_context.surface_config,
            &self.camera_bind_group_layout, &self.lights_bind_group_layout,
        );
        self.render()
    }

    pub fn render(
        &self,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.wgpu_context.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.wgpu_context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pbr_material_pipeline.render_pipeline);
            render_pass.set_bind_group(0u32, &self.world_binding.camera_binding.bind_group, &[]);
            render_pass.set_bind_group(1u32, &self.world_binding.lights_binding.bind_group, &[]);

            for mesh in &self.world_binding.pbr_mesh_bindings {
                render_pass.set_vertex_buffer(0, mesh.instance_buffer.slice(..));
                for primitive in &mesh.primitives {
                    render_pass.set_bind_group(2u32, &primitive.material_binding.bind_group, &[]);
                    render_pass.set_vertex_buffer(1u32, primitive.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(primitive.index_buffer.slice(..), primitive.index_format);
                    render_pass.draw_indexed(0..primitive.index_count, 0, 0..mesh.instance_count);
                }
            }
        }

        self.wgpu_context.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    pub fn resize(&mut self, new_size: Option<winit::dpi::PhysicalSize<u32>>) {
        let new_size = new_size.unwrap_or(self.wgpu_context.window.inner_size());
        if new_size.width > 0 && new_size.height > 0 {
            self.wgpu_context.surface_config.width = new_size.width;
            self.wgpu_context.surface_config.height = new_size.height;
            self.wgpu_context.surface.configure(&self.wgpu_context.device, &self.wgpu_context.surface_config);
            self.depth_texture = DepthTexture::new(&self.wgpu_context.device, &self.wgpu_context.surface_config);
            self.world.camera.aspect = self.wgpu_context.surface_config.width as f32 / self.wgpu_context.surface_config.height as f32;
            self.update_camera();
        }
    }

    pub fn get_camera_mut(&mut self) -> &mut Camera {
        &mut self.world.camera
    }

    pub fn update_camera(&self) {
        self.world_binding.camera_binding.update(&self.world.camera.to_camera_uniform(), &self.wgpu_context.queue);
    }
}


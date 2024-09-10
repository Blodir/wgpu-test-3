use std::{fs::File, io::Read, sync::Arc};

use winit::window::Window;

use super::{camera::{Camera, CameraBinding, CameraUniform}, lights::{Lights, LightsBinding}, pipelines::{diffuse_irradiance::DiffuseIrradiancePipeline, equirectangular::{render_cubemap, FaceRotation}, skybox::create_test_cubemap_texture}, wgpu_context::WgpuContext};

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

pub struct EnvironmentMapBinding {
    pub bind_group: wgpu::BindGroup,
}
impl EnvironmentMapBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Environment Map Bind Group Layout"),
        }
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: image::DynamicImage,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = render_cubemap(device, queue, image).unwrap();
        //let texture = create_test_cubemap_texture(device, queue, 512, wgpu::TextureFormat::Rgba8Unorm);
        let cubemap_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cubemap Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge, // Ensure texture coordinates are clamped across all cubemap faces
            mag_filter: wgpu::FilterMode::Linear, // Smooth magnification
            min_filter: wgpu::FilterMode::Linear, // Smooth minification
            mipmap_filter: wgpu::FilterMode::Linear, // Smooth mipmap transition if mipmaps are used
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0, // High value to cover any mipmap range
            compare: None, // Not typically used for cubemaps unless needed for specific effects
            anisotropy_clamp: 1, // Optionally enable anisotropic filtering (e.g., Some(16))
            border_color: None, // Only relevant if using ClampToBorder
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Environment Cubemap Bind Group"),
            layout: bind_group_layout,
            entries: &[
                    wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&cubemap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self { bind_group }
    }
}

pub struct DiffuseIrradianceBinding {
    pub bind_group: wgpu::BindGroup,
}
impl DiffuseIrradianceBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Diffuse Irradiance Bind Group Layout"),
        }
    }

    pub fn from_environment_map(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        environment_map_binding: &EnvironmentMapBinding,
        environment_map_bind_group_layout: &wgpu::BindGroupLayout,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let face_rot_bind_group_layout = device.create_bind_group_layout(&FaceRotation::desc());
        let pipeline = DiffuseIrradiancePipeline::new(device, &face_rot_bind_group_layout, environment_map_bind_group_layout);
        let cubemap = pipeline.render(device, queue, environment_map_binding, &face_rot_bind_group_layout).unwrap();
        let cubemap_view = cubemap.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cubemap Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge, // Ensure texture coordinates are clamped across all cubemap faces
            mag_filter: wgpu::FilterMode::Linear, // Smooth magnification
            min_filter: wgpu::FilterMode::Linear, // Smooth minification
            mipmap_filter: wgpu::FilterMode::Linear, // Smooth mipmap transition if mipmaps are used
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0, // High value to cover any mipmap range
            compare: None, // Not typically used for cubemaps unless needed for specific effects
            anisotropy_clamp: 1, // Optionally enable anisotropic filtering (e.g., Some(16))
            border_color: None, // Only relevant if using ClampToBorder
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Diffuse Irradiance Cubemap Bind Group"),
            layout: bind_group_layout,
            entries: &[
                    wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&cubemap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self { bind_group }
    }
}

pub struct World {
    pub camera: Camera,
    pub lights: Lights,
    pub pbr_meshes: Vec<super::pbr::Mesh>,
    pub environment_map: image::DynamicImage,
}
pub struct WorldBinding {
    pub camera_binding: CameraBinding,
    pub lights_binding: LightsBinding,
    pub pbr_mesh_bindings: Vec<super::pbr::MeshBinding>,
    pub environment_map_binding: EnvironmentMapBinding,
    pub diffuse_irradiance_binding: DiffuseIrradianceBinding,
}
impl World {
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pbr_material_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        environment_map_bind_group_layout: &wgpu::BindGroupLayout,
        diffuse_irradiance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> WorldBinding {
        let camera_binding = self.camera.to_camera_uniform().upload(device, camera_bind_group_layout);
        let lights_binding = self.lights.upload(device, lights_bind_group_layout);
        let pbr_mesh_bindings = self.pbr_meshes.iter().map(|mesh| {
            mesh.upload(device, queue, pbr_material_bind_group_layout)
        }).collect();
        let environment_map_binding = EnvironmentMapBinding::from_image(device, queue, self.environment_map.clone(), environment_map_bind_group_layout);
        let diffuse_irradiance_binding = DiffuseIrradianceBinding::from_environment_map(device, queue, &environment_map_binding, environment_map_bind_group_layout, &diffuse_irradiance_bind_group_layout);

        WorldBinding { camera_binding, lights_binding, pbr_mesh_bindings, environment_map_binding, diffuse_irradiance_binding }
    }
}

pub struct Renderer<'surface> {
    wgpu_context: WgpuContext<'surface>,
    depth_texture: DepthTexture,
    skybox_pipeline: super::pipelines::skybox::SkyboxPipeline,
    pbr_material_pipeline: super::pbr::MaterialPipeline,
    world_binding: WorldBinding,
    world: World,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    lights_bind_group_layout: wgpu::BindGroupLayout,
    diffuse_irradiance_bind_group_layout: wgpu::BindGroupLayout,
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
        let environment_map_bind_group_layout = wgpu_context.device.create_bind_group_layout(&EnvironmentMapBinding::desc());
        let diffuse_irradiance_bind_group_layout = wgpu_context.device.create_bind_group_layout(&DiffuseIrradianceBinding::desc());

        let skybox_pipeline = super::pipelines::skybox::SkyboxPipeline::new(
            &wgpu_context.device, &wgpu_context.surface_config,
            &camera_bind_group_layout, &environment_map_bind_group_layout
        );
        let pbr_material_pipeline = super::pbr::MaterialPipeline::new(
            &wgpu_context.device, &wgpu_context.surface_config,
            &camera_bind_group_layout, &lights_bind_group_layout,
            &diffuse_irradiance_bind_group_layout
        );

        let camera = Camera::new(&wgpu_context.surface_config);
        let lights = Lights::default();
        
        let environment_map = {
            let mut file = File::open("illovo_beach_balcony_4k.hdr").unwrap();
            let mut buf: Vec<u8> = vec![];
            file.read_to_end(&mut buf).unwrap();
            image::load_from_memory(&buf).unwrap()
        };

        let world = World { camera, lights, pbr_meshes, environment_map };
        let world_binding = world.upload(
            &wgpu_context.device, &wgpu_context.queue,
            &pbr_material_pipeline.material_bind_group_layout,
            &camera_bind_group_layout, &lights_bind_group_layout,
            &environment_map_bind_group_layout, &diffuse_irradiance_bind_group_layout,
        );
        
        Self { wgpu_context, depth_texture, skybox_pipeline, pbr_material_pipeline, world_binding, world, camera_bind_group_layout, lights_bind_group_layout, diffuse_irradiance_bind_group_layout }
    }

    pub fn reload_pbr_pipeline(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.pbr_material_pipeline.rebuild_pipeline(
            &self.wgpu_context.device, &self.wgpu_context.surface_config,
            &self.camera_bind_group_layout, &self.lights_bind_group_layout,
            &self.diffuse_irradiance_bind_group_layout
        );
        self.render()
    }

    pub fn render(
        &self,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.wgpu_context.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.skybox_pipeline.render(&self.wgpu_context.device, &self.wgpu_context.queue, &view, &self.depth_texture.view, &self.world_binding)?;

        // TODO move the render pass to pbr pipeline:
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
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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
            render_pass.set_bind_group(3u32, &self.world_binding.diffuse_irradiance_binding.bind_group, &[]);

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


use std::{fmt::Debug, fs::File, io::Read, sync::Arc};

use image::ImageReader;
use winit::window::Window;

use super::{
    camera::{Camera, CameraBinding, CameraUniform}, depth_texture::DepthTexture, lights::{Lights, LightsBinding}, pipelines::{
        diffuse_irradiance::DiffuseIrradiancePipeline, env_prefilter::EnvPrefilterPipeline, equirectangular::{
            render_cubemap, write_texture_to_file, FaceRotation,
        }, pbr::{
            MaterialPipeline, Mesh, MeshBinding, SamplerOptions
        }, skybox::create_test_cubemap_texture
    }, wgpu_context::WgpuContext
};

pub struct EnvironmentMapBinding {
    pub bind_group: wgpu::BindGroup,
    pub texture: wgpu::Texture,
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
                // Diffuse irradiance
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // BRDF LUT
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
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

        let cubemap_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let env_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
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
        let temp_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
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
        );

        let temp_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Environment Cubemap Bind Group"),
            layout: &temp_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&cubemap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&env_map_sampler),
                },
            ],
        });

        // --------------------- //
        // MIPMAPS
        // -------------------- //

        let face_rot_bind_group_layout = device.create_bind_group_layout(&FaceRotation::desc());
        let pipeline = EnvPrefilterPipeline::new(device, &face_rot_bind_group_layout, &temp_bind_group_layout);
        let resolution = texture.width();
        let texture = pipeline.render(device, queue, &texture, &temp_bind_group, &face_rot_bind_group_layout, resolution).unwrap();
        let env_map_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            format: Some(wgpu::TextureFormat::Rgba16Float),
            ..Default::default()
        });

        let (di_view, di_sampler) = {
            let face_rot_bind_group_layout = device.create_bind_group_layout(&FaceRotation::desc());
            let pipeline = DiffuseIrradiancePipeline::new(device, &face_rot_bind_group_layout, &temp_bind_group_layout);
            let cubemap = pipeline.render(device, queue, &temp_bind_group, &face_rot_bind_group_layout).unwrap();
            let view = cubemap.create_view(&wgpu::TextureViewDescriptor {
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
            (view, sampler)
        };

        let (brdf_view, brdf_sampler) = {
            let brdf_lut = {
                let mut file = File::open("assets/brdf_lut.png").unwrap();
                let mut buf: Vec<u8> = vec![];
                file.read_to_end(&mut buf).unwrap();
                image::load_from_memory(&buf).unwrap()
            };
            let t = super::texture::Texture::from_image(
                device, queue,
                &(
                    brdf_lut,
                    Some(
                        SamplerOptions {
                            mag_filter: wgpu::FilterMode::Linear,
                            min_filter: wgpu::FilterMode::Linear,
                            address_mode_u: wgpu::AddressMode::ClampToEdge,
                            address_mode_v: wgpu::AddressMode::ClampToEdge
                        }
                    )
                ),
                true
            );
            (t.view, t.sampler)
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Environment Cubemap Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&env_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&env_map_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&di_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&di_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&brdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&brdf_sampler),
                },
            ],
        });
        Self { bind_group, texture }
    }
}

pub struct World {
    pub camera: Camera,
    pub lights: Lights,
    pub pbr_meshes: Vec<Mesh>,
    pub environment_map: image::DynamicImage,
}
pub struct WorldBinding {
    pub camera_binding: CameraBinding,
    pub lights_binding: LightsBinding,
    pub pbr_mesh_bindings: Vec<MeshBinding>,
    pub environment_map_binding: EnvironmentMapBinding,
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
    ) -> WorldBinding {
        let camera_binding = self.camera.to_camera_uniform().upload(device, camera_bind_group_layout);
        let lights_binding = self.lights.upload(device, lights_bind_group_layout);
        let pbr_mesh_bindings = self.pbr_meshes.iter().map(|mesh| {
            mesh.upload(device, queue, pbr_material_bind_group_layout)
        }).collect();
        let environment_map_binding = EnvironmentMapBinding::from_image(device, queue, self.environment_map.clone(), environment_map_bind_group_layout);

        WorldBinding { camera_binding, lights_binding, pbr_mesh_bindings, environment_map_binding }
    }
}

pub struct Renderer<'surface> {
    wgpu_context: WgpuContext<'surface>,
    depth_texture: DepthTexture,
    skybox_pipeline: super::pipelines::skybox::SkyboxPipeline,
    pbr_material_pipeline: MaterialPipeline,
    world_binding: WorldBinding,
    world: World,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    lights_bind_group_layout: wgpu::BindGroupLayout,
    environment_map_bind_group_layout: wgpu::BindGroupLayout,
}
impl<'surface> Renderer<'surface> {
    pub async fn new(
        window: Arc<Window>,
        pbr_meshes: Vec<Mesh>,
    ) -> Self {
        let wgpu_context = WgpuContext::new(window).await;
        let depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let camera_bind_group_layout = wgpu_context.device.create_bind_group_layout(&CameraUniform::desc());
        let lights_bind_group_layout = wgpu_context.device.create_bind_group_layout(&Lights::desc());
        let environment_map_bind_group_layout = wgpu_context.device.create_bind_group_layout(&EnvironmentMapBinding::desc());

        let skybox_pipeline = super::pipelines::skybox::SkyboxPipeline::new(
            &wgpu_context.device, &wgpu_context.surface_config,
            &camera_bind_group_layout, &environment_map_bind_group_layout
        );
        let pbr_material_pipeline = MaterialPipeline::new(
            &wgpu_context.device, &wgpu_context.surface_config,
            &camera_bind_group_layout, &lights_bind_group_layout,
            &environment_map_bind_group_layout
        );

        let camera = Camera::new(&wgpu_context.surface_config);
        let lights = Lights::default();
        
        let environment_map = {
            let img = ImageReader::open("hayloft_8k.hdr")
                .expect("Failed to open environment map")
                .decode()
                .expect("Failed to decode environment map");
            img
        };

        let world = World { camera, lights, pbr_meshes, environment_map };
        let world_binding = world.upload(
            &wgpu_context.device, &wgpu_context.queue,
            &pbr_material_pipeline.material_bind_group_layout,
            &camera_bind_group_layout, &lights_bind_group_layout,
            &environment_map_bind_group_layout
        );
        
        Self { wgpu_context, depth_texture, skybox_pipeline, pbr_material_pipeline, world_binding, world, camera_bind_group_layout, lights_bind_group_layout, environment_map_bind_group_layout }
    }

    pub fn reload_pbr_pipeline(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.pbr_material_pipeline.rebuild_pipeline(
            &self.wgpu_context.device, &self.wgpu_context.surface_config,
            &self.camera_bind_group_layout, &self.lights_bind_group_layout,
            &self.environment_map_bind_group_layout
        );
        self.render()
    }

    pub fn render(
        &self,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.wgpu_context.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.skybox_pipeline.render(&self.wgpu_context.device, &self.wgpu_context.queue, &view, &self.depth_texture.view, &self.world_binding)?;
        self.pbr_material_pipeline.render(&self.wgpu_context.device, &self.wgpu_context.queue, &view, &self.depth_texture.view, &self.world_binding);

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


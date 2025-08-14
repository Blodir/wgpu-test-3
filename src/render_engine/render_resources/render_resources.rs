use std::collections::HashMap;

use super::dds;
use super::modelfile;
use super::png;
use cgmath::{Matrix3, Matrix4, Quaternion, Rotation3 as _, SquareMatrix};
use wgpu::{util::DeviceExt as _, SamplerDescriptor, TextureViewDescriptor};

use crate::{
    render_engine::pipelines::model::{instance::Instance, material::MaterialBinding},
    render_engine::wgpu_context::{self, WgpuContext},
    scene_tree::{Camera, Sun},
};

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct ModelHandle(pub String);

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct TextureHandle(pub String);

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct MaterialHandle(u32);

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct EnvironmentMapHandle(pub String);

pub struct MaterialPool {
    materials: HashMap<MaterialHandle, MaterialBinding>,
    next_id: u32,
}
impl MaterialPool {
    pub fn new() -> Self {
        Self {
            materials: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn insert(&mut self, material: MaterialBinding) -> MaterialHandle {
        let handle = MaterialHandle(self.next_id);
        self.materials.insert(handle.clone(), material);
        self.next_id += 1;
        handle
    }

    pub fn get(&self, handle: &MaterialHandle) -> Option<&MaterialBinding> {
        self.materials.get(handle)
    }
}

fn mult_primitive_instances(
    model: &modelfile::Model,
    transforms: &Vec<Matrix4<f32>>,
) -> (Vec<Instance>, Vec<u32>) {
    let mut instances: Vec<Instance> = vec![];
    let mut instance_counts = vec![];
    for prim in &model.primitives {
        let mut inst_count = 0;
        for instance in &prim.instances {
            let inst_m4 = Matrix4::from(*instance);
            for transform in transforms {
                let t = transform * inst_m4;
                instances.push(Instance::from_transform(t));
                inst_count += 1;
            }
        }
        instance_counts.push(inst_count);
    }
    (instances, instance_counts)
}

pub struct ModelData {
    /// combined index and vertex buffer
    pub index_vertex_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub primitive_instance_counts: Vec<u32>,
    pub json: modelfile::Model,
    pub materials: Vec<MaterialHandle>,
}
impl ModelData {
    pub fn update_instance_buffer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transforms: &Vec<Matrix4<f32>>,
    ) {
        let (instances, instance_counts) = mult_primitive_instances(&self.json, transforms);
        self.primitive_instance_counts = instance_counts;
        let instance_bytes: &[u8] = bytemuck::cast_slice(&instances);
        if self.instance_buffer.size() >= instance_bytes.len() as u64 {
            queue.write_buffer(&self.instance_buffer, 0, instance_bytes);
        } else {
            self.instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance buffer"),
                contents: instance_bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        }
    }
}

pub struct SunBinding {
    direction_buffer: wgpu::Buffer,
    color_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl SunBinding {
    pub fn new(
        sun: &Sun,
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let direction_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Direction Buffer"),
            contents: bytemuck::cast_slice(&sun.direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Color Buffer"),
            contents: bytemuck::cast_slice(&sun.color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: direction_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_buffer.as_entire_binding(),
                },
            ],
            label: Some("Lights Bind Group"),
        });

        SunBinding {
            bind_group,
            direction_buffer,
            color_buffer,
        }
    }

    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Sun Bind Group Layout"),
        }
    }

    pub fn update() {
        todo!();
    }
}

pub struct CameraMatrices {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub inverse_view_proj_rot: [[f32; 4]; 4],
}

pub struct CameraBinding {
    view_proj_buffer: wgpu::Buffer,
    position_buffer: wgpu::Buffer,
    inverse_view_proj_rot_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl CameraBinding {
    pub fn new(
        camera: &Camera,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let matrices = CameraBinding::camera_to_matrices(camera, surface_config);
        let view_proj = matrices.view_proj;
        let position = matrices.position;
        let inverse_view_proj_rot = matrices.inverse_view_proj_rot;
        let view_proj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Projection Buffer"),
            contents: bytemuck::cast_slice(&view_proj),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let position_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Position Buffer"),
            contents: bytemuck::cast_slice(&position),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let inverse_view_proj_rot_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Inverse View Projection Buffer"),
                contents: bytemuck::cast_slice(&inverse_view_proj_rot),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_proj_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: position_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inverse_view_proj_rot_buffer.as_entire_binding(),
                },
            ],
            label: Some("Camera Bind Group"),
        });

        CameraBinding {
            bind_group,
            view_proj_buffer,
            position_buffer,
            inverse_view_proj_rot_buffer,
        }
    }

    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Camera Bind Group Layout"),
        }
    }

    fn camera_to_matrices(
        cam: &Camera,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> CameraMatrices {
        let rot = Quaternion::from_angle_y(cam.rot_x) * Quaternion::from_angle_x(cam.rot_y);
        let eye_rotated = cgmath::Transform::transform_point(&cgmath::Matrix4::from(rot), cam.eye);
        let view = cgmath::Matrix4::look_at_rh(eye_rotated, cam.target, cam.up);
        let aspect = surface_config.width as f32 / surface_config.height as f32;
        let proj = cgmath::perspective(cgmath::Deg(cam.fovy), aspect, cam.znear, cam.zfar);
        let view_proj = wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;
        let m = view_proj;
        let m3 = Matrix3::new(
            m.x.x, m.x.y, m.x.z, m.y.x, m.y.y, m.y.z, m.z.x, m.z.y, m.z.z,
        )
        .invert()
        .unwrap();
        let inverse_view_proj_rot = Matrix4::new(
            m3.x.x, m3.x.y, m3.x.z, 0.0, m3.y.x, m3.y.y, m3.y.z, 0.0, m3.z.x, m3.z.y, m3.z.z, 0.0,
            0.0, 0.0, 0.0, 0.0,
        );
        CameraMatrices {
            view_proj: view_proj.into(),
            position: eye_rotated.into(),
            inverse_view_proj_rot: inverse_view_proj_rot.into(),
        }
    }

    pub fn update(
        &self,
        camera: &Camera,
        queue: &wgpu::Queue,
        surface_config: &wgpu::SurfaceConfiguration,
    ) {
        let matrices = CameraBinding::camera_to_matrices(camera, surface_config);
        queue.write_buffer(
            &self.view_proj_buffer,
            0,
            bytemuck::cast_slice(&matrices.view_proj),
        );
        queue.write_buffer(
            &self.position_buffer,
            0,
            bytemuck::cast_slice(&matrices.position),
        );
        queue.write_buffer(
            &self.inverse_view_proj_rot_buffer,
            0,
            bytemuck::cast_slice(&matrices.inverse_view_proj_rot),
        );
    }
}

pub struct EnvironmentMapBinding {
    pub prefiltered_view: wgpu::TextureView,
    pub di_view: wgpu::TextureView,
    pub brdf_view: wgpu::TextureView,
    pub prefiltered_sampler: wgpu::Sampler,
    pub di_sampler: wgpu::Sampler,
    pub brdf_sampler: wgpu::Sampler,
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
}

pub struct Layouts {
    pub camera: wgpu::BindGroupLayout,
    pub environment_map: wgpu::BindGroupLayout,
    pub sun: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
}
impl Layouts {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let camera = wgpu_context
            .device
            .create_bind_group_layout(&CameraBinding::desc());
        let environment_map = wgpu_context
            .device
            .create_bind_group_layout(&EnvironmentMapBinding::desc());
        let sun = wgpu_context
            .device
            .create_bind_group_layout(&SunBinding::desc());
        let material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());
        Self {
            camera,
            environment_map,
            sun,
            material,
        }
    }
}

pub struct SampledTexture {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

pub struct RenderResources {
    pub layouts: Layouts,
    pub models: HashMap<ModelHandle, ModelData>,
    pub materials: MaterialPool,
    pub textures: HashMap<TextureHandle, wgpu::Texture>,
    pub sampled_textures: HashMap<TextureHandle, SampledTexture>,
    pub camera: CameraBinding,
    pub sun: SunBinding,
    pub environment_maps: HashMap<EnvironmentMapHandle, EnvironmentMapBinding>,
}
impl RenderResources {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let layouts = Layouts::new(wgpu_context);
        let sun = Sun::default();
        let sun_binding = SunBinding::new(&sun, &wgpu_context.device, &layouts.sun);
        let camera = Camera::default();
        let camera_binding = CameraBinding::new(
            &camera,
            &wgpu_context.device,
            &wgpu_context.surface_config,
            &layouts.camera,
        );
        let this = RenderResources {
            layouts,
            models: HashMap::new(),
            materials: MaterialPool::new(),
            textures: HashMap::new(),
            sampled_textures: HashMap::new(),
            camera: camera_binding,
            sun: sun_binding,
            environment_maps: HashMap::new(),
        };
        this
    }

    pub fn load_scene_node(
        &mut self,
        scene: &crate::scene_tree::Scene,
        node_handle: &crate::scene_tree::NodeHandle,
        wgpu_context: &WgpuContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = &scene.nodes[node_handle];
        let crate::scene_tree::RenderDataType::Model(model_handle) = &node.render_data;
        self.load_model(model_handle.clone(), wgpu_context)?;
        for child_handle in &node.children {
            self.load_scene_node(scene, child_handle, wgpu_context)?;
        }
        Ok(())
    }

    pub fn load_scene(
        &mut self,
        scene: &crate::scene_tree::Scene,
        wgpu_context: &WgpuContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.load_environment_map(scene.environment.clone(), wgpu_context);
        self.load_scene_node(scene, &scene.root, wgpu_context)?;
        Ok(())
    }

    pub fn load_dds_texture(
        &mut self,
        handle: TextureHandle,
        array_layers: u32,
        wgpu_context: &WgpuContext,
    ) {
        let dds = dds::load_dds_raw(&handle.0);
        let format = dds::dds_format_to_wgpu(
            dds.get_dxgi_format()
                .expect("Dds doesn't have a DXGI format."),
        );
        let tex = dds::upload_texture(
            &dds.data,
            dds.get_width(),
            dds.get_height(),
            dds.get_num_mipmap_levels(),
            array_layers, // TODO this can't be read from dds? Bug in ddsfile?
            format,
            &wgpu_context.device,
            &wgpu_context.queue,
        );
        self.textures.insert(handle, tex);
    }

    pub fn load_png_texture(
        &mut self,
        handle: TextureHandle,
        srgb: bool,
        wgpu_context: &WgpuContext,
    ) {
        let img = png::load_png(&handle.0);
        let tex = png::upload_png(&img, srgb, &wgpu_context.device, &wgpu_context.queue);
        self.textures.insert(handle, tex);
    }

    pub fn load_sampled_texture(
        &mut self,
        json: &modelfile::SampledTexture,
        array_layers: u32,
        wgpu_context: &WgpuContext,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let handle = TextureHandle(json.source.clone());
        if self.sampled_textures.get(&handle).is_some() {
            return Ok(handle);
        }
        if self.textures.get(&handle).is_none() {
            self.load_dds_texture(handle.clone(), array_layers, wgpu_context);
        }
        let tex = self.textures.get(&handle).unwrap();
        let view = tex.create_view(&TextureViewDescriptor {
            label: None,
            format: None,
            dimension: None,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        let sampler_desc = json.sampler.to_wgpu_descriptor(None);
        let sampler = wgpu_context.device.create_sampler(&sampler_desc);

        self.sampled_textures
            .insert(handle.clone(), SampledTexture { view, sampler });

        Ok(handle)
    }

    pub fn load_material(
        &mut self,
        mat: &modelfile::Material,
        wgpu_context: &WgpuContext,
    ) -> Result<MaterialHandle, Box<dyn std::error::Error>> {
        let base_color_handle =
            self.load_sampled_texture(&mat.base_color_texture, 1, wgpu_context)?;
        let normal_sampler_handle =
            self.load_sampled_texture(&mat.normal_texture, 1, wgpu_context)?;
        let emissive_sampler_handle =
            self.load_sampled_texture(&mat.emissive_texture, 1, wgpu_context)?;
        let occlusion_sampler_handle =
            self.load_sampled_texture(&mat.occlusion_texture, 1, wgpu_context)?;
        let metallic_roughness_sampler_handle =
            self.load_sampled_texture(&mat.metallic_roughness_texture, 1, wgpu_context)?;

        let binding = MaterialBinding::upload(mat, self, wgpu_context);
        let handle = self.materials.insert(binding);

        Ok(handle)
    }

    pub fn load_environment_map(
        &mut self,
        handle: EnvironmentMapHandle,
        wgpu_context: &WgpuContext,
    ) {
        let (prefiltered_view, prefiltered_sampler) = {
            let handle = TextureHandle(handle.0.clone() + ".prefiltered.dds");
            self.load_dds_texture(handle.clone(), 6, wgpu_context);
            let texture = self.textures.get(&handle).unwrap();
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                array_layer_count: Some(6),
                mip_level_count: Some(texture.mip_level_count()),
                ..Default::default()
            });
            let sampler = wgpu_context
                .device
                .create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("Env Cubemap Sampler"),
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    lod_min_clamp: 0.0,
                    lod_max_clamp: 100.0,
                    compare: None,
                    anisotropy_clamp: 1,
                    border_color: None,
                });
            (view, sampler)
        };

        let (di_view, di_sampler) = {
            let handle = TextureHandle(handle.0.clone() + ".di.dds");
            self.load_dds_texture(handle.clone(), 6, wgpu_context);
            let texture = self.textures.get(&handle).unwrap();
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                array_layer_count: Some(6),
                mip_level_count: Some(texture.mip_level_count()),
                ..Default::default()
            });
            let sampler = wgpu_context
                .device
                .create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("Cubemap Sampler"),
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    lod_min_clamp: 0.0,
                    lod_max_clamp: 100.0,
                    compare: None,
                    anisotropy_clamp: 1,
                    border_color: None,
                });
            (view, sampler)
        };

        let (brdf_view, brdf_sampler) = {
            let handle = TextureHandle("assets/brdf_lut.png".to_string());
            self.load_png_texture(handle.clone(), false, wgpu_context);
            let texture = self.textures.get(&handle).unwrap();
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = wgpu_context
                .device
                .create_sampler(&SamplerDescriptor::default());
            (view, sampler)
        };

        let bind_group = wgpu_context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Environment Map Bind Group"),
                layout: &self.layouts.environment_map,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&prefiltered_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&prefiltered_sampler),
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

        let binding = EnvironmentMapBinding {
            prefiltered_view,
            di_view,
            brdf_view,
            prefiltered_sampler,
            di_sampler,
            brdf_sampler,
            bind_group,
        };

        self.environment_maps.insert(handle, binding);
    }

    pub fn load_model(
        &mut self,
        handle: ModelHandle,
        wgpu_context: &WgpuContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json_file = std::fs::File::open(&handle.0)?;
        let json_reader = std::io::BufReader::new(json_file);
        let model: modelfile::Model = serde_json::from_reader(json_reader)?;

        let index_vertex_data = std::fs::read(&model.buffer_path)?;
        let index_vertex_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index/vertex buffer"),
                    contents: bytemuck::cast_slice(&index_vertex_data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
                });

        let (instances, instance_counts) =
            mult_primitive_instances(&model, &vec![Matrix4::identity()]);
        let instance_buffer =
            wgpu_context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance buffer"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        let materials = {
            let mut mats = vec![];
            for mat in &model.materials {
                mats.push(self.load_material(mat, wgpu_context)?);
            }
            mats
        };

        self.models.insert(
            handle,
            ModelData {
                json: model,
                index_vertex_buffer,
                instance_buffer,
                primitive_instance_counts: instance_counts,
                materials,
            },
        );

        Ok(())
    }
}

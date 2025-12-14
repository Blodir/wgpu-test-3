use std::collections::HashMap;

use super::animation::AnimationClip;
use super::animationfile;
use super::dds;
use super::materialfile;
use super::modelfile;
use super::png;
use super::skeletonfile;
use super::skeletonfile::Skeleton;
use generational_arena::Index;
use glam::Mat3;
use glam::Mat4;
use glam::Quat;
use glam::Vec3;
use glam::Vec4;
use wgpu::{util::DeviceExt as _, SamplerDescriptor, TextureViewDescriptor};

use crate::{
    renderer::pipelines::model::{instance::Instance, material_binding::MaterialBinding},
    renderer::wgpu_context::{self, WgpuContext},
    scene_tree::{Camera, Sun},
};

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct ModelHandle(pub String);

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct SkeletonHandle(pub String);

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct TextureHandle(pub String);

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct MaterialHandle(u32);

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct AnimationHandle(pub String);

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

fn build_primitive_instances(
    model: &modelfile::Model,
    instance_data: &Vec<(Mat4, u32)>,
) -> (Vec<Instance>, Vec<u32>) {
    let mut instances: Vec<Instance> = vec![];
    let mut instance_counts = vec![];
    for prim in &model.primitives {
        let mut inst_count = 0;
        for instance in &prim.instances {
            let inst_m4 = Mat4::from_cols_array_2d(instance);
            for (transform, palette_offset) in instance_data {
                let t = transform * inst_m4;
                instances.push(Instance::new(t, *palette_offset));
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
    pub animations: Vec<AnimationHandle>,
    pub skeleton: SkeletonHandle,
}
impl ModelData {
    pub fn update_instance_buffer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instance_data: &Vec<(Mat4, u32)>,
    ) {
        let (instances, instance_counts) = build_primitive_instances(&self.json, instance_data);
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

pub struct LightsBinding {
    prefiltered_view: wgpu::TextureView,
    di_view: wgpu::TextureView,
    brdf_view: wgpu::TextureView,
    prefiltered_sampler: wgpu::Sampler,
    di_sampler: wgpu::Sampler,
    brdf_sampler: wgpu::Sampler,
    sun_direction_buffer: wgpu::Buffer,
    sun_color_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl LightsBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // sun dir
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
                // sun color
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
                // prefiltered
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
                // Diffuse irradiance
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
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
                // BRDF LUT
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Lights Group Layout"),
        }
    }

    pub fn update_sun(&self, sun: &Sun, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.sun_direction_buffer,
            0,
            bytemuck::cast_slice(&sun.direction),
        );
        queue.write_buffer(&self.sun_color_buffer, 0, bytemuck::cast_slice(&sun.color));
    }

    pub fn update_environment_map(
        &self,
        queue: &wgpu::Queue,
        prefiltered_texture: &wgpu::Texture,
        di_texture: &wgpu::Texture,
        brdf_texture: &wgpu::Texture,
    ) {
        todo!()
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
        let rot = Quat::from_rotation_y((cam.rot_x).to_radians())
            * Quat::from_rotation_x((cam.rot_y).to_radians());
        let eye_rotated: Vec3 = rot * cam.eye;
        let view = Mat4::look_at_rh(eye_rotated, cam.target, cam.up);

        let aspect = surface_config.width as f32 / surface_config.height as f32;
        // cam.fovy expected in radians (use cam.fovy.to_radians() if it’s degrees).
        let proj = Mat4::perspective_rh(cam.fovy, aspect, cam.znear, cam.zfar);

        let view_proj: Mat4 = wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;

        // Upper-left 3×3, inverted (no transpose here; cgmath code didn’t transpose either)
        let m3 = Mat3::from_mat4(view_proj).inverse();

        // Rebuild a 4×4 with zeroed last row/col (to match your cgmath layout exactly).
        let inverse_view_proj_rot = Mat4::from_cols(
            Vec4::new(m3.x_axis.x, m3.x_axis.y, m3.x_axis.z, 0.0),
            Vec4::new(m3.y_axis.x, m3.y_axis.y, m3.y_axis.z, 0.0),
            Vec4::new(m3.z_axis.x, m3.z_axis.y, m3.z_axis.z, 0.0),
            Vec4::ZERO,
        );

        CameraMatrices {
            view_proj: view_proj.to_cols_array_2d(),
            position: eye_rotated.to_array(),
            inverse_view_proj_rot: inverse_view_proj_rot.to_cols_array_2d(),
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

pub struct Layouts {
    pub camera: wgpu::BindGroupLayout,
    pub lights: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub bones: wgpu::BindGroupLayout,
}
impl Layouts {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let camera = wgpu_context
            .device
            .create_bind_group_layout(&CameraBinding::desc());
        let lights = wgpu_context
            .device
            .create_bind_group_layout(&LightsBinding::desc());
        let material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());
        let bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBinding::desc());
        Self {
            camera,
            lights,
            material,
            bones
        }
    }
}

pub struct SampledTexture {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoneMat34 {
    pub mat: [[f32; 4]; 3],
}
impl Default for BoneMat34 {
    fn default() -> Self {
        Self {
            mat: [
                [1f32, 0f32, 0f32, 0f32],
                [0f32, 1f32, 0f32, 0f32],
                [0f32, 0f32, 1f32, 0f32],
            ]
        }
    }
}

pub struct BonesBinding {
    pub bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
}
impl BonesBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Bones Bind Group Layout"),
        }
    }
    pub fn new(layout: &wgpu::BindGroupLayout, device: &wgpu::Device) -> Self {
        let data: Vec<BoneMat34> = vec![BoneMat34::default(); 1024];
        // TODO allocate extra space
        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bones SSBO"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bones Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }]
        });
        Self {
            bind_group,
            buffer: storage_buffer,
        }
    }
    pub fn update(&mut self, data: Vec<BoneMat34>, queue: &wgpu::Queue) {
        // TODO check if there's enough space?
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&data));
    }
}

pub struct RenderResources {
    pub layouts: Layouts,
    pub models: HashMap<ModelHandle, ModelData>,
    pub skeletons: HashMap<SkeletonHandle, skeletonfile::Skeleton>,
    pub materials: MaterialPool,
    pub animations: HashMap<AnimationHandle, AnimationClip>,
    pub textures: HashMap<TextureHandle, wgpu::Texture>,
    pub sampled_textures: HashMap<TextureHandle, SampledTexture>,
    pub camera: CameraBinding,
    pub lights: Option<LightsBinding>,
    pub bones: BonesBinding,
}
impl RenderResources {
    const NORMALS_PLACEHOLDER: &str = "NORMALS_PLACEHOLDER";
    const OCCLUSION_PLACEHOLDER: &str = "OCCLUSION_PLACEHOLDER";
    const BASE_COLOR_PLACEHOLDER: &str = "BASE_COLOR_PLACEHOLDER";
    const EMISSIVE_PLACEHOLDER: &str = "EMISSIVE_PLACEHOLDER";
    const METALLIC_ROUGHNESS_PLACEHOLDER: &str = "METALLIC_ROUGHNESS_PLACEHOLDER";

    fn initialize_placeholder_textures(
        &mut self,
        wgpu_context: &WgpuContext,
    ) {
        let extent = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let base_color_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Base color placeholder"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let metallic_roughness_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Metallic-roughness placeholder"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let normals_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Normals placeholder"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let occlusion_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Occlusion placeholder"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let emissive_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Emissive placeholder"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let base_color_ict = wgpu::ImageCopyTexture {
            texture: &base_color_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        let metallic_roughness_ict = wgpu::ImageCopyTexture {
            texture: &metallic_roughness_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        let normals_ict = wgpu::ImageCopyTexture {
            texture: &normals_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        let occlusion_ict = wgpu::ImageCopyTexture {
            texture: &occlusion_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        let emissive_ict = wgpu::ImageCopyTexture {
            texture: &emissive_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        wgpu_context.queue.write_texture(
            base_color_ict,
            &bytemuck::cast_slice(&[1u16, 1u16, 1u16, 1u16]).to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            extent,
        );
        wgpu_context.queue.write_texture(
            metallic_roughness_ict,
            &bytemuck::cast_slice(&[0x0000u16, 0x3800u16, 0x0000u16, 0x3C00u16]).to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            extent,
        );
        wgpu_context.queue.write_texture(
            normals_ict,
            &bytemuck::cast_slice(&[0x0000u16, 0x0000u16, 0x3C00u16, 0x3C00u16]).to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            extent,
        );
        wgpu_context.queue.write_texture(
            occlusion_ict,
            &vec![u8::MAX],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            extent,
        );
        wgpu_context.queue.write_texture(
            emissive_ict,
            &vec![0u8],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            extent,
        );
        let base_color = SampledTexture {
            view: base_color_texture.create_view(&TextureViewDescriptor::default()),
            sampler: wgpu_context.device.create_sampler(&wgpu::SamplerDescriptor::default())
        };
        let metallic_roughness = SampledTexture {
            view: metallic_roughness_texture.create_view(&TextureViewDescriptor::default()),
            sampler: wgpu_context.device.create_sampler(&wgpu::SamplerDescriptor::default())
        };
        let normals = SampledTexture {
            view: normals_texture.create_view(&TextureViewDescriptor::default()),
            sampler: wgpu_context.device.create_sampler(&wgpu::SamplerDescriptor::default())
        };
        let occlusion = SampledTexture {
            view: occlusion_texture.create_view(&TextureViewDescriptor::default()),
            sampler: wgpu_context.device.create_sampler(&wgpu::SamplerDescriptor::default())
        };
        let emissive = SampledTexture {
            view: emissive_texture.create_view(&TextureViewDescriptor::default()),
            sampler: wgpu_context.device.create_sampler(&wgpu::SamplerDescriptor::default())
        };

        self.sampled_textures.insert(Self::get_base_color_placeholder(), base_color);
        self.sampled_textures.insert(Self::get_metallic_roughness_placeholder(), metallic_roughness);
        self.sampled_textures.insert(Self::get_normals_placeholder(), normals);
        self.sampled_textures.insert(Self::get_occlusion_placeholder(), occlusion);
        self.sampled_textures.insert(Self::get_emissive_placeholder(), emissive);
    }

    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let layouts = Layouts::new(wgpu_context);
        let camera = Camera::default();
        let camera_binding = CameraBinding::new(
            &camera,
            &wgpu_context.device,
            &wgpu_context.surface_config,
            &layouts.camera,
        );
        let bones = BonesBinding::new(&layouts.bones, &wgpu_context.device);
        let mut this = RenderResources {
            layouts,
            models: HashMap::new(),
            skeletons: HashMap::new(),
            materials: MaterialPool::new(),
            animations: HashMap::new(),
            textures: HashMap::new(),
            sampled_textures: HashMap::new(),
            camera: camera_binding,
            lights: None,
            bones,
        };
        this.initialize_placeholder_textures(wgpu_context);
        this
    }

    pub fn load_lights(
        &mut self,
        wgpu_context: &WgpuContext,
        sun: Sun,
        environment_map: EnvironmentMapHandle,
    ) {
        let direction_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Direction Buffer"),
            contents: bytemuck::cast_slice(&sun.direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let color_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Color Buffer"),
            contents: bytemuck::cast_slice(&sun.color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let prefiltered_handle = TextureHandle(environment_map.0.clone() + ".prefiltered.dds");
        self.load_dds_texture(prefiltered_handle.clone(), 6, wgpu_context);
        let prefiltered_texture = self.textures.get(&prefiltered_handle).unwrap();

        let prefiltered_view = prefiltered_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            array_layer_count: Some(6),
            mip_level_count: Some(prefiltered_texture.mip_level_count()),
            ..Default::default()
        });
        let prefiltered_sampler = wgpu_context.device.create_sampler(&wgpu::SamplerDescriptor {
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

        let di_handle = TextureHandle(environment_map.0.clone() + ".di.dds");
        self.load_dds_texture(di_handle.clone(), 6, wgpu_context);
        let di_texture = self.textures.get(&di_handle).unwrap();

        let di_view = di_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            array_layer_count: Some(6),
            mip_level_count: Some(di_texture.mip_level_count()),
            ..Default::default()
        });
        let di_sampler = wgpu_context.device.create_sampler(&wgpu::SamplerDescriptor {
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

        let brdf_handle = TextureHandle("assets/brdf_lut.png".to_string());
        self.load_png_texture(brdf_handle.clone(), false, wgpu_context);
        let brdf_texture = self.textures.get(&brdf_handle).unwrap();

        let brdf_view = brdf_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let brdf_sampler = wgpu_context.device.create_sampler(&SamplerDescriptor::default());

        let bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.layouts.lights,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: direction_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&prefiltered_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&prefiltered_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&di_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&di_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&brdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&brdf_sampler),
                },
            ],
            label: Some("Lights Bind Group"),
        });

        self.lights = Some(LightsBinding {
            bind_group,
            sun_direction_buffer: direction_buffer,
            sun_color_buffer: color_buffer,
            prefiltered_view,
            di_view,
            brdf_view,
            prefiltered_sampler,
            di_sampler,
            brdf_sampler,
        });
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
        json: &materialfile::SampledTexture,
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

    pub fn get_base_color_placeholder() -> TextureHandle {
        TextureHandle(Self::BASE_COLOR_PLACEHOLDER.to_string())
    }

    pub fn get_normals_placeholder() -> TextureHandle {
        TextureHandle(Self::NORMALS_PLACEHOLDER.to_string())
    }

    pub fn get_emissive_placeholder() -> TextureHandle {
        TextureHandle(Self::EMISSIVE_PLACEHOLDER.to_string())
    }

    pub fn get_occlusion_placeholder() -> TextureHandle {
        TextureHandle(Self::OCCLUSION_PLACEHOLDER.to_string())
    }

    pub fn get_metallic_roughness_placeholder() -> TextureHandle {
        TextureHandle(Self::METALLIC_ROUGHNESS_PLACEHOLDER.to_string())
    }

    pub fn load_material(
        &mut self,
        mat: &materialfile::Material,
        wgpu_context: &WgpuContext,
    ) -> Result<MaterialHandle, Box<dyn std::error::Error>> {
        let base_color_handle =
            &mat.base_color_texture.as_ref()
            .map(|s| self.load_sampled_texture(s, 1, wgpu_context))
            .unwrap_or(Ok(Self::get_base_color_placeholder()))?;
        let normal_sampler_handle =
            &mat.normal_texture.as_ref()
            .map(|s| self.load_sampled_texture(s, 1, wgpu_context))
            .unwrap_or(Ok(Self::get_normals_placeholder()))?;
        let emissive_sampler_handle =
            &mat.emissive_texture.as_ref()
            .map(|s| self.load_sampled_texture(s, 1, wgpu_context))
            .unwrap_or(Ok(Self::get_emissive_placeholder()))?;
        let occlusion_sampler_handle =
            &mat.occlusion_texture.as_ref()
            .map(|s| self.load_sampled_texture(s, 1, wgpu_context))
            .unwrap_or(Ok(Self::get_occlusion_placeholder()))?;
        let metallic_roughness_sampler_handle =
            &mat.metallic_roughness_texture.as_ref()
            .map(|s| self.load_sampled_texture(s, 1, wgpu_context))
            .unwrap_or(Ok(Self::get_metallic_roughness_placeholder()))?;

        let binding = MaterialBinding::upload(mat, self, wgpu_context);
        let handle = self.materials.insert(binding);

        Ok(handle)
    }

    pub fn load_skeleton(
        &mut self,
        handle: SkeletonHandle,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json_file = std::fs::File::open(&handle.0)?;
        let json_reader = std::io::BufReader::new(json_file);
        let skeleton: skeletonfile::Skeleton = serde_json::from_reader(json_reader)?;
        self.skeletons.insert(handle, skeleton);
        Ok(())
    }

    pub fn load_animation(
        &mut self,
        path: &str,
    ) -> Result<AnimationHandle, Box<dyn std::error::Error>> {
        let handle = AnimationHandle(path.to_string());
        if self.animations.get(&handle).is_some() {
            return Ok(handle);
        }

        let json_file = std::fs::File::open(&handle.0)?;
        let json_reader = std::io::BufReader::new(json_file);
        let header: animationfile::AnimationClip = serde_json::from_reader(json_reader)?;
        let bytes = std::fs::read(header.binary_path)?;

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
        let tracks: Vec<super::animation::Track> = header.tracks.iter().map(|track| {
            let target = track.target;
            let shared_times = track.shared_times.as_ref().map(read_f32_ref);
            let translation = track.translation.as_ref().map(|s| {
                let interpolation = s.interpolation;
                let times = s.times.as_ref().map(read_f32_ref);
                let values = read_vec3_ref(&s.values);
                super::animation::Channel::<Vec3> {
                    interpolation, times, values
                }
            });
            let rotation = track.rotation.as_ref().map(|s| {
                let interpolation = s.interpolation;
                let times = s.times.as_ref().map(read_f32_ref);
                let values = read_quat_ref(&s.values);
                super::animation::Channel::<Quat> {
                    interpolation, times, values
                }
            });
            let scale = track.scale.as_ref().map(|s| {
                let interpolation = s.interpolation;
                let times = s.times.as_ref().map(read_f32_ref);
                let values = read_vec3_ref(&s.values);
                super::animation::Channel::<Vec3> {
                    interpolation, times, values
                }
            });
            super::animation::Track {
                target, shared_times, translation, rotation, scale
            }
        }).collect();

        let animation = super::animation::AnimationClip {
            duration,
            tracks,
            primitive_groups,
        };

        self.animations.insert(handle.clone(), animation);
        Ok(handle)
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

        let (instances, instance_counts) = build_primitive_instances(&model, &vec![(Mat4::IDENTITY, 0u32)]);
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
            for mat in &model.material_paths {
                mats.push(self.load_material(mat, wgpu_context)?);
            }
            mats
        };

        let skeleton_handle = SkeletonHandle(model.skeletonfile_path.clone());
        if !self.skeletons.contains_key(&skeleton_handle) {
            self.load_skeleton(skeleton_handle.clone())?;
        }

        let animations = {
            let mut anims = vec![];
            for path in &model.animations {
                anims.push(self.load_animation(path)?);
            }
            anims
        };

        self.models.insert(
            handle,
            ModelData {
                json: model,
                index_vertex_buffer,
                instance_buffer,
                primitive_instance_counts: instance_counts,
                materials,
                skeleton: skeleton_handle,
                animations
            },
        );

        Ok(())
    }
}

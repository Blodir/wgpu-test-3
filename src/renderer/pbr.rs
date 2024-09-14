use std::{fs::File, io::Read, mem::size_of};

use cgmath::{Matrix, Matrix3, Matrix4, SquareMatrix, Transform};
use wgpu::util::DeviceExt;

use super::texture::Texture;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    m4: [[f32; 4]; 4],
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            m4: Matrix4::identity().into(),
        }
    }
}

impl Instance {
    const BASE_SHADER_LOCATION: u32 = 0;
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: Self::BASE_SHADER_LOCATION + 0,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 4]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 2,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: size_of::<[f32; 12]>() as wgpu::BufferAddress,
            shader_location: Self::BASE_SHADER_LOCATION + 3,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub fn from(mat4: Matrix4<f32>) -> Self {
        Self {
            m4: mat4.into(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub tangent: [f32; 4],
    pub weights: [f32; 4],
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub normal_tex_coords: [f32; 2],
    pub occlusion_tex_coords: [f32; 2],
    pub emissive_tex_coords: [f32; 2],
    pub base_color_tex_coords: [f32; 2],
    pub metallic_roughness_tex_coords: [f32; 2],
    pub joints: [u8; 4],
    // TODO add padding for alignment
}

impl Default for Vertex {
    fn default() -> Self {
        Vertex {
            tangent: [1.0, 0.0, 0.0, 1.0],
            weights: [1.0, 0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            normal_tex_coords: [0.0, 0.0],
            occlusion_tex_coords: [0.0, 0.0],
            emissive_tex_coords: [0.0, 0.0],
            base_color_tex_coords: [0.0, 0.0],
            metallic_roughness_tex_coords: [0.0, 0.0],
            joints: [0, 0, 0, 0],
        }
    }
}

impl Vertex {
    const BASE_SHADER_LOCATION: u32 = 4;
    const OFFSET_TAN: wgpu::BufferAddress = 0;
    const OFFSET_WEI: wgpu::BufferAddress = Self::OFFSET_TAN + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    const OFFSET_POS: wgpu::BufferAddress = Self::OFFSET_WEI + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    const OFFSET_NOR: wgpu::BufferAddress = Self::OFFSET_POS + size_of::<[f32; 3]>() as wgpu::BufferAddress;
    const OFFSET_NTC: wgpu::BufferAddress = Self::OFFSET_NOR + size_of::<[f32; 3]>() as wgpu::BufferAddress;
    //const OFFSET_OCC: wgpu::BufferAddress = Self::OFFSET_NTC + size_of::<[f32; 2]>() as wgpu::BufferAddress;
    // optimization: combining normal tex coords and occlusion tex coords
    const OFFSET_EMI: wgpu::BufferAddress = Self::OFFSET_NTC + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    //const OFFSET_BAS: wgpu::BufferAddress = Self::OFFSET_EMI + size_of::<[f32; 2]>() as wgpu::BufferAddress;
    // optimization: combining emissive and base color tex coords
    const OFFSET_MET: wgpu::BufferAddress = Self::OFFSET_EMI + size_of::<[f32; 4]>() as wgpu::BufferAddress;
    const OFFSET_JOI: wgpu::BufferAddress = Self::OFFSET_MET + size_of::<[f32; 2]>() as wgpu::BufferAddress;
    const ATTRIBUTES: [wgpu::VertexAttribute; 8] = [
        // 16 byte fields are first for better data alignment
        // I have not tested if this actually matters
        // at least need to add padding first for data alignment to matter
        wgpu::VertexAttribute {
            offset: Self::OFFSET_TAN,
            shader_location: Self::BASE_SHADER_LOCATION + 0,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_WEI,
            shader_location: Self::BASE_SHADER_LOCATION + 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_POS,
            shader_location: Self::BASE_SHADER_LOCATION + 2,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_NOR,
            shader_location: Self::BASE_SHADER_LOCATION + 3,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_NTC,
            shader_location: Self::BASE_SHADER_LOCATION + 4,
            // optimization: combining normal tex coords and occlusion tex coords
            format: wgpu::VertexFormat::Float32x4,
        },
        /*
        wgpu::VertexAttribute {
            offset: Self::OFFSET_OCC,
            shader_location: Self::BASE_SHADER_LOCATION + 5,
            format: wgpu::VertexFormat::Float32x2,
        },
        */
        wgpu::VertexAttribute {
            offset: Self::OFFSET_EMI,
            shader_location: Self::BASE_SHADER_LOCATION + 5,
            // optimization: combining emissive base color tex coords
            format: wgpu::VertexFormat::Float32x4,
        },
        /*
        wgpu::VertexAttribute {
            offset: Self::OFFSET_BAS,
            shader_location: Self::BASE_SHADER_LOCATION + 6,
            format: wgpu::VertexFormat::Float32x2,
        },
        */
        wgpu::VertexAttribute {
            offset: Self::OFFSET_MET,
            shader_location: Self::BASE_SHADER_LOCATION + 6,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: Self::OFFSET_JOI,
            shader_location: Self::BASE_SHADER_LOCATION + 7,
            format: wgpu::VertexFormat::Uint8x4,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub struct Material {
    pub base_color_factor: [f32; 4],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub emissive_factor: [f32; 3],
    pub normal_texture: (image::DynamicImage, Option<SamplerOptions>),
    pub occlusion_texture: (image::DynamicImage, Option<SamplerOptions>),
    pub emissive_texture: (image::DynamicImage, Option<SamplerOptions>),
    pub base_color_texture: (image::DynamicImage, Option<SamplerOptions>),
    pub metallic_roughness_texture: (image::DynamicImage, Option<SamplerOptions>),
}

pub struct SamplerOptions {
    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub mag_filter: wgpu::FilterMode,
    pub min_filter: wgpu::FilterMode,
}

impl SamplerOptions {
    pub fn to_sampler_descriptor(&self) -> wgpu::SamplerDescriptor {
        wgpu::SamplerDescriptor {
            address_mode_u: self.address_mode_u,
            address_mode_v: self.address_mode_v,
            mag_filter: self.mag_filter,
            min_filter: self.min_filter,
            ..wgpu::SamplerDescriptor::default()
        }
    }
}

impl Default for Material {
    fn default() -> Self {
        let mut img = image::RgbaImage::new(1, 1);
        for px in img.pixels_mut() {
            *px = image::Rgba([255, 255, 255, 255]);
        }
        let default_texture = image::DynamicImage::from(img);

        let mut img2 = image::RgbaImage::new(1, 1);
        for px in img2.pixels_mut() {
            *px = image::Rgba([255, 255, 255, 0]);
        }
        let default_normals = image::DynamicImage::from(img2);

        Material {
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            emissive_factor: [0.0, 0.0, 0.0],
            normal_texture: (default_normals, None),
            occlusion_texture: (default_texture.clone(), None),
            emissive_texture: (default_texture.clone(), None),
            base_color_texture: (default_texture.clone(), None),
            metallic_roughness_texture: (default_texture, None),
        }
    }
}

pub struct MaterialBinding {
    pub bind_group: wgpu::BindGroup,
    base_color_factor: wgpu::Buffer,
    metallic_factor: wgpu::Buffer,
    roughness_factor: wgpu::Buffer,
    emissive_factor: wgpu::Buffer,
    normal_texture: Texture,
    occlusion_texture: Texture,
    emissive_texture: Texture,
    base_color_texture: Texture,
    metallic_roughness_texture: Texture,
}
impl Material {
    fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // base color factor
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
                // metallic factor
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
                // roughness factor
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // emissive factor
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                // normal texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // occlusion texture
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                // occlusion texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // emissive texture
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                // emissive texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // base color texture
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                // base color texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 11,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // metallic roughness texture
                wgpu::BindGroupLayoutEntry {
                    binding: 12,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                },
                // metallic roughness texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 13,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Material Bind Group Layout"),
        }
    }

    fn upload(
        &self, device: &wgpu::Device, queue: &wgpu::Queue,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> MaterialBinding {
        let base_color_factor = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Base Color Factor Buffer"),
                contents: bytemuck::cast_slice(&self.base_color_factor),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let metallic_factor = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Metallic Factor Buffer"),
                contents: bytemuck::cast_slice(&[self.metallic_factor]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let roughness_factor = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Roughness Factor Buffer"),
                contents: bytemuck::cast_slice(&[self.roughness_factor]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let emissive_factor = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Emissive Factor Buffer"),
                contents: bytemuck::cast_slice(&self.emissive_factor),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let normal_texture = Texture::from_image(device, queue, &self.normal_texture);
        let occlusion_texture = Texture::from_image(device, queue, &self.occlusion_texture);
        let emissive_texture = Texture::from_image(device, queue, &self.emissive_texture);
        let base_color_texture = Texture::from_image(device, queue, &self.base_color_texture);
        let metallic_roughness_texture = Texture::from_image(device, queue, &self.metallic_roughness_texture);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: material_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: base_color_factor.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: metallic_factor.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: roughness_factor.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: emissive_factor.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&occlusion_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&occlusion_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&emissive_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&emissive_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&base_color_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::Sampler(&base_color_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&metallic_roughness_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::Sampler(&metallic_roughness_texture.sampler),
                },
            ],
            label: Some("Material Bind Group"),
        });
        MaterialBinding {
            bind_group,
            base_color_factor,
            metallic_factor,
            roughness_factor,
            emissive_factor,
            normal_texture,
            occlusion_texture,
            emissive_texture,
            base_color_texture,
            metallic_roughness_texture,
        }
    }
}

pub enum VertexIndices {
    //U8(Vec<u8>), wgpu does not allow u8s while gltf does (i think?)
    U16(Vec<u16>),
    U32(Vec<u32>),
}

pub struct Primitive {
    pub vertices: Vec<Vertex>,
    pub material: Material,
    pub indices: VertexIndices,
}

pub struct PrimitiveBinding {
    pub vertex_buffer: wgpu::Buffer,
    pub material_binding: MaterialBinding,
    pub index_buffer: wgpu::Buffer,
    pub index_format: wgpu::IndexFormat,
    pub index_count: u32,
}

impl Default for Primitive {
    fn default() -> Self {
        let mut p1 = Vertex::default();
        p1.position = [0., 0., 0.];
        let mut p2 = Vertex::default();
        p2.position = [1., 0., 0.];
        let mut p3 = Vertex::default();
        p3.position = [0., 1., 0.];

        let indices = VertexIndices::U16(vec![0, 1, 2]);
        let material = Material::default();
        Self {
            vertices: vec![p1, p2, p3],
            indices,
            material,
        }
    }
}

impl Primitive {
    pub fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue, material_bind_group_layout: &wgpu::BindGroupLayout) -> PrimitiveBinding {
        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&self.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        let material_binding = self.material.upload(device, queue, material_bind_group_layout);
        let (indices, index_format, index_count) = match self.indices {
            VertexIndices::U16(ref v) => {
                (bytemuck::cast_slice(v), wgpu::IndexFormat::Uint16, v.len() as u32)
            },
            VertexIndices::U32(ref v) => {
                (bytemuck::cast_slice(v), wgpu::IndexFormat::Uint32, v.len() as u32)
            },
        };
        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: indices,
                usage: wgpu::BufferUsages::INDEX,
            }
        );

        PrimitiveBinding { vertex_buffer, material_binding, index_buffer, index_format, index_count }
    }
}

pub struct Mesh {
    pub primitives: Vec<Primitive>,
    pub instances: Vec<Instance>,
}

pub struct MeshBinding {
    pub primitives: Vec<PrimitiveBinding>,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            primitives: vec![Primitive::default()],
            instances: vec![Instance::default()],
        }
    }
}

impl Mesh {
    pub fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue, material_bind_group_layout: &wgpu::BindGroupLayout) -> MeshBinding {
        let instance_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&self.instances),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        let primitives = self.primitives.iter().map(|primitive| {
            primitive.upload(device, queue, material_bind_group_layout)
        }).collect();
        MeshBinding { primitives, instance_buffer, instance_count: self.instances.len() as u32 }
    }
}

pub struct MaterialPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
}

impl MaterialPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        diffuse_irradiance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let material_bind_group_layout = device.create_bind_group_layout(&Material::desc());
        let render_pipeline = Self::build_pipeline(device, surface_config, camera_bind_group_layout, lights_bind_group_layout, &material_bind_group_layout, diffuse_irradiance_bind_group_layout);

        Self { render_pipeline, material_bind_group_layout }
    }

    pub fn rebuild_pipeline(
        &mut self,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        diffuse_irradiance_bind_group_layout: &wgpu::BindGroupLayout,
    ) {
        self.render_pipeline = Self::build_pipeline(device, surface_config, camera_bind_group_layout, lights_bind_group_layout, &self.material_bind_group_layout, diffuse_irradiance_bind_group_layout);
    }
    
    pub fn build_pipeline(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lights_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        diffuse_irradiance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[Instance::desc(), Vertex::desc()];
        let bind_group_layouts = &[camera_bind_group_layout, lights_bind_group_layout, material_bind_group_layout, diffuse_irradiance_bind_group_layout];
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts,
            push_constant_ranges: &[],
        });
        let shader_module = super::utils::create_shader_module(device, "src/renderer/shaders/pbr.wgsl");
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: vertex_buffer_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                // TODO gltf may have different topologies
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                // TODO should get from depth texture
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }
}


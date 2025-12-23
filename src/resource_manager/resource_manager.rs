use std::{collections::{HashMap, VecDeque}, fs::File, io::Read as _, ops::Range, sync::{Arc, Mutex}};

use ddsfile::{Caps2, Dds};
use generational_arena::{Arena, Index};
use glam::{Quat, Vec3};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::renderer::{pipelines::model::{material_binding::MaterialBinding, vertex::Vertex}, render_resources::{animation, animationfile, dds, materialfile, modelfile, skeletonfile}, wgpu_context::{self, WgpuContext}, Layouts};

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ResourceKind {
    Model,
    Mesh,
    Material,
    Skeleton,
    AnimationClip,
    Animation,
    Texture,
}

pub trait ResourceTag {
    const KIND: ResourceKind;
}

/// Non-owning reference to a registry entry, the entry is not guaranteed to be present
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct HandleId<T: ResourceTag> {
    idx: generational_arena::Index,
    _marker: std::marker::PhantomData<T>,
}

/// Owned/refcounted reference to a registry entry
pub struct Handle<T: ResourceTag> {
    idx: generational_arena::Index,
    manager: std::sync::Weak<ResourceManager>,
    _marker: std::marker::PhantomData<T>,
}
impl<T: ResourceTag> Handle<T> {
    pub fn new(idx: generational_arena::Index, resource_manager_arc: &std::sync::Arc<ResourceManager>) -> Self {
        Self {
            idx,
            manager: Arc::downgrade(resource_manager_arc),
            _marker: std::marker::PhantomData,
        }
    }
    pub fn id(&self) -> HandleId<T> {
        HandleId {
            idx: self.idx,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: ResourceTag> Clone for Handle<T> {
    fn clone(&self) -> Self {
        if let Some(manager) = self.manager.upgrade() {
            manager.inc_ref(self.idx, T::KIND);
        }
        Self {
            idx: self.idx,
            manager: self.manager.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: ResourceTag> Drop for Handle<T> {
    fn drop(&mut self) {
        if let Some(manager) = self.manager.upgrade() {
            manager.dec_ref(self.idx, T::KIND);
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Model;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Mesh;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Material;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Skeleton;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _AnimationClip;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Animation;
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct _Texture;

impl ResourceTag for _Model { const KIND: ResourceKind = ResourceKind::Model; }
impl ResourceTag for _Mesh { const KIND: ResourceKind = ResourceKind::Mesh; }
impl ResourceTag for _Material { const KIND: ResourceKind = ResourceKind::Material; }
impl ResourceTag for _Skeleton { const KIND: ResourceKind = ResourceKind::Skeleton; }
impl ResourceTag for _AnimationClip { const KIND: ResourceKind = ResourceKind::AnimationClip; }
impl ResourceTag for _Animation { const KIND: ResourceKind = ResourceKind::Animation; }
impl ResourceTag for _Texture { const KIND: ResourceKind = ResourceKind::Texture; }

pub type ModelHandle = Handle<_Model>;
pub type MeshHandle = Handle<_Mesh>;
pub type MaterialHandle = Handle<_Material>;
pub type SkeletonHandle = Handle<_Skeleton>;
pub type AnimationClipHandle = Handle<_AnimationClip>;
pub type AnimationHandle = Handle<_Animation>;
pub type TextureHandle = Handle<_Texture>;

pub type ModelId = HandleId<_Model>;
pub type MeshId = HandleId<_Mesh>;
pub type MaterialId = HandleId<_Material>;
pub type SkeletonId = HandleId<_Skeleton>;
pub type AnimationClipId = HandleId<_AnimationClip>;
pub type AnimationId = HandleId<_Animation>;
pub type TextureId = HandleId<_Texture>;

pub enum CpuState {
    Absent, Loading, Ready(Index)
}

pub enum GpuState {
    Absent, Queued, Uploading(Index), Ready(Index)
}

struct Entry {
    pub kind: ResourceKind,
    ref_count: u32,
    pub cpu_state: CpuState,
    pub gpu_state: GpuState,
}
impl Entry {
    pub fn new(kind: ResourceKind) -> Self {
        Self {
            kind,
            ref_count: 0,
            cpu_state: CpuState::Absent,
            gpu_state: GpuState::Absent,
        }
    }
}

struct ResourceRegistry {
    entries: Arena<Entry>,
    by_path: HashMap<String, Index>,
}
impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            entries: Arena::new(),
            by_path: HashMap::new(),
        }
    }

    pub fn get<T: ResourceTag>(&self, handle: &Handle<T>) -> &Entry {
        self.entries.get(handle.idx).unwrap()
    }

    pub fn get_id<T: ResourceTag>(&self, id: &HandleId<T>) -> Option<&Entry> {
        self.entries.get(id.idx)
    }
}

struct SubMesh {
    pub instances: Vec<[[f32; 4]; 4]>,
    pub index_range: Range<u32>,
    pub base_vertex: u32,
    pub material: MaterialHandle,
}

struct ModelCpuData {
    pub manifest: modelfile::Model,
    pub mesh: MeshHandle,
    pub submeshes: Vec<SubMesh>,
    pub animations: Vec<AnimationClipHandle>,
    pub skeleton: SkeletonHandle,
}

struct MeshCpuData {
    pub index_vertex_data: Vec<u8>,
}

struct MaterialCpuData {
    pub manifest: materialfile::Material,
    pub normal_texture: Option<TextureHandle>,
    pub occlusion_texture: Option<TextureHandle>,
    pub emissive_texture: Option<TextureHandle>,
    pub base_color_texture: Option<TextureHandle>,
    pub metallic_roughness_texture: Option<TextureHandle>,
}

struct SkeletonCpuData {
    pub manifest: skeletonfile::Skeleton,
}

struct AnimationClipCpuData {
    pub manifest: animationfile::AnimationClip,
    pub animation: AnimationHandle,
}

type AnimationCpuData = animation::AnimationClip;

type TextureCpuData = TextureLoadData;

struct CpuResources {
    pub models: Mutex<Arena<ModelCpuData>>,
    pub meshes: Mutex<Arena<MeshCpuData>>,
    pub materials: Mutex<Arena<MaterialCpuData>>,
    pub skeletons: Mutex<Arena<SkeletonCpuData>>,
    pub animation_clips: Mutex<Arena<AnimationClipCpuData>>,
    pub animations: Mutex<Arena<AnimationCpuData>>,
    pub textures: Mutex<Arena<TextureCpuData>>,
}
impl CpuResources {
    pub fn new() -> Self {
        Self {
            models: Mutex::new(Arena::new()),
            meshes: Mutex::new(Arena::new()),
            materials: Mutex::new(Arena::new()),
            skeletons: Mutex::new(Arena::new()),
            animation_clips: Mutex::new(Arena::new()),
            animations: Mutex::new(Arena::new()),
            textures: Mutex::new(Arena::new()),
        }
    }
}

struct MeshGpuData {
    pub buffer: wgpu::Buffer,
}

struct TextureGpuData {
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
}

/// indices to GpuResources textures arena
pub struct PlaceholderTextureIds {
    normals: Index,
    base_color: Index,
    occlusion: Index,
    emissive: Index,
    metallic_roughness: Index,
    prefiltered: Index,
    di: Index,
    brdf: Index,
}

struct GpuResources {
    pub meshes: Mutex<Arena<MeshGpuData>>,
    pub materials: Mutex<Arena<MaterialBinding>>,
    pub textures: Mutex<Arena<TextureGpuData>>,
}
impl GpuResources {
    pub fn new() -> Self {
        let meshes = Mutex::new(Arena::new());
        let materials = Mutex::new(Arena::new());
        let textures = Mutex::new(Arena::new());

        Self {
            meshes,
            materials,
            textures,
        }
    }

    pub fn initialize_placeholders(&self, wgpu_context: &WgpuContext) -> PlaceholderTextureIds {
        let mut textures = self.textures.lock().unwrap();
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
        let prefiltered_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Prefiltered placeholder"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let di_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DI placeholder"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let brdf_texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("BRDF placeholder"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
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
        let prefiltered_ict = wgpu::ImageCopyTexture {
            texture: &prefiltered_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        let di_ict = wgpu::ImageCopyTexture {
            texture: &di_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        let brdf_ict = wgpu::ImageCopyTexture {
            texture: &brdf_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0, },
            aspect: wgpu::TextureAspect::All,
        };
        wgpu_context.queue.write_texture(
            base_color_ict,
            &bytemuck::cast_slice(&[1u16, 1u16, 1u16, 1u16]).to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(2*4),
                rows_per_image: Some(1),
            },
            extent,
        );
        wgpu_context.queue.write_texture(
            metallic_roughness_ict,
            &bytemuck::cast_slice(&[0x0000u16, 0x3800u16, 0x0000u16, 0x3C00u16]).to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(2*4),
                rows_per_image: Some(1),
            },
            extent,
        );
        wgpu_context.queue.write_texture(
            normals_ict,
            &bytemuck::cast_slice(&[0x0000u16, 0x0000u16, 0x3C00u16, 0x3C00u16]).to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(2*4),
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

        // One RGBA16F black texel
        let rgba16f_texel: [u16; 4] = [0x0000, 0x0000, 0x0000, 0x0000];
        // 6 faces
        let rgba16f_cube = [rgba16f_texel; 6];
        wgpu_context.queue.write_texture(
            prefiltered_ict,
            &bytemuck::cast_slice(&rgba16f_cube),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
        );
        wgpu_context.queue.write_texture(
            di_ict,
            &bytemuck::cast_slice(&rgba16f_cube),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            extent,
        );
        wgpu_context.queue.write_texture(
            brdf_ict,
            &bytemuck::cast_slice(&[0x39E1u16, 0x2404u16, 0x0000u16, 0x3C00u16]).to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            extent,
        );

        let normals_view = normals_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let base_color_view = base_color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let occlusion_view = occlusion_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let emissive_view = emissive_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let metallic_roughness_view = metallic_roughness_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let prefiltered_view = prefiltered_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let di_view = di_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let brdf_view = brdf_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let normals = textures.insert(TextureGpuData { texture: normals_texture, texture_view: normals_view });
        let base_color = textures.insert(TextureGpuData { texture: base_color_texture, texture_view: base_color_view });
        let occlusion = textures.insert(TextureGpuData { texture: occlusion_texture, texture_view: occlusion_view });
        let emissive = textures.insert(TextureGpuData { texture: emissive_texture, texture_view: emissive_view });
        let metallic_roughness = textures.insert(TextureGpuData { texture: metallic_roughness_texture, texture_view: metallic_roughness_view });
        let prefiltered = textures.insert(TextureGpuData { texture: prefiltered_texture, texture_view: prefiltered_view });
        let di = textures.insert(TextureGpuData { texture: di_texture, texture_view: di_view });
        let brdf = textures.insert(TextureGpuData { texture: brdf_texture, texture_view: brdf_view });


        PlaceholderTextureIds {
            normals,
            base_color,
            occlusion,
            emissive,
            metallic_roughness,
            prefiltered,
            di,
            brdf,
        }
    }
}

struct TextureLoadData {
    data: Vec<u8>,
    base_width: u32,
    base_height: u32,
    mips: u32,
    layers: u32,
    format: wgpu::TextureFormat,
}

enum IoRequest {
    LoadModel { id: Index, path: String },
    LoadMesh { id: Index, path: String },
    LoadMaterial { id: Index, path: String },
    LoadSkeleton { id: Index, path: String },
    LoadAnimationClip { id: Index, path: String },
    LoadAnimation { id: Index, path: String, header: animationfile::AnimationClip },
    LoadTexture { id: Index, path: String, srgb: bool },
}

enum IoResponse {
    ModelLoaded { id: Index, model: modelfile::Model },
    MeshLoaded { id: Index, data: Vec<u8> },
    MaterialLoaded { id: Index, material: materialfile::Material },
    SkeletonLoaded { id: Index, skeleton: skeletonfile::Skeleton },
    AnimationClipLoaded { id: Index, clip: animationfile::AnimationClip },
    AnimationLoaded { id: Index, parsed_clip: animation::AnimationClip },
    TextureLoaded { id: Index, data: TextureLoadData },
    Error { path: String, message: String },
}

fn load_json<T>(path: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: serde::de::DeserializeOwned,
{
    let json_file = std::fs::File::open(path)?;
    let json_reader = std::io::BufReader::new(json_file);
    let model: T = serde_json::from_reader(json_reader)?;
    Ok(model)
}

fn load_bin(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    Ok(bytes)
}

fn load_dds(bytes: &mut [u8]) -> TextureLoadData {
    let dds = Dds::read(&mut &bytes[..]).unwrap();

    let format = dds::dds_format_to_wgpu(
        dds.get_dxgi_format()
            .expect("Dds doesn't have a DXGI format."),
    );
    let is_cubemap = dds
        .header
        .caps2
        .contains(Caps2::CUBEMAP);
    let base_width = dds.get_width();
    let base_height = dds.get_height();
    let mips = dds.get_num_mipmap_levels();
    let layers = if is_cubemap { 6 } else { dds.get_num_array_layers() };

    TextureLoadData {
        data: dds.data,
        base_width,
        base_height,
        mips,
        layers,
        format,
    }
}

fn load_png(bytes: &mut [u8], srgb: bool) -> TextureLoadData {
    let img: image::DynamicImage = image::load_from_memory(&bytes).unwrap();
    let dimensions = image::GenericImageView::dimensions(&img);
    let (remapped, format): (Vec<u8>, wgpu::TextureFormat) = match (&img, srgb) {
        (image::DynamicImage::ImageRgb32F(_), false) => (
            bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba32Float,
        ),
        (image::DynamicImage::ImageRgba32F(_), false) => (
            bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba32Float,
        ),
        (_, true) => (
            bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ),
        (_, false) => (
            bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
    };
    let base_width = dimensions.0;
    let base_height = dimensions.1;
    let mips = 1;
    let layers = 1;

    TextureLoadData {
        data: remapped,
        base_width,
        base_height,
        mips,
        layers,
        format,
    }
}

fn load_texture(path: &str, srgb: bool) -> Result<TextureLoadData, Box<dyn std::error::Error>> {
    let mut file = File::open(path).unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();

    if bytes.starts_with(b"DDS ") {
        Ok(load_dds(&mut bytes))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Ok(load_png(&mut bytes, srgb))
    } else {
        Err("invalid texture format".into())
    }
}

pub fn load_animation(
    path: &str,
    header: animationfile::AnimationClip,
) -> Result<animation::AnimationClip, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;

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
    let tracks: Vec<animation::Track> = header.tracks.iter().map(|track| {
        let target = track.target;
        let shared_times = track.shared_times.as_ref().map(read_f32_ref);
        let translation = track.translation.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_vec3_ref(&s.values);
            animation::Channel::<Vec3> {
                interpolation, times, values
            }
        });
        let rotation = track.rotation.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_quat_ref(&s.values);
            animation::Channel::<Quat> {
                interpolation, times, values
            }
        });
        let scale = track.scale.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_vec3_ref(&s.values);
            animation::Channel::<Vec3> {
                interpolation, times, values
            }
        });
        animation::Track {
            target, shared_times, translation, rotation, scale
        }
    }).collect();

    Ok(animation::AnimationClip {
        duration,
        tracks,
        primitive_groups,
    })
}

fn io_worker_loop(
    rx: crossbeam::channel::Receiver<IoRequest>,
    tx: crossbeam::channel::Sender<IoResponse>,
) {
    while let Ok(req) = rx.recv() {
        let result = match req {
            IoRequest::LoadModel { id, path } => load_json::<modelfile::Model>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |model| IoResponse::ModelLoaded { id, model },
                ),
            IoRequest::LoadMesh { id, path } => load_bin(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::MeshLoaded { id, data },
                ),
            IoRequest::LoadMaterial { id, path } => load_json::<materialfile::Material>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |material| IoResponse::MaterialLoaded { id, material },
                ),
            IoRequest::LoadSkeleton { id, path } => load_json::<skeletonfile::Skeleton>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |skeleton| IoResponse::SkeletonLoaded { id, skeleton },
                ),
            IoRequest::LoadAnimationClip { id, path } => load_json::<animationfile::AnimationClip>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |clip| IoResponse::AnimationClipLoaded { id, clip },
                ),
            IoRequest::LoadAnimation { id, path, header } => load_animation(&path, header)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::AnimationLoaded { id, parsed_clip: data },
                ),
            IoRequest::LoadTexture { id, path, srgb } => load_texture(&path, srgb)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::TextureLoaded { id, data },
                ),
        };

        // ignore send errors on shutdown
        let _ = tx.send(result);
    }
}

struct IoManager {
    pub req_tx: crossbeam::channel::Sender<IoRequest>,
    pub res_rx: crossbeam::channel::Receiver<IoResponse>,
    workers: Vec<std::thread::JoinHandle<()>>,
}
impl IoManager {
    pub fn new() -> Self {
        let (req_tx, req_rx) = crossbeam::channel::unbounded();
        let (res_tx, res_rx) = crossbeam::channel::unbounded();

        let workers = (0..2)
            .map(|_| {
                let rx = req_rx.clone();
                let tx = res_tx.clone();
                std::thread::spawn(move || {
                    io_worker_loop(rx, tx);
                })
            })
            .collect();

        Self {
            req_tx, res_rx, workers
        }
    }
}

pub struct ResourceManager {
    pub registry: Mutex<ResourceRegistry>,
    pub gpu: GpuResources,
    pub cpu: CpuResources,
    io: IoManager,
    upload_queue: Mutex<VecDeque<Index>>,
}
impl ResourceManager {
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(ResourceRegistry::new()),
            gpu: GpuResources::new(),
            cpu: CpuResources::new(),
            io: IoManager::new(),
            upload_queue: Mutex::new(VecDeque::new()),
        }
    }

    fn inc_ref(&self, idx: Index, kind: ResourceKind) {
        let mut reg = self.registry.lock().unwrap();
        let Some(entry) = reg.entries.get_mut(idx) else {
            debug_assert!(false, "inc_ref on stale handle");
            return;
        };
        debug_assert_eq!(entry.kind, kind);
        entry.ref_count += 1;
    }

    fn dec_ref(&self, idx: Index, kind: ResourceKind) {
        let mut reg = self.registry.lock().unwrap();

        let entry = match reg.entries.get_mut(idx) {
            Some(e) => e,
            None => return,
        };

        debug_assert_eq!(entry.kind, kind);

        entry.ref_count = entry.ref_count.checked_sub(1)
            .expect("refcount underflow");
    }

    pub fn process_io_responses(
        self: &std::sync::Arc<Self>,
    ) {
        let mut reg = self.registry.lock().unwrap();
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
                    let entry = reg.entries.get_mut(id).unwrap();
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
                    let cpu_idx = self.cpu.models.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::MeshLoaded { id, data } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data = MeshCpuData { index_vertex_data: data };
                    let cpu_idx = self.cpu.meshes.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::MaterialLoaded { id, material } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let normal_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let occlusion_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, false));
                    let emissive_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let base_color_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let metallic_roughness_texture = material.normal_texture.as_ref().map(|t| self.request_texture(&t.source, true));
                    let cpu_data = MaterialCpuData { manifest: material, normal_texture, occlusion_texture, emissive_texture, base_color_texture, metallic_roughness_texture };
                    let cpu_idx = self.cpu.materials.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::SkeletonLoaded { id, skeleton } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data = SkeletonCpuData { manifest: skeleton };
                    let cpu_idx = self.cpu.skeletons.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::AnimationClipLoaded { id, clip } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let animation = self.request_animation(&clip.binary_path, &clip);
                    let cpu_data = AnimationClipCpuData { manifest: clip, animation };
                    let cpu_idx = self.cpu.animation_clips.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::AnimationLoaded { id, parsed_clip } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data: AnimationCpuData = parsed_clip;
                    let cpu_idx = self.cpu.animations.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                },
                IoResponse::TextureLoaded { id, data } => {
                    let entry = reg.entries.get_mut(id).unwrap();
                    let cpu_data: TextureCpuData = data;
                    let cpu_idx = self.cpu.textures.lock().unwrap().insert(cpu_data);
                    entry.cpu_state = CpuState::Ready(cpu_idx);
                    entry.gpu_state = GpuState::Queued;
                    self.upload_queue.lock().unwrap().push_back(id);
                },
                IoResponse::Error { path, message } => {
                    println!("IO Error: path: {}, message: {}", path, message);
                },
            }
        }
    }

    pub fn process_upload_queue(
        self: &std::sync::Arc<Self>,
        wgpu_context: &WgpuContext,
        layouts: &Layouts,
        placeholders: &PlaceholderTextureIds,
    ) {
        let mut reg = self.registry.lock().unwrap();
        let mut upload_queue = self.upload_queue.lock().unwrap();
        let mut meshes_cpu = self.cpu.meshes.lock().unwrap();
        let mut textures_cpu = self.cpu.textures.lock().unwrap();
        let mut materials_cpu = self.cpu.materials.lock().unwrap();
        let mut textures_gpu = self.gpu.textures.lock().unwrap();
        let mut materials_gpu = self.gpu.materials.lock().unwrap();
        let mut queue_next_frame: Vec<Index> = vec![];
        'upload_queue: while let Some(id) = upload_queue.pop_front() {
            let (entry_kind, entry_cpu_idx) = {
                let entry = reg.entries.get(id).unwrap();

                if entry.ref_count == 0 {
                    continue 'upload_queue; // cancelled
                }

                let entry_kind = entry.kind;
                let entry_cpu_idx = if let CpuState::Ready(cpu_idx) = entry.cpu_state {
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
                    let mut meshes_gpu = self.gpu.meshes.lock().unwrap();
                    let gpu_idx = meshes_gpu.insert(mesh_gpu);
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.gpu_state = GpuState::Ready(gpu_idx);
                    entry.cpu_state = CpuState::Absent;
                    meshes_cpu.remove(entry_cpu_idx);
                },
                ResourceKind::Texture => {
                    let texture_cpu = textures_cpu.get(entry_cpu_idx).unwrap();
                    // TODO move this function away from dds module, also it should probs just take the cpudata directly
                    let texture = dds::upload_texture(
                        &texture_cpu.data,
                        texture_cpu.base_width,
                        texture_cpu.base_height,
                        texture_cpu.mips,
                        texture_cpu.layers,
                        texture_cpu.format,
                        &wgpu_context.device,
                        &wgpu_context.queue
                    );
                    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let texture_gpu = TextureGpuData { texture, texture_view };
                    let mut textures_gpu = self.gpu.textures.lock().unwrap();
                    let gpu_idx = textures_gpu.insert(texture_gpu);
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.gpu_state = GpuState::Ready(gpu_idx);
                    entry.cpu_state = CpuState::Absent;
                    textures_cpu.remove(entry_cpu_idx);
                },
                ResourceKind::Material => {
                    let material_cpu = materials_cpu.get(entry_cpu_idx).unwrap();
                    // check if textures uploaded to gpu, re-schedule if not
                    let base_color_gpu_idx = {
                        if let Some(handle) = material_cpu.base_color_texture.as_ref() {
                            match reg.get(handle).gpu_state {
                                GpuState::Ready(gpu_idx) => {
                                    Some(gpu_idx)
                                },
                                _ => {
                                    queue_next_frame.push(id);
                                    continue 'upload_queue;
                                }
                            }
                        } else {
                            None
                        }
                    };
                    let emissive_gpu_idx = {
                        if let Some(handle) = material_cpu.emissive_texture.as_ref() {
                            match reg.get(handle).gpu_state {
                                GpuState::Ready(gpu_idx) => {
                                    Some(gpu_idx)
                                },
                                _ => {
                                    queue_next_frame.push(id);
                                    continue 'upload_queue;
                                }
                            }
                        } else {
                            None
                        }
                    };
                    let metallic_roughness_gpu_idx = {
                        if let Some(handle) = material_cpu.metallic_roughness_texture.as_ref() {
                            match reg.get(handle).gpu_state {
                                GpuState::Ready(gpu_idx) => {
                                    Some(gpu_idx)
                                },
                                _ => {
                                    queue_next_frame.push(id);
                                    continue 'upload_queue;
                                }
                            }
                        } else {
                            None
                        }
                    };
                    let normal_gpu_idx = {
                        if let Some(handle) = material_cpu.normal_texture.as_ref() {
                            match reg.get(handle).gpu_state {
                                GpuState::Ready(gpu_idx) => {
                                    Some(gpu_idx)
                                },
                                _ => {
                                    queue_next_frame.push(id);
                                    continue 'upload_queue;
                                }
                            }
                        } else {
                            None
                        }
                    };
                    let occlusion_gpu_idx = {
                        if let Some(handle) = material_cpu.occlusion_texture.as_ref() {
                            match reg.get(handle).gpu_state {
                                GpuState::Ready(gpu_idx) => {
                                    Some(gpu_idx)
                                },
                                _ => {
                                    queue_next_frame.push(id);
                                    continue 'upload_queue;
                                }
                            }
                        } else {
                            None
                        }
                    };
                    let base_color_view = &textures_gpu.get(base_color_gpu_idx.unwrap_or(placeholders.base_color)).unwrap().texture_view;
                    let emissive_view = &textures_gpu.get(emissive_gpu_idx.unwrap_or(placeholders.emissive)).unwrap().texture_view;
                    let metallic_roughness_view = &textures_gpu.get(metallic_roughness_gpu_idx.unwrap_or(placeholders.metallic_roughness)).unwrap().texture_view;
                    let normal_view = &textures_gpu.get(normal_gpu_idx.unwrap_or(placeholders.normals)).unwrap().texture_view;
                    let occlusion_view = &textures_gpu.get(occlusion_gpu_idx.unwrap_or(placeholders.occlusion)).unwrap().texture_view;
                    let material_binding = MaterialBinding::upload(&material_cpu.manifest, base_color_view, emissive_view, metallic_roughness_view, normal_view, occlusion_view, &layouts.material, wgpu_context);
                    let gpu_idx = materials_gpu.insert(material_binding);
                    let entry = reg.entries.get_mut(id).unwrap();
                    entry.gpu_state = GpuState::Ready(gpu_idx);
                    entry.cpu_state = CpuState::Absent;
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

    fn make_io_request(&self, req: IoRequest) {
        if self.io.req_tx.send(req).is_err() {
            todo!()
        }
    }

    pub fn request_model(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> ModelHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return ModelHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Model,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadModel { id: idx, path: path.to_string() });

        ModelHandle::new(idx, self)
    }

    pub fn request_mesh(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> MeshHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return MeshHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Mesh,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadMesh { id: idx, path: path.to_string() });

        MeshHandle::new(idx, self)
    }

    pub fn request_material(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> MaterialHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return MaterialHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Material,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadMaterial { id: idx, path: path.to_string() });

        MaterialHandle::new(idx, self)
    }

    pub fn request_skeleton(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> SkeletonHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return SkeletonHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Skeleton,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadModel { id: idx, path: path.to_string() });

        SkeletonHandle::new(idx, self)
    }

    pub fn request_animation_clip(
        self: &std::sync::Arc<Self>,
        path: &str,
    ) -> AnimationClipHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return AnimationClipHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::AnimationClip,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadAnimationClip { id: idx, path: path.to_string() });

        AnimationClipHandle::new(idx, self)
    }

    fn request_animation(
        self: &std::sync::Arc<Self>,
        path: &str,
        header: &animationfile::AnimationClip,
    ) -> AnimationHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return AnimationHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Animation,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        // TODO cloning header sounds pretty bad
        self.make_io_request(IoRequest::LoadAnimation { id: idx, path: path.to_string(), header: header.clone() });

        AnimationHandle::new(idx, self)
    }

    pub fn request_texture(
        self: &std::sync::Arc<Self>,
        path: &str,
        srgb: bool,
    ) -> TextureHandle {
        let mut reg = self.registry.lock().unwrap();
        if let Some(&idx) = reg.by_path.get(path) {
            let entry = reg.entries.get_mut(idx).unwrap();
            entry.ref_count += 1;
            return TextureHandle::new(idx, self);
        }

        let idx = reg.entries.insert(
            Entry {
                kind: ResourceKind::Texture,
                ref_count: 1u32,
                cpu_state: CpuState::Loading,
                gpu_state: GpuState::Absent,
            }
        );
        reg.by_path.insert(path.to_string(), idx);

        self.make_io_request(IoRequest::LoadTexture { id: idx, path: path.to_string(), srgb });

        TextureHandle::new(idx, self)
    }
}

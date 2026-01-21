use std::ops::Range;

use generational_arena::{Arena, Index};

use crate::renderer::{bindgroups::material::MaterialBinding, wgpu_context::WgpuContext};

use super::{animation::AnimationClip, file_formats::{animationfile, skeletonfile::Skeleton}};

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct ModelRenderId(pub Index);
impl Into<Index> for ModelRenderId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct SkeletonRenderId(pub Index);
impl Into<Index> for SkeletonRenderId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct AnimationClipRenderId(pub Index);
impl Into<Index> for AnimationClipRenderId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct AnimationRenderId(pub Index);
impl Into<Index> for AnimationRenderId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct MeshRenderId(pub Index);
impl Into<Index> for MeshRenderId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct MaterialRenderId(pub Index);
impl Into<Index> for MaterialRenderId {
    fn into(self) -> Index {
        self.0
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct TextureRenderId(pub Index);
impl Into<Index> for TextureRenderId {
    fn into(self) -> Index {
        self.0
    }
}

pub struct MeshGpuData {
    pub buffer: wgpu::Buffer,
}

pub struct TextureGpuData {
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
}

/// indices to GpuResources textures arena
pub struct PlaceholderTextureIds {
    pub normals: TextureRenderId,
    pub base_color: TextureRenderId,
    pub occlusion: TextureRenderId,
    pub emissive: TextureRenderId,
    pub metallic_roughness: TextureRenderId,
    pub prefiltered: TextureRenderId,
    pub di: TextureRenderId,
    pub brdf: TextureRenderId,
}

pub struct SubMesh {
    pub instances: Vec<[[f32; 4]; 4]>,
    pub index_range: Range<u32>,
    pub base_vertex: u32,
    pub material: MaterialRenderId,
}

pub struct ModelRenderData {
    pub vertex_buffer_start_offset: u32,
    pub mesh: MeshRenderId,
    pub submeshes: Vec<SubMesh>,
    pub skeleton: SkeletonRenderId,
    pub anim_clips: Vec<AnimationClipRenderId>,
}

pub struct AnimationClipRenderData {
    pub manifest: animationfile::AnimationClip,
    pub animation: AnimationRenderId,
}

pub struct RenderResources {
    pub models: Arena<ModelRenderData>,
    pub skeletons: Arena<Skeleton>,
    pub animation_clips: Arena<AnimationClipRenderData>,
    pub animations: Arena<AnimationClip>,
    pub meshes: Arena<MeshGpuData>,
    pub materials: Arena<MaterialBinding>,
    pub textures: Arena<TextureGpuData>,
}
impl RenderResources {
    pub fn new() -> Self {
        let meshes = Arena::new();
        let materials = Arena::new();
        let textures = Arena::new();
        let models = Arena::new();
        let skeletons = Arena::new();
        let animation_clips = Arena::new();
        let animations = Arena::new();

        Self {
            meshes,
            materials,
            textures,
            models,
            skeletons,
            animation_clips,
            animations,
        }
    }

    pub fn initialize_placeholders(&mut self, wgpu_context: &WgpuContext) -> PlaceholderTextureIds {
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
        let prefiltered_view = prefiltered_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let di_view = di_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let brdf_view = brdf_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let normals = self.textures.insert(TextureGpuData { texture: normals_texture, texture_view: normals_view });
        let base_color = self.textures.insert(TextureGpuData { texture: base_color_texture, texture_view: base_color_view });
        let occlusion = self.textures.insert(TextureGpuData { texture: occlusion_texture, texture_view: occlusion_view });
        let emissive = self.textures.insert(TextureGpuData { texture: emissive_texture, texture_view: emissive_view });
        let metallic_roughness = self.textures.insert(TextureGpuData { texture: metallic_roughness_texture, texture_view: metallic_roughness_view });
        let prefiltered = self.textures.insert(TextureGpuData { texture: prefiltered_texture, texture_view: prefiltered_view });
        let di = self.textures.insert(TextureGpuData { texture: di_texture, texture_view: di_view });
        let brdf = self.textures.insert(TextureGpuData { texture: brdf_texture, texture_view: brdf_view });

        PlaceholderTextureIds {
            normals: TextureRenderId(normals),
            base_color: TextureRenderId(base_color),
            occlusion: TextureRenderId(occlusion),
            emissive: TextureRenderId(emissive),
            metallic_roughness: TextureRenderId(metallic_roughness),
            prefiltered: TextureRenderId(prefiltered),
            di: TextureRenderId(di),
            brdf: TextureRenderId(brdf),
        }
    }
}

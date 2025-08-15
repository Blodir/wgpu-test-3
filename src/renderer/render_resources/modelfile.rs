use serde::{Deserialize, Serialize};

use crate::renderer::render_resources::TextureHandle;

#[derive(Serialize, Deserialize, Debug)]
pub struct Primitive {
    pub instances: Vec<[[f32; 4]; 4]>,
    pub index_byte_offset: u32,
    pub index_byte_length: u32,
    pub vertex_byte_offset: u32,
    pub vertex_byte_length: u32,
    pub base_vertex: u32,
    pub material: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// index buffer format is always u32
#[derive(Serialize, Deserialize, Debug)]
pub struct Model {
    pub primitives: Vec<Primitive>,
    pub materials: Vec<Material>,
    pub buffer_path: String,
    pub vertex_buffer_start_offset: u32,
    pub aabb: Aabb,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum FilterMode {
    Nearest,
    Linear,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum MipmapFilterMode {
    Nearest,
    Linear,
    None, // disables mipmapping
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum WrapMode {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sampler {
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_filter: MipmapFilterMode,
    pub wrap_u: WrapMode,
    pub wrap_v: WrapMode,
    pub wrap_w: WrapMode,
}
impl Sampler {
    pub fn to_wgpu_descriptor<'a, 'b>(
        &'a self,
        label: Option<&'b str>,
    ) -> wgpu::SamplerDescriptor<'b> {
        wgpu::SamplerDescriptor {
            label,
            address_mode_u: self.wrap_u.to_wgpu(),
            address_mode_v: self.wrap_v.to_wgpu(),
            address_mode_w: self.wrap_w.to_wgpu(),
            mag_filter: self.mag_filter.to_wgpu(),
            min_filter: self.min_filter.to_wgpu(),
            mipmap_filter: self.mipmap_filter.to_wgpu(),
            ..Default::default()
        }
    }
}

impl FilterMode {
    pub fn to_wgpu(&self) -> wgpu::FilterMode {
        match self {
            FilterMode::Nearest => wgpu::FilterMode::Nearest,
            FilterMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

impl MipmapFilterMode {
    pub fn to_wgpu(&self) -> wgpu::FilterMode {
        match self {
            MipmapFilterMode::Nearest => wgpu::FilterMode::Nearest,
            MipmapFilterMode::Linear => wgpu::FilterMode::Linear,
            MipmapFilterMode::None => wgpu::FilterMode::Nearest, // or handle specially
        }
    }
}

impl WrapMode {
    pub fn to_wgpu(&self) -> wgpu::AddressMode {
        match self {
            WrapMode::Repeat => wgpu::AddressMode::Repeat,
            WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            WrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SampledTexture {
    pub source: String,
    pub sampler: Sampler,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Material {
    pub base_color_factor: [f32; 4],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub emissive_factor: [f32; 3],
    pub normal_texture_scale: f32,
    pub occlusion_strength: f32,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub normal_texture: SampledTexture,
    pub occlusion_texture: SampledTexture,
    pub emissive_texture: SampledTexture,
    pub base_color_texture: SampledTexture,
    pub metallic_roughness_texture: SampledTexture,
}

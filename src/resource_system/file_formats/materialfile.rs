use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum FilterMode {
    Nearest,
    Linear,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum MipmapFilterMode {
    Nearest,
    Linear,
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum WrapMode {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq, Clone)]
pub struct Sampler {
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_filter: MipmapFilterMode,
    pub wrap_u: WrapMode,
    pub wrap_v: WrapMode,
    pub wrap_w: WrapMode,
}
impl Default for Sampler {
    fn default() -> Self {
        Self {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Nearest,
            wrap_u: WrapMode::ClampToEdge,
            wrap_v: WrapMode::ClampToEdge,
            wrap_w: WrapMode::ClampToEdge,
        }
    }
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
            MipmapFilterMode::None => wgpu::FilterMode::Nearest,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SampledTexture {
    pub source: String,
    pub sampler: Sampler,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    pub normal_texture: Option<SampledTexture>,
    pub occlusion_texture: Option<SampledTexture>,
    pub emissive_texture: Option<SampledTexture>,
    pub base_color_texture: Option<SampledTexture>,
    pub metallic_roughness_texture: Option<SampledTexture>,
}

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt as _;

use crate::{
    fixed_snapshot::PointLightSnapshot,
    game::scene_tree::Sun,
    global_paths::{
        SHADER_DEFERRED_LIGHTING_WGSL, SHADER_GI_BLUR_WGSL, SHADER_G_BUFFER_FRAG_WGSL,
        SHADER_G_BUFFER_SKINNED_VERT_WGSL, SHADER_G_BUFFER_STATIC_VERT_WGSL,
        SHADER_HBGI_PYRAMID_WGSL, SHADER_HBGI_REPROJECT_WGSL, SHADER_HBGI_WGSL,
        SHADER_PBR_FRAG_WGSL, SHADER_SKINNED_TRANSPARENT_VERT_WGSL, SHADER_SKYBOX_WGSL,
        SHADER_STATIC_TRANSPARENT_VERT_WGSL, SHADER_SUN_SHADOW_SKINNED_VERT_WGSL,
        SHADER_SUN_SHADOW_STATIC_VERT_WGSL,
    },
    host::{
        renderer::{
            rw_buffer::{RWBuffer, RWBufferOptions},
            rw_texture::{RWTexture, RWTextureView},
            world::{
                bindgroups::{
                    bones::BoneMat34, hbgi_settings::HbgiSettingsUniform, lights::MAX_POINT_LIGHTS,
                },
                buffers::{
                    skinned_instance::SkinnedInstance, skinned_vertex::SkinnedVertex,
                    static_instance::StaticInstance, static_vertex::StaticVertex,
                },
            },
        },
        sampler_cache::SamplerCache,
        shader_cache::ShaderCache,
        wgpu_context::WgpuContext,
        world::{
            attachments::{
                color::HdrColorTexture,
                deferred::{GBufferTargets, HbgiTexture},
                depth::DepthTexture,
                hbgi_pyramid::FloatPyramidTexture,
                skybox::SkyboxOutputTexture,
            },
            pipelines::post_processing::PostProcessingPipeline,
            sun_shadow::SunShadowUniform,
            Layouts,
        },
    },
};

pub struct GBufferTextures {
    pub albedo_ao: wgpu::Texture,
    pub normal_roughness: wgpu::Texture,
    pub emissive_metallic: wgpu::Texture,
    pub motion_vectors: wgpu::Texture,
    pub depth: wgpu::Texture,
}
impl GBufferTextures {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let albedo_ao = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Albedo + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let normal_roughness = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Normal + Roughness Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let emissive_metallic = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Emissive + Metallic Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let motion_vectors = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Motion Vectors Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let depth = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GBuffer: Depth Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            albedo_ao,
            normal_roughness,
            emissive_metallic,
            motion_vectors,
            depth,
        }
    }
}

pub struct GBufferTextureViews {
    pub albedo_ao: wgpu::TextureView,
    pub normal_roughness: wgpu::TextureView,
    pub emissive_metallic: wgpu::TextureView,
    pub motion_vectors: wgpu::TextureView,
    pub depth: wgpu::TextureView,
}
impl GBufferTextureViews {
    pub fn new(textures: &GBufferTextures) -> Self {
        let albedo_ao = textures
            .albedo_ao
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("GBuffer: Albedo + AO Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let normal_roughness =
            textures
                .normal_roughness
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("GBuffer: Normal + Roughness Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });
        let emissive_metallic =
            textures
                .emissive_metallic
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("GBuffer: Emissive + Metallic Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });
        let motion_vectors = textures
            .motion_vectors
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("GBuffer: Motion Vectors Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let depth = textures
            .depth
            .create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            albedo_ao,
            normal_roughness,
            emissive_metallic,
            motion_vectors,
            depth,
        }
    }
}

pub struct MipPyramidTextures {
    pub diffuse_radiance_ao_pyramid: wgpu::Texture,
    pub depth_pyramid: wgpu::Texture,
    pub normal_pyramid: wgpu::Texture,
    pub mip_level_count: u32,
}
impl MipPyramidTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let mip_level_count = surface_config.width.max(surface_config.height).ilog2() + 1;
        let diffuse_radiance_ao_pyramid =
            wgpu_context
                .device
                .create_texture(&wgpu::wgt::TextureDescriptor {
                    label: Some("Diffuse Radiance + AO Pyramid Texture"),
                    size,
                    mip_level_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
        let depth_pyramid = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("Depth Pyramid Texture"),
                size,
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let normal_pyramid = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("Normal Pyramid Texture"),
                size,
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            diffuse_radiance_ao_pyramid,
            depth_pyramid,
            normal_pyramid,
            mip_level_count,
        }
    }
}

pub struct MipPyramidTextureViews {
    pub diffuse_radiance_ao_pyramid: wgpu::TextureView,
    pub diffuse_radiance_ao_mips: Vec<wgpu::TextureView>,
    pub depth_pyramid: wgpu::TextureView,
    pub depth_mips: Vec<wgpu::TextureView>,
    pub normal_pyramid: wgpu::TextureView,
    pub normal_mips: Vec<wgpu::TextureView>,
}
impl MipPyramidTextureViews {
    pub fn new(textures: &MipPyramidTextures) -> Self {
        let mip_level_count = textures.mip_level_count;
        let diffuse_radiance_ao_pyramid =
            textures
                .diffuse_radiance_ao_pyramid
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Diffuse Radiance + AO Pyramid Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });
        let diffuse_radiance_ao_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures
                    .diffuse_radiance_ao_pyramid
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!(
                            "Diffuse Radiance + AO Pyramid Mip {mip_level} Texture View"
                        )),
                        format: None,
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        usage: Some(
                            wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING,
                        ),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: mip_level,
                        mip_level_count: Some(1),
                        base_array_layer: 0,
                        array_layer_count: Some(1),
                    })
            })
            .collect();

        let depth_pyramid = textures
            .depth_pyramid
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Depth Pyramid Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let depth_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures
                    .depth_pyramid
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!("Depth Pyramid Mip {mip_level} Texture View")),
                        format: None,
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        usage: Some(
                            wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING,
                        ),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: mip_level,
                        mip_level_count: Some(1),
                        base_array_layer: 0,
                        array_layer_count: Some(1),
                    })
            })
            .collect();

        let normal_pyramid = textures
            .normal_pyramid
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Normal Pyramid Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            });
        let normal_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures
                    .normal_pyramid
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!("Normal Pyramid Mip {mip_level} Texture View")),
                        format: None,
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        usage: Some(
                            wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING,
                        ),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: mip_level,
                        mip_level_count: Some(1),
                        base_array_layer: 0,
                        array_layer_count: Some(1),
                    })
            })
            .collect();

        Self {
            diffuse_radiance_ao_pyramid,
            depth_pyramid,
            normal_pyramid,
            diffuse_radiance_ao_mips,
            depth_mips,
            normal_mips,
        }
    }
}

pub struct HbgiTextures {
    pub bent_ao: wgpu::Texture,
    pub near_field_irradiance: wgpu::Texture,
}
impl HbgiTextures {
    pub const SCALE_DIVISOR: u32 = 2;
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let width = (surface_config.width / Self::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / Self::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let bent_ao = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("HBGI: Bent Normal + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let near_field_irradiance =
            wgpu_context
                .device
                .create_texture(&wgpu::wgt::TextureDescriptor {
                    label: Some("HBGI: Near Field Irradiance Texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });

        Self {
            bent_ao,
            near_field_irradiance,
        }
    }
}

pub struct HbgiTextureViews {
    pub bent_ao: wgpu::TextureView,
    pub near_field_irradiance: wgpu::TextureView,
}
impl HbgiTextureViews {
    pub fn new(textures: &HbgiTextures) -> Self {
        let bent_ao = textures.bent_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HBGI: Bent Normals + AO Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let near_field_irradiance =
            textures
                .near_field_irradiance
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("HBGI: Near Field Irradiance Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });

        Self {
            bent_ao,
            near_field_irradiance,
        }
    }
}

pub struct GiBlurTextures {
    pub bent_ao: wgpu::Texture,
    pub near_field_irradiance: wgpu::Texture,
}
impl GiBlurTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let width = (surface_config.width / HbgiTextures::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / HbgiTextures::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let bent_ao = wgpu_context
            .device
            .create_texture(&wgpu::wgt::TextureDescriptor {
                label: Some("GI Blur: Bent Normal + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let near_field_irradiance =
            wgpu_context
                .device
                .create_texture(&wgpu::wgt::TextureDescriptor {
                    label: Some("GI Blur: Near Field Irradiance Texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });

        Self {
            bent_ao,
            near_field_irradiance,
        }
    }
}

pub struct GiBlurTextureViews {
    pub bent_ao: wgpu::TextureView,
    pub near_field_irradiance: wgpu::TextureView,
}
impl GiBlurTextureViews {
    pub fn new(textures: &GiBlurTextures) -> Self {
        let bent_ao = textures.bent_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("GI Blur: Bent Normals + AO Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let near_field_irradiance =
            textures
                .near_field_irradiance
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("GI Blur: Near Field Irradiance Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                });

        Self {
            bent_ao,
            near_field_irradiance,
        }
    }
}

pub struct SunShadowTexture2(wgpu::Texture);
impl SunShadowTexture2 {
    pub const SHADOW_MAP_SIZE: u32 = crate::host::world::sun_shadow::SUN_SHADOW_MAP_SIZE;
    pub const CASCADE_COUNT: u32 =
        crate::host::world::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT as u32;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let tex = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Sun Shadow Texture"),
                size: wgpu::Extent3d {
                    width: Self::SHADOW_MAP_SIZE,
                    height: Self::SHADOW_MAP_SIZE,
                    depth_or_array_layers: Self::CASCADE_COUNT,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self(tex)
    }
}

pub struct SunShadowTextureViews {
    pub array: wgpu::TextureView,
    pub cascades: Vec<wgpu::TextureView>,
}
impl SunShadowTextureViews {
    pub fn new(texture: &wgpu::Texture) -> Self {
        let array = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sun Shadow Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });

        let cascades = (0..SunShadowTexture2::CASCADE_COUNT)
            .map(|idx| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("Sun Shadow Cascade {idx} Texture View")),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    base_array_layer: idx,
                    array_layer_count: Some(1),
                })
            })
            .collect();

        Self { array, cascades }
    }
}

pub struct ReprojectTextures {
    pub near_field_irradiance: RWTexture,
    pub bent_ao: RWTexture,
    pub depth_history: RWTexture,
    pub normal_history: RWTexture,
}
impl ReprojectTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let width = (surface_config.width / HbgiTextures::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / HbgiTextures::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let near_field_irradiance = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Near Field Irradiance Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context,
        );
        let bent_ao = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Bent Normals + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context,
        );
        let depth_history = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Depth History Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context,
        );
        let normal_history = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Normal History Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context,
        );

        Self {
            near_field_irradiance,
            bent_ao,
            depth_history,
            normal_history,
        }
    }
}

pub struct ReprojectTextureViews {
    pub near_field_irradiance: RWTextureView,
    pub bent_ao: RWTextureView,
    pub depth_history: RWTextureView,
    pub normal_history: RWTextureView,
}
impl ReprojectTextureViews {
    pub fn new(textures: ReprojectTextures) -> Self {
        let near_field_irradiance = RWTextureView::new(
            &textures.near_field_irradiance,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Near Field Irradiance Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        let bent_ao = RWTextureView::new(
            &textures.bent_ao,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Bent Normals + AO Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        let depth_history = RWTextureView::new(
            &textures.depth_history,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Depth History Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        let normal_history = RWTextureView::new(
            &textures.normal_history,
            &wgpu::TextureViewDescriptor {
                label: Some("Reproject: Normal History Texture View"),
                ..wgpu::TextureViewDescriptor::default()
            },
        );
        Self {
            near_field_irradiance,
            bent_ao,
            depth_history,
            normal_history,
        }
    }
}

struct LightingTextures {
    lit_hdr: wgpu::Texture,
    diffuse_radiance_ao: wgpu::Texture,
}
impl LightingTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let lit_hdr = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Lighting: Lit HDR Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let diffuse_radiance_ao = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Lighting: Diffuse Radiance + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            lit_hdr,
            diffuse_radiance_ao,
        }
    }
}

struct LightingTextureViews {
    lit_hdr: wgpu::TextureView,
    diffuse_radiance_ao: wgpu::TextureView,
}
impl LightingTextureViews {
    pub fn new(textures: &LightingTextures) -> Self {
        let lit_hdr = textures.lit_hdr.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Lighting: Lit HDR Texture View"),
            ..Default::default()
        });
        let diffuse_radiance_ao =
            textures
                .diffuse_radiance_ao
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Lighting: Diffuse Radiance + AO Texture View"),
                    ..Default::default()
                });

        Self {
            lit_hdr,
            diffuse_radiance_ao,
        }
    }
}

struct Textures {
    gbuffer: GBufferTextures,
    hbgi: HbgiTextures,
    gi_blur: GiBlurTextures,
    reproject: ReprojectTextures,
    pyramids: MipPyramidTextures,
    sky: wgpu::Texture,
    sun_shadow: SunShadowTexture2,
    lighting_target: LightingTextures,
    post_process_target: wgpu::Texture,
}
impl Textures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let gbuffer = GBufferTextures::new(wgpu_context, surface_config);
        let hbgi = HbgiTextures::new(wgpu_context, surface_config);
        let gi_blur = GiBlurTextures::new(wgpu_context, surface_config);
        let reproject = ReprojectTextures::new(wgpu_context, surface_config);
        let pyramids = MipPyramidTextures::new(wgpu_context, surface_config);
        let sun_shadow = SunShadowTexture2::new(wgpu_context);
        let lighting_target = LightingTextures::new(wgpu_context, surface_config);

        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let sky = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Sky Target Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        let post_process_target = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Post Process Target Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self {
            gbuffer,
            hbgi,
            gi_blur,
            reproject,
            pyramids,
            sky,
            sun_shadow,
            post_process_target,
            lighting_target,
        }
    }
}

struct TextureViews {
    gbuffer: GBufferTextureViews,
    hbgi: HbgiTextureViews,
    gi_blur: GiBlurTextureViews,
    reproject: ReprojectTextureViews,
    pyramids: MipPyramidTextureViews,
    sky: wgpu::TextureView,
    sun_shadow: SunShadowTextureViews,
    lighting_target: LightingTextureViews,
    post_process_target: wgpu::TextureView,
}
impl TextureViews {
    pub fn new(textures: Textures) -> Self {
        let gbuffer = GBufferTextureViews::new(&textures.gbuffer);
        let hbgi = HbgiTextureViews::new(&textures.hbgi);
        let gi_blur = GiBlurTextureViews::new(&textures.gi_blur);
        let reproject = ReprojectTextureViews::new(textures.reproject);
        let pyramids = MipPyramidTextureViews::new(&textures.pyramids);
        let sun_shadow = SunShadowTextureViews::new(&textures.sun_shadow.0);
        let lighting_target = LightingTextureViews::new(&textures.lighting_target);

        let sky = textures.sky.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sky Target Texture View"),
            ..Default::default()
        });
        let post_process_target =
            textures
                .post_process_target
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Post Process Target Texture View"),
                    ..Default::default()
                });

        Self {
            gbuffer,
            hbgi,
            gi_blur,
            reproject,
            pyramids,
            sky,
            sun_shadow,
            lighting_target,
            post_process_target,
        }
    }
}

pub struct CameraBuffers {
    pub view_proj: wgpu::Buffer,
    pub position: wgpu::Buffer,
    pub inverse_view_proj: wgpu::Buffer,
    pub forward: wgpu::Buffer,
    pub view_rotation: wgpu::Buffer,
    pub prev_view_proj: wgpu::Buffer,
}
impl CameraBuffers {
    fn padded_view_rotation(right: Vec3, up: Vec3, look: Vec3) -> [[f32; 4]; 3] {
        [
            [right.x, right.y, right.z, 0.0],
            [up.x, up.y, up.z, 0.0],
            [look.x, look.y, look.z, 0.0],
        ]
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let position = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Position Buffer"),
            contents: bytemuck::cast_slice(&Vec3::ZERO.to_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let inverse_view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Inverse View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let forward = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Forward Buffer"),
            contents: bytemuck::cast_slice(&[0.0, 0.0, -1.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let view_rotation = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera View Rotation Buffer"),
            contents: bytemuck::cast_slice(&Self::padded_view_rotation(Vec3::X, Vec3::Y, -Vec3::Z)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let prev_view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Previous View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        CameraBuffers {
            view_proj,
            position,
            inverse_view_proj,
            forward,
            view_rotation,
            prev_view_proj,
        }
    }

    pub fn update(
        &self,
        view_proj: &[f32; 16],
        position: &[f32; 3],
        inverse_view_proj: &[f32; 16],
        forward: &[f32; 3],
        view_rotation: &[[f32; 4]; 3],
        prev_view_proj: &[f32; 16],
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(&self.view_proj, 0, bytemuck::cast_slice(view_proj));
        queue.write_buffer(&self.position, 0, bytemuck::cast_slice(position));
        queue.write_buffer(
            &self.inverse_view_proj,
            0,
            bytemuck::cast_slice(inverse_view_proj),
        );
        queue.write_buffer(&self.forward, 0, bytemuck::cast_slice(forward));
        queue.write_buffer(&self.view_rotation, 0, bytemuck::cast_slice(view_rotation));
        queue.write_buffer(
            &self.prev_view_proj,
            0,
            bytemuck::cast_slice(prev_view_proj),
        );
    }
}

pub struct SunBuffers {
    pub direction: wgpu::Buffer,
    pub color: wgpu::Buffer,
    pub shadow: wgpu::Buffer,
}
impl SunBuffers {
    pub fn new(device: &wgpu::Device) -> Self {
        let sun = Sun::default();
        let direction = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Direction Buffer"),
            contents: bytemuck::cast_slice(&sun.direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let color = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Color Buffer"),
            contents: bytemuck::cast_slice(&sun.color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shadow = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Shadow Buffer"),
            contents: bytemuck::bytes_of(&SunShadowUniform::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            direction,
            color,
            shadow,
        }
    }
    pub fn update_direction(&self, direction: &[f32; 3], queue: &wgpu::Queue) {
        queue.write_buffer(&self.direction, 0, bytemuck::cast_slice(direction));
    }
    pub fn update_color(&self, color: &[f32; 3], queue: &wgpu::Queue) {
        queue.write_buffer(&self.color, 0, bytemuck::cast_slice(color));
    }
    pub fn update_shadow(&self, shadow: &SunShadowUniform, queue: &wgpu::Queue) {
        queue.write_buffer(&self.shadow, 0, bytemuck::bytes_of(shadow));
    }
}

pub struct LightsBuffers {
    pub sun: SunBuffers,
    pub environment_map_intensity: wgpu::Buffer,
    pub point_light_count: wgpu::Buffer,
    pub point_light_positions_ranges: wgpu::Buffer,
    pub point_light_colors_intensities: wgpu::Buffer,
}
impl LightsBuffers {
    pub fn new(device: &wgpu::Device) -> Self {
        let sun = SunBuffers::new(device);
        let environment_map_intensity =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Environment Map Intensity Buffer"),
                contents: bytemuck::cast_slice(&[1.0f32]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let point_light_count = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Point Light Count Buffer"),
            contents: bytemuck::cast_slice(&[[0u32, 0u32, 0u32, 0u32]]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let point_light_positions_ranges =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Point Light Positions and Ranges Buffer"),
                contents: bytemuck::cast_slice(&[[0.0f32; 4]; MAX_POINT_LIGHTS]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let point_light_colors_intensities =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Point Light Colors and Intensities Buffer"),
                contents: bytemuck::cast_slice(&[[0.0f32; 4]; MAX_POINT_LIGHTS]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            sun,
            environment_map_intensity,
            point_light_count,
            point_light_positions_ranges,
            point_light_colors_intensities,
        }
    }

    pub fn update_sun(&self, sun: &Sun, queue: &wgpu::Queue) {
        self.sun.update_direction(&sun.direction, queue);
        self.sun.update_color(&sun.color, queue);
    }

    pub fn update_environment_map_intensity(&self, intensity: f32, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.environment_map_intensity,
            0,
            bytemuck::cast_slice(&[intensity]),
        );
    }

    pub fn update_point_lights(&self, point_lights: &[PointLightSnapshot], queue: &wgpu::Queue) {
        let clamped_count = point_lights.len().min(MAX_POINT_LIGHTS);
        let mut point_positions_ranges = [[0.0f32; 4]; MAX_POINT_LIGHTS];
        let mut point_colors_intensities = [[0.0f32; 4]; MAX_POINT_LIGHTS];

        for (idx, light) in point_lights.iter().take(clamped_count).enumerate() {
            point_positions_ranges[idx] = [
                light.position.x,
                light.position.y,
                light.position.z,
                light.range,
            ];
            point_colors_intensities[idx] = [
                light.color[0],
                light.color[1],
                light.color[2],
                light.intensity,
            ];
        }

        queue.write_buffer(
            &self.point_light_count,
            0,
            bytemuck::cast_slice(&[[clamped_count as u32, 0u32, 0u32, 0u32]]),
        );
        queue.write_buffer(
            &self.point_light_positions_ranges,
            0,
            bytemuck::cast_slice(&point_positions_ranges),
        );
        queue.write_buffer(
            &self.point_light_colors_intensities,
            0,
            bytemuck::cast_slice(&point_colors_intensities),
        );
    }
}

const FULLSCREEN_QUAD_INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct HbgiReprojectUniform {
    prev_inverse_view_proj: [[f32; 4]; 4],
}

struct Buffers {
    pub bones: RWBuffer,
    pub camera: CameraBuffers,
    pub hbgi_settings: wgpu::Buffer,
    pub hbgi_reproject_settings: wgpu::Buffer,
    pub fullscreen_quad_indices: wgpu::Buffer,
    pub skinned_instances: RWBuffer,
    pub static_instances: wgpu::Buffer,
    pub lights: LightsBuffers,
}
impl Buffers {
    pub fn new(wgpu_context: &WgpuContext, hbgi_settings: Option<&HbgiSettingsUniform>) -> Self {
        let device = &wgpu_context.device;
        let storage_usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

        let mut bones = RWBuffer::new(
            RWBufferOptions {
                label: Some("Bones SSBO".to_string()),
                usage: storage_usage,
            },
            wgpu_context,
        );
        bones.write(
            bytemuck::cast_slice(&vec![BoneMat34::default(); 2048]),
            wgpu_context,
        );

        let camera = CameraBuffers::new(device);

        let hbgi_settings = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HBGI Settings Buffer"),
            contents: bytemuck::bytes_of(
                &hbgi_settings
                    .copied()
                    .unwrap_or_else(HbgiSettingsUniform::default),
            ),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let hbgi_reproject_settings =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HBGI Reproject Settings Buffer"),
                contents: bytemuck::bytes_of(&HbgiReprojectUniform {
                    prev_inverse_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let fullscreen_quad_indices =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Fullscreen Quad Index Buffer"),
                contents: bytemuck::cast_slice(FULLSCREEN_QUAD_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let mut skinned_instances = RWBuffer::new(
            RWBufferOptions {
                label: Some("Skinned Instance Buffer".to_string()),
                usage: storage_usage,
            },
            wgpu_context,
        );
        skinned_instances.write(
            bytemuck::cast_slice(&[SkinnedInstance::default()]),
            wgpu_context,
        );

        let static_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance buffer"),
            contents: bytemuck::cast_slice(&[StaticInstance::default()]),
            usage: storage_usage,
        });

        let lights = LightsBuffers::new(device);

        Self {
            bones,
            camera,
            hbgi_settings,
            hbgi_reproject_settings,
            fullscreen_quad_indices,
            skinned_instances,
            static_instances,
            lights,
        }
    }

    pub fn update_hbgi_settings(&self, hbgi_settings: &HbgiSettingsUniform, queue: &wgpu::Queue) {
        queue.write_buffer(&self.hbgi_settings, 0, bytemuck::bytes_of(hbgi_settings));
    }

    pub fn update_hbgi_reproject_settings(
        &self,
        prev_inverse_view_proj: &Mat4,
        queue: &wgpu::Queue,
    ) {
        let uniform = HbgiReprojectUniform {
            prev_inverse_view_proj: prev_inverse_view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(
            &self.hbgi_reproject_settings,
            0,
            bytemuck::bytes_of(&uniform),
        );
    }
}

struct Samplers {
    sampler_cache: SamplerCache,
    hbgi_pyramid_downsample: wgpu::Sampler,
}
impl Samplers {
    fn new(device: &wgpu::Device) -> Self {
        let sampler_cache = SamplerCache::new();
        let hbgi_pyramid_downsample = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            sampler_cache,
            hbgi_pyramid_downsample,
        }
    }
}

struct GpuResources {
    textures: Textures,
    buffers: Buffers,
    samplers: Samplers,
}

struct BonesBindGroups {
    pub bones_bind_group: wgpu::BindGroup,
    pub motion_bind_group: wgpu::BindGroup,
}
impl BonesBindGroups {
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
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

    pub fn motion_desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Motion Bones Bind Group Layout"),
        }
    }

    fn create_bones_bind_group(
        bones: &RWBuffer,
        skinned_instances: &RWBuffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bones Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: bones.get_write_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skinned_instances.get_write_buf().as_entire_binding(),
                },
            ],
        })
    }

    fn create_motion_bind_group(
        bones: &RWBuffer,
        skinned_instances: &RWBuffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Motion Bones Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: bones.get_write_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bones.get_read_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: skinned_instances.get_write_buf().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: skinned_instances.get_read_buf().as_entire_binding(),
                },
            ],
        })
    }

    pub fn new(
        bones: &RWBuffer,
        skinned_instances: &RWBuffer,
        layout: &wgpu::BindGroupLayout,
        motion_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bones_bind_group: Self::create_bones_bind_group(
                bones,
                skinned_instances,
                layout,
                device,
            ),
            motion_bind_group: Self::create_motion_bind_group(
                bones,
                skinned_instances,
                motion_layout,
                device,
            ),
        }
    }

    pub fn update(
        &mut self,
        bones: &RWBuffer,
        skinned_instances: &RWBuffer,
        layout: &wgpu::BindGroupLayout,
        motion_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bones_bind_group =
            Self::create_bones_bind_group(bones, skinned_instances, layout, device);
        self.motion_bind_group =
            Self::create_motion_bind_group(bones, skinned_instances, motion_layout, device);
    }
}

struct CameraBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl CameraBindGroup {
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
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

    fn create_bind_group(
        buffers: &CameraBuffers,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.view_proj.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.position.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffers.inverse_view_proj.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffers.forward.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: buffers.view_rotation.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: buffers.prev_view_proj.as_entire_binding(),
                },
            ],
            label: Some("Camera Bind Group"),
        })
    }

    pub fn new(
        buffers: &CameraBuffers,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(buffers, layout, device),
        }
    }
}

struct GBufferBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl GBufferBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
            label: Some("GBuffer Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        texture_views: &GBufferTextureViews,
        albedo_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        emissive_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_views.albedo_ao),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&texture_views.normal_roughness),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(normal_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&texture_views.emissive_metallic),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(emissive_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&texture_views.depth),
                },
            ],
            label: Some("GBuffer Inputs Bind Group"),
        })
    }

    pub fn new(
        texture_views: &GBufferTextureViews,
        albedo_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        emissive_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                texture_views,
                albedo_sampler,
                normal_sampler,
                emissive_sampler,
                layout,
                device,
            ),
        }
    }

    pub fn update(
        &mut self,
        texture_views: &GBufferTextureViews,
        albedo_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        emissive_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(
            texture_views,
            albedo_sampler,
            normal_sampler,
            emissive_sampler,
            layout,
            device,
        );
    }
}

struct HbgiSettingsBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl HbgiSettingsBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("HBGI Settings Bind Group Layout"),
        }
    }

    fn create_bind_group(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("HBGI Settings Bind Group"),
        })
    }

    pub fn new(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(buffer, layout, device),
        }
    }
}

struct HbgiInputsBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl HbgiInputsBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("HBGI Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        texture_views: &MipPyramidTextureViews,
        hbgi_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture_views.diffuse_radiance_ao_pyramid,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(hbgi_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&texture_views.depth_pyramid),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&texture_views.normal_pyramid),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(normal_sampler),
                },
            ],
            label: Some("HBGI Inputs Bind Group"),
        })
    }

    pub fn new(
        texture_views: &MipPyramidTextureViews,
        hbgi_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                texture_views,
                hbgi_sampler,
                normal_sampler,
                layout,
                device,
            ),
        }
    }

    pub fn update(
        &mut self,
        texture_views: &MipPyramidTextureViews,
        hbgi_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group =
            Self::create_bind_group(texture_views, hbgi_sampler, normal_sampler, layout, device);
    }
}

struct HbgiBindGroups {
    pub settings: HbgiSettingsBindGroup,
    pub inputs: HbgiInputsBindGroup,
}
impl HbgiBindGroups {
    pub fn new(
        hbgi_settings_buffer: &wgpu::Buffer,
        hbgi_settings_layout: &wgpu::BindGroupLayout,
        mip_texture_views: &MipPyramidTextureViews,
        hbgi_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        hbgi_inputs_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        let settings =
            HbgiSettingsBindGroup::new(hbgi_settings_buffer, hbgi_settings_layout, device);
        let inputs = HbgiInputsBindGroup::new(
            mip_texture_views,
            hbgi_sampler,
            normal_sampler,
            hbgi_inputs_layout,
            device,
        );

        Self { settings, inputs }
    }

    pub fn update_inputs(
        &mut self,
        mip_texture_views: &MipPyramidTextureViews,
        hbgi_sampler: &wgpu::Sampler,
        normal_sampler: &wgpu::Sampler,
        hbgi_inputs_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.inputs.update(
            mip_texture_views,
            hbgi_sampler,
            normal_sampler,
            hbgi_inputs_layout,
            device,
        );
    }

    pub fn update_settings(
        &mut self,
        hbgi_settings_buffer: &wgpu::Buffer,
        hbgi_settings_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.settings =
            HbgiSettingsBindGroup::new(hbgi_settings_buffer, hbgi_settings_layout, device);
    }
}

pub(crate) struct GiBlurBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl GiBlurBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
            label: Some("GI Blur Bind Group Layout"),
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        raw_hbgi_view: &wgpu::TextureView,
        raw_hbgi_sampler: &wgpu::Sampler,
        raw_hbgi_irradiance_view: &wgpu::TextureView,
        raw_hbgi_irradiance_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(raw_hbgi_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(raw_hbgi_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(raw_hbgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(raw_hbgi_irradiance_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(normal_roughness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(normal_roughness_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
            ],
            label: Some("GI Blur Bind Group"),
        })
    }

    pub fn new(
        raw_hbgi_view: &wgpu::TextureView,
        raw_hbgi_sampler: &wgpu::Sampler,
        raw_hbgi_irradiance_view: &wgpu::TextureView,
        raw_hbgi_irradiance_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                device,
                layout,
                raw_hbgi_view,
                raw_hbgi_sampler,
                raw_hbgi_irradiance_view,
                raw_hbgi_irradiance_sampler,
                normal_roughness_view,
                normal_roughness_sampler,
                depth_view,
            ),
        }
    }

    pub fn update(
        &mut self,
        raw_hbgi_view: &wgpu::TextureView,
        raw_hbgi_sampler: &wgpu::Sampler,
        raw_hbgi_irradiance_view: &wgpu::TextureView,
        raw_hbgi_irradiance_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(
            device,
            layout,
            raw_hbgi_view,
            raw_hbgi_sampler,
            raw_hbgi_irradiance_view,
            raw_hbgi_irradiance_sampler,
            normal_roughness_view,
            normal_roughness_sampler,
            depth_view,
        );
    }
}

pub(crate) struct DeferredLightingGBufferBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl DeferredLightingGBufferBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
            label: Some("GBuffer Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        albedo_ao_view: &wgpu::TextureView,
        albedo_ao_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        emissive_metallic_view: &wgpu::TextureView,
        emissive_metallic_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(albedo_ao_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(albedo_ao_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(normal_roughness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(normal_roughness_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(emissive_metallic_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(emissive_metallic_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
            ],
            label: Some("GBuffer Inputs Bind Group"),
        })
    }

    pub fn new(
        albedo_ao_view: &wgpu::TextureView,
        albedo_ao_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        emissive_metallic_view: &wgpu::TextureView,
        emissive_metallic_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                device,
                layout,
                albedo_ao_view,
                albedo_ao_sampler,
                normal_roughness_view,
                normal_roughness_sampler,
                emissive_metallic_view,
                emissive_metallic_sampler,
                depth_view,
            ),
        }
    }

    pub fn update(
        &mut self,
        albedo_ao_view: &wgpu::TextureView,
        albedo_ao_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        emissive_metallic_view: &wgpu::TextureView,
        emissive_metallic_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(
            device,
            layout,
            albedo_ao_view,
            albedo_ao_sampler,
            normal_roughness_view,
            normal_roughness_sampler,
            emissive_metallic_view,
            emissive_metallic_sampler,
            depth_view,
        );
    }
}

pub(crate) struct DeferredLightingHbgiBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl DeferredLightingHbgiBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
            ],
            label: Some("HBGI Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        blurred_view: &wgpu::TextureView,
        blurred_sampler: &wgpu::Sampler,
        blurred_irradiance_view: &wgpu::TextureView,
        blurred_irradiance_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(blurred_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(blurred_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(blurred_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(blurred_irradiance_sampler),
                },
            ],
            label: Some("HBGI Inputs Bind Group"),
        })
    }

    pub fn new(
        blurred_view: &wgpu::TextureView,
        blurred_sampler: &wgpu::Sampler,
        blurred_irradiance_view: &wgpu::TextureView,
        blurred_irradiance_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                device,
                layout,
                blurred_view,
                blurred_sampler,
                blurred_irradiance_view,
                blurred_irradiance_sampler,
            ),
        }
    }

    pub fn update(
        &mut self,
        blurred_view: &wgpu::TextureView,
        blurred_sampler: &wgpu::Sampler,
        blurred_irradiance_view: &wgpu::TextureView,
        blurred_irradiance_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(
            device,
            layout,
            blurred_view,
            blurred_sampler,
            blurred_irradiance_view,
            blurred_irradiance_sampler,
        );
    }
}

struct DeferredLightingBindGroups {
    pub gbuffer: DeferredLightingGBufferBindGroup,
    pub hbgi: DeferredLightingHbgiBindGroup,
}
impl DeferredLightingBindGroups {
    pub fn new(
        albedo_ao_view: &wgpu::TextureView,
        albedo_ao_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        emissive_metallic_view: &wgpu::TextureView,
        emissive_metallic_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        blurred_view: &wgpu::TextureView,
        blurred_sampler: &wgpu::Sampler,
        blurred_irradiance_view: &wgpu::TextureView,
        blurred_irradiance_sampler: &wgpu::Sampler,
        layouts: &Layouts,
        device: &wgpu::Device,
    ) -> Self {
        let gbuffer = DeferredLightingGBufferBindGroup::new(
            albedo_ao_view,
            albedo_ao_sampler,
            normal_roughness_view,
            normal_roughness_sampler,
            emissive_metallic_view,
            emissive_metallic_sampler,
            depth_view,
            &layouts.deferred_lighting_gbuffer,
            device,
        );
        let hbgi = DeferredLightingHbgiBindGroup::new(
            blurred_view,
            blurred_sampler,
            blurred_irradiance_view,
            blurred_irradiance_sampler,
            &layouts.deferred_lighting_hbgi,
            device,
        );

        Self { gbuffer, hbgi }
    }

    pub fn update_gbuffer(
        &mut self,
        albedo_ao_view: &wgpu::TextureView,
        albedo_ao_sampler: &wgpu::Sampler,
        normal_roughness_view: &wgpu::TextureView,
        normal_roughness_sampler: &wgpu::Sampler,
        emissive_metallic_view: &wgpu::TextureView,
        emissive_metallic_sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        layouts: &Layouts,
        device: &wgpu::Device,
    ) {
        self.gbuffer.update(
            albedo_ao_view,
            albedo_ao_sampler,
            normal_roughness_view,
            normal_roughness_sampler,
            emissive_metallic_view,
            emissive_metallic_sampler,
            depth_view,
            &layouts.deferred_lighting_gbuffer,
            device,
        );
    }

    pub fn update_hbgi(
        &mut self,
        blurred_view: &wgpu::TextureView,
        blurred_sampler: &wgpu::Sampler,
        blurred_irradiance_view: &wgpu::TextureView,
        blurred_irradiance_sampler: &wgpu::Sampler,
        layouts: &Layouts,
        device: &wgpu::Device,
    ) {
        self.hbgi.update(
            blurred_view,
            blurred_sampler,
            blurred_irradiance_view,
            blurred_irradiance_sampler,
            &layouts.deferred_lighting_hbgi,
            device,
        );
    }
}

pub(crate) struct HbgiReprojectInputsBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl HbgiReprojectInputsBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
            label: Some("HBGI Reproject Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        motion_vectors_view: &wgpu::TextureView,
        current_hbgi_view: &wgpu::TextureView,
        prev_hbgi_reproject_view: &wgpu::TextureView,
        current_depth_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        prev_depth_history_view: &wgpu::TextureView,
        prev_normal_history_view: &wgpu::TextureView,
        current_hbgi_irradiance_view: &wgpu::TextureView,
        prev_hbgi_irradiance_reproject_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(motion_vectors_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(current_hbgi_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(prev_hbgi_reproject_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(current_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(current_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(prev_depth_history_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(prev_normal_history_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(current_hbgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(
                        prev_hbgi_irradiance_reproject_view,
                    ),
                },
            ],
            label: Some("HBGI Reproject Inputs Bind Group"),
        })
    }

    pub fn new(
        motion_vectors_view: &wgpu::TextureView,
        current_hbgi_view: &wgpu::TextureView,
        prev_hbgi_reproject_view: &wgpu::TextureView,
        current_depth_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        prev_depth_history_view: &wgpu::TextureView,
        prev_normal_history_view: &wgpu::TextureView,
        current_hbgi_irradiance_view: &wgpu::TextureView,
        prev_hbgi_irradiance_reproject_view: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                device,
                layout,
                motion_vectors_view,
                current_hbgi_view,
                prev_hbgi_reproject_view,
                current_depth_view,
                current_normal_view,
                prev_depth_history_view,
                prev_normal_history_view,
                current_hbgi_irradiance_view,
                prev_hbgi_irradiance_reproject_view,
            ),
        }
    }

    pub fn update(
        &mut self,
        motion_vectors_view: &wgpu::TextureView,
        current_hbgi_view: &wgpu::TextureView,
        prev_hbgi_reproject_view: &wgpu::TextureView,
        current_depth_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        prev_depth_history_view: &wgpu::TextureView,
        prev_normal_history_view: &wgpu::TextureView,
        current_hbgi_irradiance_view: &wgpu::TextureView,
        prev_hbgi_irradiance_reproject_view: &wgpu::TextureView,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(
            device,
            layout,
            motion_vectors_view,
            current_hbgi_view,
            prev_hbgi_reproject_view,
            current_depth_view,
            current_normal_view,
            prev_depth_history_view,
            prev_normal_history_view,
            current_hbgi_irradiance_view,
            prev_hbgi_irradiance_reproject_view,
        );
    }
}

pub(crate) struct HbgiReprojectSettingsBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl HbgiReprojectSettingsBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("HBGI Reproject Settings Bind Group Layout"),
        }
    }

    fn create_bind_group(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("HBGI Reproject Settings Bind Group"),
        })
    }

    pub fn new(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(buffer, layout, device),
        }
    }

    pub fn update(
        &mut self,
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(buffer, layout, device);
    }
}

struct HbgiReprojectBindGroups {
    pub inputs: HbgiReprojectInputsBindGroup,
    pub settings: HbgiReprojectSettingsBindGroup,
}
impl HbgiReprojectBindGroups {
    pub fn new(
        motion_vectors_view: &wgpu::TextureView,
        current_hbgi_view: &wgpu::TextureView,
        prev_hbgi_reproject_view: &wgpu::TextureView,
        current_depth_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        prev_depth_history_view: &wgpu::TextureView,
        prev_normal_history_view: &wgpu::TextureView,
        current_hbgi_irradiance_view: &wgpu::TextureView,
        prev_hbgi_irradiance_reproject_view: &wgpu::TextureView,
        settings_buffer: &wgpu::Buffer,
        layouts: &Layouts,
        device: &wgpu::Device,
    ) -> Self {
        let inputs = HbgiReprojectInputsBindGroup::new(
            motion_vectors_view,
            current_hbgi_view,
            prev_hbgi_reproject_view,
            current_depth_view,
            current_normal_view,
            prev_depth_history_view,
            prev_normal_history_view,
            current_hbgi_irradiance_view,
            prev_hbgi_irradiance_reproject_view,
            &layouts.hbgi_reproject_inputs,
            device,
        );
        let settings = HbgiReprojectSettingsBindGroup::new(
            settings_buffer,
            &layouts.hbgi_reproject_settings,
            device,
        );

        Self { inputs, settings }
    }

    pub fn update_inputs(
        &mut self,
        motion_vectors_view: &wgpu::TextureView,
        current_hbgi_view: &wgpu::TextureView,
        prev_hbgi_reproject_view: &wgpu::TextureView,
        current_depth_view: &wgpu::TextureView,
        current_normal_view: &wgpu::TextureView,
        prev_depth_history_view: &wgpu::TextureView,
        prev_normal_history_view: &wgpu::TextureView,
        current_hbgi_irradiance_view: &wgpu::TextureView,
        prev_hbgi_irradiance_reproject_view: &wgpu::TextureView,
        layouts: &Layouts,
        device: &wgpu::Device,
    ) {
        self.inputs.update(
            motion_vectors_view,
            current_hbgi_view,
            prev_hbgi_reproject_view,
            current_depth_view,
            current_normal_view,
            prev_depth_history_view,
            prev_normal_history_view,
            current_hbgi_irradiance_view,
            prev_hbgi_irradiance_reproject_view,
            &layouts.hbgi_reproject_inputs,
            device,
        );
    }

    pub fn update_settings(
        &mut self,
        settings_buffer: &wgpu::Buffer,
        layouts: &Layouts,
        device: &wgpu::Device,
    ) {
        self.settings
            .update(settings_buffer, &layouts.hbgi_reproject_settings, device);
    }
}

pub(crate) struct HbgiPyramidBindGroups {
    pub base: wgpu::BindGroup,
    pub downsample: Vec<wgpu::BindGroup>,
}
impl HbgiPyramidBindGroups {
    pub fn base_desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("HBGI Pyramid Base Bind Group Layout"),
        }
    }

    pub fn downsample_desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("HBGI Pyramid Downsample Bind Group Layout"),
        }
    }

    fn create_base_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        source_hbgi_view: &wgpu::TextureView,
        source_hbgi_sampler: &wgpu::Sampler,
        source_depth_view: &wgpu::TextureView,
        source_normal_view: &wgpu::TextureView,
        source_normal_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_hbgi_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(source_hbgi_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(source_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(source_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(source_normal_sampler),
                },
            ],
            label: Some("HBGI Pyramid Base Bind Group"),
        })
    }

    fn create_downsample_bind_groups(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        downsample_sampler: &wgpu::Sampler,
        pyramids: &MipPyramidTextureViews,
    ) -> Vec<wgpu::BindGroup> {
        (1..pyramids.diffuse_radiance_ao_mips.len())
            .map(|dst_mip| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramids.diffuse_radiance_ao_mips[dst_mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramids.depth_mips[dst_mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::TextureView(
                                &pyramids.normal_mips[dst_mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::Sampler(downsample_sampler),
                        },
                    ],
                    label: Some("HBGI Pyramid Downsample Bind Group"),
                })
            })
            .collect()
    }

    pub fn new(
        device: &wgpu::Device,
        layouts: &Layouts,
        downsample_sampler: &wgpu::Sampler,
        source_hbgi_view: &wgpu::TextureView,
        source_hbgi_sampler: &wgpu::Sampler,
        source_depth_view: &wgpu::TextureView,
        source_normal_view: &wgpu::TextureView,
        source_normal_sampler: &wgpu::Sampler,
        pyramids: &MipPyramidTextureViews,
    ) -> Self {
        let base = Self::create_base_bind_group(
            device,
            &layouts.hbgi_pyramid_base,
            source_hbgi_view,
            source_hbgi_sampler,
            source_depth_view,
            source_normal_view,
            source_normal_sampler,
        );
        let downsample = Self::create_downsample_bind_groups(
            device,
            &layouts.hbgi_pyramid_downsample,
            downsample_sampler,
            pyramids,
        );

        Self { base, downsample }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        layouts: &Layouts,
        downsample_sampler: &wgpu::Sampler,
        source_hbgi_view: &wgpu::TextureView,
        source_hbgi_sampler: &wgpu::Sampler,
        source_depth_view: &wgpu::TextureView,
        source_normal_view: &wgpu::TextureView,
        source_normal_sampler: &wgpu::Sampler,
        pyramids: &MipPyramidTextureViews,
    ) {
        *self = Self::new(
            device,
            layouts,
            downsample_sampler,
            source_hbgi_view,
            source_hbgi_sampler,
            source_depth_view,
            source_normal_view,
            source_normal_sampler,
            pyramids,
        );
    }
}

struct LightsBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl LightsBindGroup {
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
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 11,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 12,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 13,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 14,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
            label: Some("Lights Group Layout"),
        }
    }

    fn create_bind_group(
        buffers: &LightsBuffers,
        prefiltered_view: &wgpu::TextureView,
        prefiltered_sampler: &wgpu::Sampler,
        di_view: &wgpu::TextureView,
        di_sampler: &wgpu::Sampler,
        brdf_view: &wgpu::TextureView,
        brdf_sampler: &wgpu::Sampler,
        sun_shadow_view: &wgpu::TextureView,
        sun_shadow_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.sun.direction.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.sun.color.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(prefiltered_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(prefiltered_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(di_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(di_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(brdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(brdf_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: buffers.environment_map_intensity.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: buffers.point_light_count.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: buffers.point_light_positions_ranges.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: buffers.point_light_colors_intensities.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: buffers.sun.shadow.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(sun_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::Sampler(sun_shadow_sampler),
                },
            ],
            label: Some("Lights Bind Group"),
        })
    }

    pub fn new(
        buffers: &LightsBuffers,
        prefiltered_view: &wgpu::TextureView,
        prefiltered_sampler: &wgpu::Sampler,
        di_view: &wgpu::TextureView,
        di_sampler: &wgpu::Sampler,
        brdf_view: &wgpu::TextureView,
        brdf_sampler: &wgpu::Sampler,
        sun_shadow_view: &wgpu::TextureView,
        sun_shadow_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                buffers,
                prefiltered_view,
                prefiltered_sampler,
                di_view,
                di_sampler,
                brdf_view,
                brdf_sampler,
                sun_shadow_view,
                sun_shadow_sampler,
                layout,
                device,
            ),
        }
    }

    pub fn update(
        &mut self,
        buffers: &LightsBuffers,
        prefiltered_view: &wgpu::TextureView,
        prefiltered_sampler: &wgpu::Sampler,
        di_view: &wgpu::TextureView,
        di_sampler: &wgpu::Sampler,
        brdf_view: &wgpu::TextureView,
        brdf_sampler: &wgpu::Sampler,
        sun_shadow_view: &wgpu::TextureView,
        sun_shadow_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(
            buffers,
            prefiltered_view,
            prefiltered_sampler,
            di_view,
            di_sampler,
            brdf_view,
            brdf_sampler,
            sun_shadow_view,
            sun_shadow_sampler,
            layout,
            device,
        );
    }
}

struct PostProcessingBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl PostProcessingBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
            ],
            label: Some("Post Processing Inputs Bind Group Layout"),
        }
    }

    fn create_bind_group(
        skybox_view: &wgpu::TextureView,
        skybox_sampler: &wgpu::Sampler,
        hdr_view: &wgpu::TextureView,
        hdr_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(skybox_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(skybox_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(hdr_sampler),
                },
            ],
            label: Some("Post Processing Inputs Bind Group"),
        })
    }

    pub fn new(
        skybox_view: &wgpu::TextureView,
        skybox_sampler: &wgpu::Sampler,
        hdr_view: &wgpu::TextureView,
        hdr_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(
                skybox_view,
                skybox_sampler,
                hdr_view,
                hdr_sampler,
                layout,
                device,
            ),
        }
    }

    pub fn update(
        &mut self,
        skybox_view: &wgpu::TextureView,
        skybox_sampler: &wgpu::Sampler,
        hdr_view: &wgpu::TextureView,
        hdr_sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(
            skybox_view,
            skybox_sampler,
            hdr_view,
            hdr_sampler,
            layout,
            device,
        );
    }
}

struct SunShadowMatrixBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl SunShadowMatrixBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Sun Shadow Matrix Bind Group Layout"),
        }
    }

    fn create_bind_group(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Sun Shadow Matrix Bind Group"),
        })
    }

    pub fn new(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(buffer, layout, device),
        }
    }

    pub fn update(
        &mut self,
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(buffer, layout, device);
    }
}

struct InstanceStorageBindGroup {
    pub bind_group: wgpu::BindGroup,
}
impl InstanceStorageBindGroup {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Instance Storage Bind Group Layout"),
        }
    }

    fn create_bind_group(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Instance Storage Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    pub fn new(
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        Self {
            bind_group: Self::create_bind_group(buffer, layout, device),
        }
    }

    pub fn update(
        &mut self,
        buffer: &wgpu::Buffer,
        layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) {
        self.bind_group = Self::create_bind_group(buffer, layout, device);
    }
}

struct BindGroups {
    bones: BonesBindGroups,
    camera: CameraBindGroup,
    deferred_lighting: DeferredLightingBindGroups,
    g_buffer: GBufferBindGroup,
    hbgi: HbgiBindGroups,
    hbgi_reproject: HbgiReprojectBindGroups,
    gi_blur: GiBlurBindGroup,
    hbgi_pyramid: HbgiPyramidBindGroups,
    lights: LightsBindGroup,
    post_processing: PostProcessingBindGroup,
    sun_shadow_matrix: SunShadowMatrixBindGroup,
    static_instances: InstanceStorageBindGroup,
}

struct Descriptors {
    bind_groups: BindGroups,
    bind_group_layouts: Layouts,
    texture_views: TextureViews,
}

struct GBufferPipeline {
    skinned_pipeline: wgpu::RenderPipeline,
    static_pipeline: wgpu::RenderPipeline,
}
impl GBufferPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let skinned_pipeline = Self::build_skinned_pipeline(wgpu_context, shader_cache, layouts);
        let static_pipeline = Self::build_static_pipeline(wgpu_context, shader_cache, layouts);

        Self {
            skinned_pipeline,
            static_pipeline,
        }
    }

    fn build_skinned_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.material,
            &layouts.motion_bones,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Skinned G-Buffer Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module =
            shader_cache.get(SHADER_G_BUFFER_SKINNED_VERT_WGSL.to_string(), wgpu_context);
        let fragment_shader_module =
            shader_cache.get(SHADER_G_BUFFER_FRAG_WGSL.to_string(), wgpu_context);
        let targets = &[
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Skinned G-Buffer Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[SkinnedVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: Some("fs_main"),
                    targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: GBufferTextures::DEPTH_FORMAT,
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
                cache: None,
            })
    }

    fn build_static_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.material,
            &layouts.instance_storage,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Static G-Buffer Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module =
            shader_cache.get(SHADER_G_BUFFER_STATIC_VERT_WGSL.to_string(), wgpu_context);
        let fragment_shader_module =
            shader_cache.get(SHADER_G_BUFFER_FRAG_WGSL.to_string(), wgpu_context);
        let targets = &[
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Static G-Buffer Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[StaticVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: Some("fs_main"),
                    targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: GBufferTextures::DEPTH_FORMAT,
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
                cache: None,
            })
    }
}

struct HbgiPipeline {
    render_pipeline: wgpu::RenderPipeline,
}
impl HbgiPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let hbgi_inputs_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&HbgiInputsBindGroup::desc());
        let hbgi_settings_bind_group_layout = wgpu_context
            .device
            .create_bind_group_layout(&HbgiSettingsBindGroup::desc());
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Pipeline Layout"),
                    bind_group_layouts: &[
                        &layouts.camera,
                        &hbgi_inputs_bind_group_layout,
                        &hbgi_settings_bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_HBGI_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
        Self { render_pipeline }
    }
}

const HBGI_PYRAMID_COLOR_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const HBGI_PYRAMID_DEPTH_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

struct HbgiPyramidPipelines {
    base_pipeline: wgpu::RenderPipeline,
    downsample_pipeline: wgpu::RenderPipeline,
}
impl HbgiPyramidPipelines {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let shader_module = shader_cache.get(SHADER_HBGI_PYRAMID_WGSL.to_string(), wgpu_context);
        let base_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Pyramid Base Pipeline Layout"),
                    bind_group_layouts: &[&layouts.hbgi_pyramid_base],
                    push_constant_ranges: &[],
                });
        let downsample_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Pyramid Downsample Pipeline Layout"),
                    bind_group_layouts: &[&layouts.hbgi_pyramid_downsample],
                    push_constant_ranges: &[],
                });
        let color_targets = &[
            Some(wgpu::ColorTargetState {
                format: HBGI_PYRAMID_COLOR_TARGET_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: HBGI_PYRAMID_DEPTH_TARGET_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            Some(wgpu::ColorTargetState {
                format: HBGI_PYRAMID_COLOR_TARGET_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];
        let base_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Pyramid Base Pipeline"),
                    layout: Some(&base_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_copy_base"),
                        targets: color_targets,
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
        let downsample_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Pyramid Downsample Pipeline"),
                    layout: Some(&downsample_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_downsample"),
                        targets: color_targets,
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        Self {
            base_pipeline,
            downsample_pipeline,
        }
    }
}

struct GiBlurPipeline {
    render_pipeline: wgpu::RenderPipeline,
}
impl GiBlurPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("GI Blur Pipeline Layout"),
                    bind_group_layouts: &[&layouts.gi_blur, &layouts.camera],
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_GI_BLUR_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("GI Blur Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        Self { render_pipeline }
    }
}

struct DeferredLightingPipeline {
    render_pipeline: wgpu::RenderPipeline,
}
impl DeferredLightingPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.deferred_lighting_gbuffer,
            &layouts.deferred_lighting_hbgi,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Deferred Lighting Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let shader_module =
            shader_cache.get(SHADER_DEFERRED_LIGHTING_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Deferred Lighting Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        Self { render_pipeline }
    }
}

struct HbgiReprojectPipeline {
    render_pipeline: wgpu::RenderPipeline,
}
impl HbgiReprojectPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("HBGI Reproject Pipeline Layout"),
                    bind_group_layouts: &[
                        &layouts.camera,
                        &layouts.hbgi_reproject_inputs,
                        &layouts.hbgi_reproject_settings,
                    ],
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_HBGI_REPROJECT_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("HBGI Reproject Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::R32Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        Self { render_pipeline }
    }
}

struct SkyboxPipeline {
    render_pipeline: wgpu::RenderPipeline,
}
impl SkyboxPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let bind_group_layouts = &[&layouts.camera, &layouts.lights];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Skybox Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let shader_module = shader_cache.get(SHADER_SKYBOX_WGSL.to_string(), wgpu_context);
        let render_pipeline =
            wgpu_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Skybox Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        Self { render_pipeline }
    }
}

struct SunShadowPipeline {
    skinned_pipeline: wgpu::RenderPipeline,
    static_pipeline: wgpu::RenderPipeline,
}
impl SunShadowPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let skinned_pipeline = Self::build_skinned_pipeline(wgpu_context, shader_cache, layouts);
        let static_pipeline = Self::build_static_pipeline(wgpu_context, shader_cache, layouts);

        Self {
            skinned_pipeline,
            static_pipeline,
        }
    }

    fn build_skinned_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[&layouts.sun_shadow_matrix, &layouts.bones];
        let pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Sun Shadow Skinned Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader = shader_cache.get(
            SHADER_SUN_SHADOW_SKINNED_VERT_WGSL.to_string(),
            wgpu_context,
        );

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Sun Shadow Skinned Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[SkinnedVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DepthTexture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
    }

    fn build_static_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
    ) -> wgpu::RenderPipeline {
        let bind_group_layouts = &[&layouts.sun_shadow_matrix, &layouts.instance_storage];
        let pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Sun Shadow Static Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader =
            shader_cache.get(SHADER_SUN_SHADOW_STATIC_VERT_WGSL.to_string(), wgpu_context);

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Sun Shadow Static Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[StaticVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DepthTexture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
    }
}

struct SkinnedTransparentPipeline {
    pub pipeline: wgpu::RenderPipeline,
}
impl SkinnedTransparentPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let pipeline = Self::build_pipeline(wgpu_context, shader_cache, layouts);
        Self { pipeline }
    }

    fn build_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[SkinnedVertex::desc()];
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.pbr_material,
            &layouts.motion_bones,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Skinned Transparent Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module = shader_cache.get(
            SHADER_SKINNED_TRANSPARENT_VERT_WGSL.to_string(),
            wgpu_context,
        );
        let fragment_shader_module =
            shader_cache.get(SHADER_PBR_FRAG_WGSL.to_string(), wgpu_context);

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Skinned Transparent Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: Some("vs_main"),
                    buffers: vertex_buffer_layouts,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DepthTexture::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            })
    }
}

struct StaticTransparentPipeline {
    pub pipeline: wgpu::RenderPipeline,
}
impl StaticTransparentPipeline {
    fn new(wgpu_context: &WgpuContext, shader_cache: &mut ShaderCache, layouts: &Layouts) -> Self {
        let pipeline = Self::build_pipeline(wgpu_context, shader_cache, layouts);
        Self { pipeline }
    }

    fn build_pipeline(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
    ) -> wgpu::RenderPipeline {
        let vertex_buffer_layouts = &[StaticVertex::desc()];
        let bind_group_layouts = &[
            &layouts.camera,
            &layouts.lights,
            &layouts.pbr_material,
            &layouts.instance_storage,
        ];
        let render_pipeline_layout =
            wgpu_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Static Transparent Pipeline Layout"),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });
        let vertex_shader_module = shader_cache.get(
            SHADER_STATIC_TRANSPARENT_VERT_WGSL.to_string(),
            wgpu_context,
        );
        let fragment_shader_module =
            shader_cache.get(SHADER_PBR_FRAG_WGSL.to_string(), wgpu_context);

        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Static Transparent Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: Some("vs_main"),
                    buffers: vertex_buffer_layouts,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DepthTexture::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            })
    }
}

struct WorldPipelines {
    g_buffer: GBufferPipeline,
    hbgi: HbgiPipeline,
    hbgi_pyramid: HbgiPyramidPipelines,
    gi_blur: GiBlurPipeline,
    deferred_lighting: DeferredLightingPipeline,
    hbgi_reproject: HbgiReprojectPipeline,
    skybox: SkyboxPipeline,
    sun_shadow: SunShadowPipeline,
    skinned_transparent: SkinnedTransparentPipeline,
    static_transparent: StaticTransparentPipeline,
    post: PostProcessingPipeline,
}
impl WorldPipelines {
    fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
        _resources: &GpuResources,
        g_buffer_targets: &GBufferTargets,
        depth_texture_view: &wgpu::TextureView,
        hbgi_texture: &HbgiTexture,
        hbgi_reproject_prev: &HdrColorTexture,
        hbgi_irradiance_reproject_prev: &HdrColorTexture,
        hbgi_reproject_depth_prev: &FloatPyramidTexture,
        hbgi_reproject_normal_prev: &HdrColorTexture,
        hbgi_reproject_write: &HdrColorTexture,
        hbgi_irradiance_reproject_write: &HdrColorTexture,
        skybox_output: &SkyboxOutputTexture,
        hdr_color: &HdrColorTexture,
    ) -> Self {
        let g_buffer = GBufferPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi = HbgiPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi_pyramid = HbgiPyramidPipelines::new(wgpu_context, shader_cache, layouts);
        let gi_blur = GiBlurPipeline::new(wgpu_context, shader_cache, layouts);
        let deferred_lighting = DeferredLightingPipeline::new(wgpu_context, shader_cache, layouts);
        let hbgi_reproject = HbgiReprojectPipeline::new(wgpu_context, shader_cache, layouts);
        let skybox = SkyboxPipeline::new(wgpu_context, shader_cache, layouts);
        let sun_shadow = SunShadowPipeline::new(wgpu_context, shader_cache, layouts);
        let skinned_transparent =
            SkinnedTransparentPipeline::new(wgpu_context, shader_cache, layouts);
        let static_transparent =
            StaticTransparentPipeline::new(wgpu_context, shader_cache, layouts);
        let post =
            PostProcessingPipeline::new(wgpu_context, shader_cache, skybox_output, hdr_color);

        Self {
            g_buffer,
            hbgi,
            hbgi_pyramid,
            gi_blur,
            deferred_lighting,
            hbgi_reproject,
            skybox,
            sun_shadow,
            skinned_transparent,
            static_transparent,
            post,
        }
    }
}

struct WorldContext {
    resources: GpuResources,
    descriptors: Descriptors,
    pipelines: WorldPipelines,
}

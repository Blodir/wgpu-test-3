use crate::host::{
    renderer::rw_texture::{RWTexture, RWTextureView},
    wgpu_context::WgpuContext,
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
    pub fn new(textures: &ReprojectTextures) -> Self {
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

pub(crate) struct LightingTextures {
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

pub(crate) struct LightingTextureViews {
    pub(crate) lit_hdr: wgpu::TextureView,
    pub(crate) diffuse_radiance_ao: wgpu::TextureView,
}
impl LightingTextureViews {
    fn new(textures: &LightingTextures) -> Self {
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

pub(crate) struct Textures {
    pub(crate) gbuffer: GBufferTextures,
    pub(crate) hbgi: HbgiTextures,
    pub(crate) gi_blur: GiBlurTextures,
    pub(crate) reproject: ReprojectTextures,
    pub(crate) pyramids: MipPyramidTextures,
    pub(crate) sky: wgpu::Texture,
    pub(crate) sun_shadow: SunShadowTexture2,
    pub(crate) lighting_target: LightingTextures,
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
        Self {
            gbuffer,
            hbgi,
            gi_blur,
            reproject,
            pyramids,
            sky,
            sun_shadow,
            lighting_target,
        }
    }
}

pub(crate) struct TextureViews {
    pub(crate) gbuffer: GBufferTextureViews,
    pub(crate) hbgi: HbgiTextureViews,
    pub(crate) gi_blur: GiBlurTextureViews,
    pub(crate) reproject: ReprojectTextureViews,
    pub(crate) pyramids: MipPyramidTextureViews,
    pub(crate) sky: wgpu::TextureView,
    pub(crate) sun_shadow: SunShadowTextureViews,
    pub(crate) lighting_target: LightingTextureViews,
}
impl TextureViews {
    pub fn new(textures: &Textures) -> Self {
        let gbuffer = GBufferTextureViews::new(&textures.gbuffer);
        let hbgi = HbgiTextureViews::new(&textures.hbgi);
        let gi_blur = GiBlurTextureViews::new(&textures.gi_blur);
        let reproject = ReprojectTextureViews::new(&textures.reproject);
        let pyramids = MipPyramidTextureViews::new(&textures.pyramids);
        let sun_shadow = SunShadowTextureViews::new(&textures.sun_shadow.0);
        let lighting_target = LightingTextureViews::new(&textures.lighting_target);

        let sky = textures.sky.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sky Target Texture View"),
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
        }
    }
}

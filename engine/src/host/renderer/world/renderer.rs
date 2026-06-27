use std::{array, time::Instant};

use glam::{Mat4, Vec3};
use wgpu::BindGroup;
use wgpu::util::DeviceExt as _;

use super::super::sampler_cache::SamplerCache;
use super::super::shader_cache::ShaderCache;
use super::anim_pose_store::AnimPoseStore;
use super::attachments::color::HdrColorTexture;
use super::attachments::deferred::{GBufferTargets, HbgiTexture};
use super::attachments::depth::DepthTexture;
use super::attachments::hbgi_pyramid::{FloatPyramidTexture, HbgiPyramidTextures};
use super::attachments::skybox::SkyboxOutputTexture;
use super::attachments::sun_shadow::SunShadowTexture;
use super::bindgroups::bones::BonesBinding;
use super::bindgroups::camera::CameraBinding;
use super::bindgroups::instance_storage::InstanceStorageBinding;
use super::bindgroups::lights::LightsBinding;
use super::bindgroups::material::MaterialBinding;
use super::bindgroups::sun_shadow_matrix::SunShadowMatrixBindGroup;
use super::buffers::skinned_instance::SkinnedInstances;
use super::pipelines::deferred_lighting::DeferredLightingPipeline;
use super::pipelines::g_buffer::GBufferPipeline;
use super::pipelines::gi_blur::GiBlurPipeline;
use super::pipelines::hbgi::HbgiPipeline;
use super::pipelines::hbgi_pyramid::HbgiPyramidPipeline;
use super::pipelines::hbgi_reproject::HbgiReprojectPipeline;
use super::pipelines::post_processing::PostProcessingPipeline;
use super::pipelines::skinned_transparent::SkinnedTransparentPipeline;
use super::pipelines::skybox::SkyboxPipeline;
use super::pipelines::sun_shadow::SunShadowPipeline;
use super::prepare::camera::prepare_camera;
use super::prepare::lights::prepare_lights;
use super::prepare::mesh::{resolve_skinned_draw, PassDrawContext};
use super::prepare::sun_shadow::prepare_sun_shadow;

use crate::host::assets::io::asset_formats::materialfile;
use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
use crate::host::renderer::rw_buffer::RWBuffer;
use crate::host::renderer::rw_texture::{RWTexture, RWTextureView};
use crate::host::renderer::{HbgiOptions, RendererOptions};
use crate::host::wgpu_context::{self, WgpuContext};
use crate::host::world::buffers::static_instance::StaticInstances;
use crate::host::world::pipelines::static_transparent::StaticTransparentPipeline;
use crate::host::world::prepare::mesh::resolve_static_draw;
use crate::host::world::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT;
use crate::{fixed_snapshot::FixedSnapshotGuard, var_snapshot::CameraSnapshotPair};

pub struct Layouts {
    pub camera: wgpu::BindGroupLayout,
    pub lights: wgpu::BindGroupLayout,
    pub sun_shadow_matrix: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub bones: wgpu::BindGroupLayout,
    pub motion_bones: wgpu::BindGroupLayout,
    pub instance_storage: wgpu::BindGroupLayout,
    pub pbr_material: wgpu::BindGroupLayout,
}
impl Layouts {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let camera = wgpu_context
            .device
            .create_bind_group_layout(&CameraBinding::desc());
        let lights = wgpu_context
            .device
            .create_bind_group_layout(&LightsBinding::desc());
        let sun_shadow_matrix = wgpu_context
            .device
            .create_bind_group_layout(&SunShadowMatrixBindGroup::desc());
        let material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());
        let bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBinding::desc());
        let motion_bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBinding::motion_desc());
        let instance_storage = wgpu_context
            .device
            .create_bind_group_layout(&InstanceStorageBinding::desc());
        let pbr_material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());

        Self {
            camera,
            lights,
            sun_shadow_matrix,
            material,
            bones,
            motion_bones,
            instance_storage,
            pbr_material,
        }
    }
}

pub struct UploadMaterialRequest<'a> {
    pub manifest: &'a materialfile::Material,
    pub normal_texture: &'a Option<TextureRenderId>,
    pub occlusion_texture: &'a Option<TextureRenderId>,
    pub emissive_texture: &'a Option<TextureRenderId>,
    pub base_color_texture: &'a Option<TextureRenderId>,
    pub metallic_roughness_texture: &'a Option<TextureRenderId>,
}

struct WorldAttachments {
    skybox_output: SkyboxOutputTexture,
    depth_texture: DepthTexture,
    hdr_color: HdrColorTexture,
    sun_shadow: SunShadowTexture,
}
impl WorldAttachments {
    fn new(wgpu_context: &WgpuContext) -> Self {
        Self {
            skybox_output: SkyboxOutputTexture::new(
                &wgpu_context.device,
                &wgpu_context.surface_config,
            ),
            depth_texture: DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config),
            hdr_color: HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config),
            sun_shadow: SunShadowTexture::new(&wgpu_context.device),
        }
    }

    fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.skybox_output =
            SkyboxOutputTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.hdr_color = HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
    }
}

struct WorldBindGroups {
    layouts: Layouts,
    bones: BonesBinding,
    camera: CameraBinding,
    lights: LightsBinding,
    static_instances: InstanceStorageBinding,
    sun_shadow_matrices: [SunShadowMatrixBindGroup; SUN_SHADOW_MAX_CASCADE_COUNT],
}
impl WorldBindGroups {
    fn new(
        wgpu_context: &WgpuContext,
        placeholders: &PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        sampler_cache: &mut SamplerCache,
        render_resources: &RenderAssetStore,
        sun_shadow_view: &wgpu::TextureView,
        skinned_instance_buffer: &wgpu::Buffer,
        prev_skinned_instance_buffer: &wgpu::Buffer,
        static_instance_buffer: &wgpu::Buffer,
    ) -> Self {
        let layouts = Layouts::new(wgpu_context);
        let lights = LightsBinding::new(
            render_resources,
            brdf_lut,
            sampler_cache,
            placeholders,
            wgpu_context,
            &layouts.lights,
            sun_shadow_view,
        );
        let sun_shadow_matrices = array::from_fn(|_| {
            SunShadowMatrixBindGroup::new(&wgpu_context.device, &layouts.sun_shadow_matrix)
        });
        let camera = CameraBinding::new(&wgpu_context.device, &layouts.camera);
        let bones = BonesBinding::new(
            &layouts.bones,
            &layouts.motion_bones,
            skinned_instance_buffer,
            prev_skinned_instance_buffer,
            &wgpu_context.device,
        );
        let static_instances = InstanceStorageBinding::new(
            static_instance_buffer,
            &layouts.instance_storage,
            &wgpu_context.device,
        );
        Self {
            layouts,
            bones,
            camera,
            lights,
            static_instances,
            sun_shadow_matrices,
        }
    }
}

struct WorldPipelines {
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
        attachments: &WorldAttachments,
    ) -> Self {
        let skybox =
            SkyboxPipeline::new(wgpu_context, shader_cache, &layouts.camera, &layouts.lights);
        let sun_shadow = SunShadowPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.sun_shadow_matrix,
            &layouts.bones,
            &layouts.instance_storage,
        );
        let skinned_transparent = SkinnedTransparentPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
            &layouts.motion_bones,
        );
        let static_transparent = StaticTransparentPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
            &layouts.instance_storage,
        );
        let post = PostProcessingPipeline::new(
            wgpu_context,
            shader_cache,
            &attachments.skybox_output,
            &attachments.hdr_color,
        );

        Self {
            skybox,
            sun_shadow,
            skinned_transparent,
            static_transparent,
            post,
        }
    }
}

// TODO move all resources here: ------------
pub struct GBufferTextures {
    pub albedo_ao: wgpu::Texture,
    pub normal_roughness: wgpu::Texture,
    pub emissive_metallic: wgpu::Texture,
    pub motion_vectors: wgpu::Texture,
    pub depth: wgpu::Texture,
}
impl GBufferTextures {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(
        wgpu_context: &WgpuContext,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let albedo_ao = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("GBuffer: Albedo + AO Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let normal_roughness = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("GBuffer: Normal + Roughness Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let emissive_metallic = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("GBuffer: Emissive + Metallic Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let motion_vectors = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("GBuffer: Motion Vectors Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("GBuffer: Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
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
        let albedo_ao = textures.albedo_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("GBuffer: Albedo + AO Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let normal_roughness = textures.albedo_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("GBuffer: Normal + Roughness Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let emissive_metallic = textures.albedo_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("GBuffer: Emissive + Metallic Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let motion_vectors = textures.albedo_ao.create_view(&wgpu::TextureViewDescriptor {
            label: Some("GBuffer: Motion Vectors Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let depth = textures.depth.create_view(&wgpu::TextureViewDescriptor::default());

        Self { albedo_ao, normal_roughness, emissive_metallic, motion_vectors, depth }
    }
}

pub struct MipPyramidTextures {
    pub diffuse_radiance_ao_pyramid: wgpu::Texture,
    pub depth_pyramid: wgpu::Texture,
    pub normal_pyramid: wgpu::Texture,
    pub mip_level_count: u32,
}
impl MipPyramidTextures {
    pub fn new(
        wgpu_context: &WgpuContext,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let mip_level_count = surface_config.width.max(surface_config.height).ilog2() + 1;
        let diffuse_radiance_ao_pyramid = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("Diffuse Radiance + AO Pyramid Texture"),
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_pyramid = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("Depth Pyramid Texture"),
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let normal_pyramid = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("Normal Pyramid Texture"),
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self { diffuse_radiance_ao_pyramid, depth_pyramid, normal_pyramid, mip_level_count }
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
        let diffuse_radiance_ao_pyramid = textures.diffuse_radiance_ao_pyramid.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Diffuse Radiance + AO Pyramid Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let diffuse_radiance_ao_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures.diffuse_radiance_ao_pyramid.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("Diffuse Radiance + AO Pyramid Mip {mip_level} Texture View")),
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

        let depth_pyramid = textures.depth_pyramid.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth Pyramid Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let depth_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures.depth_pyramid.create_view(&wgpu::TextureViewDescriptor {
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

        let normal_pyramid = textures.normal_pyramid.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Normal Pyramid Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });
        let normal_mips = (0..mip_level_count)
            .map(|mip_level| {
                textures.normal_pyramid.create_view(&wgpu::TextureViewDescriptor {
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

        Self { diffuse_radiance_ao_pyramid, depth_pyramid, normal_pyramid, diffuse_radiance_ao_mips, depth_mips, normal_mips }
    }
}

pub struct HbgiTextures {
    pub bent_ao: wgpu::Texture,
    pub near_field_irradiance: wgpu::Texture,
}
impl HbgiTextures {
    pub const SCALE_DIVISOR: u32 = 2;
    pub fn new(
        wgpu_context: &WgpuContext,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let width = (surface_config.width / Self::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / Self::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let bent_ao = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("HBGI: Bent Normal + AO Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let near_field_irradiance = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("HBGI: Near Field Irradiance Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self { bent_ao, near_field_irradiance }
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
        let near_field_irradiance = textures.near_field_irradiance.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HBGI: Near Field Irradiance Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });

        Self { bent_ao, near_field_irradiance }
    }
}

pub struct GiBlurTextures {
    pub bent_ao: wgpu::Texture,
    pub near_field_irradiance: wgpu::Texture,
}
impl GiBlurTextures {
    pub fn new(
        wgpu_context: &WgpuContext,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let width = (surface_config.width / HbgiTextures::SCALE_DIVISOR).max(1);
        let height = (surface_config.height / HbgiTextures::SCALE_DIVISOR).max(1);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let bent_ao = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("GI Blur: Bent Normal + AO Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let near_field_irradiance = wgpu_context.device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("GI Blur: Near Field Irradiance Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self { bent_ao, near_field_irradiance }
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
        let near_field_irradiance = textures.near_field_irradiance.create_view(&wgpu::TextureViewDescriptor {
            label: Some("GI Blur: Near Field Irradiance Texture View"),
            ..wgpu::TextureViewDescriptor::default()
        });

        Self { bent_ao, near_field_irradiance }
    }
}

pub struct SunShadowTexture2(wgpu::Texture);
impl SunShadowTexture2 {
    pub const SHADOW_MAP_SIZE: u32 = crate::host::world::sun_shadow::SUN_SHADOW_MAP_SIZE;
    pub const CASCADE_COUNT: u32 =
        crate::host::world::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT as u32;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let tex = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
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

        let cascades = (0..SunShadowTexture2::CASCADE_COUNT).map(|idx| {
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
        }).collect();

        Self { array, cascades }
    }
}

pub struct ReprojectTextures {
    pub near_field_irradiance: RWTexture,
    pub bent_ao: RWTexture,
}
impl ReprojectTextures {
    pub fn new(
        wgpu_context: &WgpuContext,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Self {
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context
        );
        let bent_ao = RWTexture::new(
            wgpu::TextureDescriptor {
                label: Some("Reproject: Bent Normals + AO Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu_context
        );

        Self { near_field_irradiance, bent_ao }
    }
}

pub struct ReprojectTextureViews {
    pub near_field_irradiance: RWTextureView,
    pub bent_ao: RWTextureView,
}
impl ReprojectTextureViews {
    pub fn new(textures: ReprojectTextures) -> Self {
        let near_field_irradiance =
            RWTextureView::new(
                &textures.near_field_irradiance,
                &wgpu::TextureViewDescriptor {
                    label: Some("Reproject: Near Field Irradiance Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                }
            );
        let bent_ao =
            RWTextureView::new(
                &textures.near_field_irradiance,
                &wgpu::TextureViewDescriptor {
                    label: Some("Reproject: Bent Normals + AO Texture View"),
                    ..wgpu::TextureViewDescriptor::default()
                }
            );
        Self { near_field_irradiance, bent_ao }
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
        let lit_hdr = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Lighting: Lit HDR Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let diffuse_radiance_ao = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Lighting: Diffuse Radiance + AO Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self { lit_hdr, diffuse_radiance_ao }
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
        let diffuse_radiance_ao = textures.lit_hdr.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Lighting: Diffuse Radiance + AO Texture View"),
            ..Default::default()
        });

        Self { lit_hdr, diffuse_radiance_ao }
    }
}

struct Textures {
    gbuffer: GBufferTextures,
    hbgi: HbgiTextures,
    gi_blur: GiBlurTextures,
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
        let pyramids = MipPyramidTextures::new(wgpu_context, surface_config);
        let sun_shadow = SunShadowTexture2::new(wgpu_context);
        let lighting_target = LightingTextures::new(wgpu_context, surface_config);

        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let sky = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Sky Target Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let post_process_target = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Post Process Target Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self { gbuffer, hbgi, gi_blur, pyramids, sky, sun_shadow, post_process_target, lighting_target }
    }
}

struct TextureViews {
    gbuffer: GBufferTextureViews,
    hbgi: HbgiTextureViews,
    gi_blur: GiBlurTextureViews,
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
        let pyramids = MipPyramidTextureViews::new(&textures.pyramids);
        let sun_shadow = SunShadowTextureViews::new(&textures.sun_shadow.0);
        let lighting_target = LightingTextureViews::new(&textures.lighting_target);

        let sky = textures.sky.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Sky Target Texture View"),
            ..Default::default()
        });
        let post_process_target = textures.post_process_target.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Post Process Target Texture View"),
            ..Default::default()
        });

        Self { gbuffer, hbgi, gi_blur, pyramids, sky, sun_shadow, lighting_target, post_process_target }
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
        let inverse_view_proj =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
        queue.write_buffer(&self.inverse_view_proj, 0, bytemuck::cast_slice(inverse_view_proj));
        queue.write_buffer(&self.forward, 0, bytemuck::cast_slice(forward));
        queue.write_buffer(&self.view_rotation, 0, bytemuck::cast_slice(view_rotation));
        queue.write_buffer(&self.prev_view_proj, 0, bytemuck::cast_slice(prev_view_proj));
    }
}

pub struct LightsBuffers {
    pub sun_direction: wgpu::Buffer,
    pub sun_color: wgpu::Buffer,
    pub sun_shadow: wgpu::Buffer,
    pub environment_map_intensity: wgpu::Buffer,
    pub point_light_count: wgpu::Buffer,
    pub point_light_positions_ranges: wgpu::Buffer,
    pub point_light_colors_intensities: wgpu::Buffer,
}
impl LightsBuffers {
    pub fn new(device: &wgpu::Device) -> Self {
        todo!()
    }
}

struct Buffers {
    pub bones: RWBuffer,
    pub camera: CameraBuffers,
    pub hbgi_settings: wgpu::Buffer,
    pub skinned_instances: RWBuffer,
    pub static_instances: wgpu::Buffer,
    pub lights: LightsBuffers,
}

struct Samplers {
    sampler_cache: SamplerCache,
}

struct GpuResources {
    textures: Textures,
    buffers: Buffers,
    samplers: Samplers,
}

struct BindGroups {
    bones: BindGroup,
    motion_bones: BindGroup,
    camera: BindGroup,
    g_buffer: BindGroup,
    hbgi_settings: BindGroup,
    lights: BindGroup,
    post_processing: BindGroup,
    sun_shadow_matrix: BindGroup,
}

struct Descriptors {
    bind_groups: BindGroups,
    bind_group_layouts: Layouts,
    texture_views: TextureViews,
}

struct WorldContext {
    resources: GpuResources,
    descriptors: Descriptors,
    pipelines: WorldPipelines,
}

// -------------------------------------------

pub struct WorldRenderer {
    attachments: WorldAttachments,
    bind_groups: WorldBindGroups,
    pipelines: WorldPipelines,
    hbgi_options: Option<HbgiOptions>,
    g_buffer_targets: GBufferTargets,
    hbgi_texture: HbgiTexture,
    gi_source_write: HdrColorTexture,
    gi_source_prev: HdrColorTexture,
    hbgi_reproject_write: HdrColorTexture,
    hbgi_reproject_prev: HdrColorTexture,
    hbgi_irradiance_reproject_write: HdrColorTexture,
    hbgi_irradiance_reproject_prev: HdrColorTexture,
    hbgi_reproject_depth_write: FloatPyramidTexture,
    hbgi_reproject_depth_prev: FloatPyramidTexture,
    hbgi_reproject_normal_write: HdrColorTexture,
    hbgi_reproject_normal_prev: HdrColorTexture,
    hbgi_pyramids: HbgiPyramidTextures,
    g_buffer_pipeline: GBufferPipeline,
    hbgi_pipeline: HbgiPipeline,
    hbgi_pyramid_pipeline: HbgiPyramidPipeline,
    gi_blur_pipeline: GiBlurPipeline,
    deferred_lighting_pipeline: DeferredLightingPipeline,
    hbgi_reproject_pipeline: HbgiReprojectPipeline,
    hbgi_reproject_valid: bool,
    placeholders: PlaceholderTextureIds,
    brdf_lut: TextureRenderId,
    skinned_instances: SkinnedInstances,
    static_instances: StaticInstances,
    pose_storage: AnimPoseStore,
    prev_motion_view_proj: Option<Mat4>,
}
impl WorldRenderer {
    fn refresh_temporal_bind_groups(&mut self, device: &wgpu::Device) {
        self.hbgi_reproject_pipeline.update_input_bindgroup(
            device,
            &self.g_buffer_targets.motion_vectors,
            &self.hbgi_texture.view,
            &self.hbgi_texture.irradiance_view,
            &self.hbgi_reproject_prev,
            &self.hbgi_irradiance_reproject_prev,
            &self.attachments.depth_texture.view,
            &self.g_buffer_targets.normal_roughness,
            &self.hbgi_reproject_depth_prev,
            &self.hbgi_reproject_normal_prev,
        );
        self.gi_blur_pipeline.update_input_bindgroup(
            device,
            &self.g_buffer_targets,
            &self.attachments.depth_texture.view,
            &self.hbgi_reproject_write,
            &self.hbgi_irradiance_reproject_write,
            &self.hbgi_texture,
        );
        self.hbgi_pipeline.update_input_bindgroups(
            device,
            &self.hbgi_pyramids,
            &self.hbgi_options.unwrap_or_default(),
            0,
        );
    }

    fn clear_temporal_inputs(&self, encoder: &mut wgpu::CommandEncoder) {
        for (label, view, clear_color) in [
            (
                "Previous GI Source Clear Pass",
                &self.gi_source_prev.view,
                wgpu::Color::TRANSPARENT,
            ),
            (
                "Previous HBGI Reproject Clear Pass",
                &self.hbgi_reproject_prev.view,
                wgpu::Color::TRANSPARENT,
            ),
            (
                "Previous HBGI Irradiance Reproject Clear Pass",
                &self.hbgi_irradiance_reproject_prev.view,
                wgpu::Color::TRANSPARENT,
            ),
            (
                "Previous HBGI Reproject Depth Clear Pass",
                &self.hbgi_reproject_depth_prev.view,
                wgpu::Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            ),
            (
                "Previous HBGI Reproject Normal Clear Pass",
                &self.hbgi_reproject_normal_prev.view,
                wgpu::Color::TRANSPARENT,
            ),
        ] {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
    }

    fn rotate_temporal_buffers(&mut self, device: &wgpu::Device) {
        std::mem::swap(&mut self.gi_source_write, &mut self.gi_source_prev);
        std::mem::swap(
            &mut self.hbgi_reproject_write,
            &mut self.hbgi_reproject_prev,
        );
        std::mem::swap(
            &mut self.hbgi_irradiance_reproject_write,
            &mut self.hbgi_irradiance_reproject_prev,
        );
        std::mem::swap(
            &mut self.hbgi_reproject_depth_write,
            &mut self.hbgi_reproject_depth_prev,
        );
        std::mem::swap(
            &mut self.hbgi_reproject_normal_write,
            &mut self.hbgi_reproject_normal_prev,
        );
        self.refresh_temporal_bind_groups(device);
    }

    fn render_deferred_opaque<'a>(
        &mut self,
        skinned_opaque_pass: &'a PassDrawContext<'a>,
        static_opaque_pass: &'a PassDrawContext<'a>,
        encoder: &mut wgpu::CommandEncoder,
        render_resources: &'a RenderAssetStore,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prev_inverse_view_proj: &Mat4,
        frame_idx: u32,
    ) {
        self.g_buffer_pipeline.render_skinned_opaque(
            skinned_opaque_pass,
            encoder,
            &self.g_buffer_targets,
            &self.attachments.depth_texture.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
            &self.bind_groups.bones.motion_bind_group,
            render_resources,
        );
        self.g_buffer_pipeline.render_static_opaque(
            static_opaque_pass,
            encoder,
            &self.g_buffer_targets,
            &self.attachments.depth_texture.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
            &self.bind_groups.static_instances.bind_group,
            render_resources,
        );
        if !self.hbgi_reproject_valid {
            self.clear_temporal_inputs(encoder);
        }
        if self.hbgi_options.is_some() {
            self.hbgi_pyramid_pipeline.generate(
                device,
                encoder,
                &self.gi_source_prev,
                &self.attachments.depth_texture.view,
                &self.g_buffer_targets,
                &self.hbgi_pyramids,
            );
            self.hbgi_pipeline.update_input_bindgroups(
                device,
                &self.hbgi_pyramids,
                &self.hbgi_options.unwrap_or_default(),
                frame_idx,
            );
            self.hbgi_pipeline.render(
                encoder,
                &self.hbgi_texture,
                &self.bind_groups.camera.bind_group,
            );
            self.hbgi_reproject_pipeline
                .update_temporal_state(queue, prev_inverse_view_proj);
            self.hbgi_reproject_pipeline.render(
                encoder,
                &self.bind_groups.camera.bind_group,
                &self.hbgi_reproject_write.view,
                &self.hbgi_reproject_depth_write.view,
                &self.hbgi_reproject_normal_write.view,
                &self.hbgi_irradiance_reproject_write.view,
            );
            self.gi_blur_pipeline.render(
                encoder,
                &self.hbgi_texture,
                &self.bind_groups.camera.bind_group,
            );
            self.hbgi_reproject_valid = true;
        } else {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HBGI Disabled Clear Pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.hbgi_texture.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.hbgi_texture.irradiance_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.hbgi_texture.blurred_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 1.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.hbgi_texture.blurred_irradiance_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.hbgi_reproject_valid = false;
        }
        self.deferred_lighting_pipeline.render(
            encoder,
            &self.attachments.hdr_color.view,
            &self.gi_source_write.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
        );
        self.rotate_temporal_buffers(device);
    }

    fn resize_deferred(&mut self, wgpu_context: &WgpuContext) {
        self.g_buffer_targets =
            GBufferTargets::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_texture = HbgiTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.gi_source_write =
            HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.gi_source_prev =
            HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_reproject_write =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_reproject_prev =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_irradiance_reproject_write =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_irradiance_reproject_prev =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_reproject_depth_write = FloatPyramidTexture::new_half_res(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            "HBGI Reproject Depth Write",
        );
        self.hbgi_reproject_depth_prev = FloatPyramidTexture::new_half_res(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            "HBGI Reproject Depth Previous",
        );
        self.hbgi_reproject_normal_write =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_reproject_normal_prev =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_pyramids =
            HbgiPyramidTextures::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.hbgi_pipeline.update_input_bindgroups(
            &wgpu_context.device,
            &self.hbgi_pyramids,
            &self.hbgi_options.unwrap_or_default(),
            0,
        );
        self.gi_blur_pipeline.update_input_bindgroup(
            &wgpu_context.device,
            &self.g_buffer_targets,
            &self.attachments.depth_texture.view,
            &self.hbgi_reproject_write,
            &self.hbgi_irradiance_reproject_write,
            &self.hbgi_texture,
        );
        self.deferred_lighting_pipeline.update_input_bindgroup(
            &wgpu_context.device,
            &self.g_buffer_targets,
            &self.attachments.depth_texture.view,
            &self.hbgi_texture,
        );
        self.hbgi_reproject_pipeline.update_input_bindgroup(
            &wgpu_context.device,
            &self.g_buffer_targets.motion_vectors,
            &self.hbgi_texture.view,
            &self.hbgi_texture.irradiance_view,
            &self.hbgi_reproject_prev,
            &self.hbgi_irradiance_reproject_prev,
            &self.attachments.depth_texture.view,
            &self.g_buffer_targets.normal_roughness,
            &self.hbgi_reproject_depth_prev,
            &self.hbgi_reproject_normal_prev,
        );
        self.hbgi_reproject_valid = false;
    }

    pub fn new(
        wgpu_context: &WgpuContext,
        placeholders: PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        sampler_cache: &mut SamplerCache,
        shader_cache: &mut ShaderCache,
        render_resources: &RenderAssetStore,
        options: RendererOptions,
    ) -> Self {
        let attachments = WorldAttachments::new(wgpu_context);
        let skinned_instances = SkinnedInstances::new(wgpu_context);
        let static_instances = StaticInstances::new(wgpu_context);
        let bind_groups = WorldBindGroups::new(
            wgpu_context,
            &placeholders,
            brdf_lut,
            sampler_cache,
            render_resources,
            &attachments.sun_shadow.array_view,
            &skinned_instances.buffer,
            &skinned_instances.prev_buffer,
            &static_instances.buffer,
        );
        let pose_storage = AnimPoseStore::new();
        let pipelines = WorldPipelines::new(
            wgpu_context,
            shader_cache,
            &bind_groups.layouts,
            &attachments,
        );
        let hbgi_options = options.hbgi;
        let g_buffer_targets =
            GBufferTargets::new(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_texture = HbgiTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let gi_source_write =
            HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let gi_source_prev =
            HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_reproject_write =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_reproject_prev =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_irradiance_reproject_write =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_irradiance_reproject_prev =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_reproject_depth_write = FloatPyramidTexture::new_half_res(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            "HBGI Reproject Depth Write",
        );
        let hbgi_reproject_depth_prev = FloatPyramidTexture::new_half_res(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            "HBGI Reproject Depth Previous",
        );
        let hbgi_reproject_normal_write =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_reproject_normal_prev =
            HdrColorTexture::new_half_res(&wgpu_context.device, &wgpu_context.surface_config);
        let hbgi_pyramids =
            HbgiPyramidTextures::new(&wgpu_context.device, &wgpu_context.surface_config);
        let g_buffer_pipeline = GBufferPipeline::new(
            wgpu_context,
            shader_cache,
            &bind_groups.layouts.material,
            &bind_groups.layouts.camera,
            &bind_groups.layouts.lights,
            &bind_groups.layouts.motion_bones,
            &bind_groups.layouts.instance_storage,
        );
        let hbgi_pipeline = HbgiPipeline::new(
            wgpu_context,
            shader_cache,
            &bind_groups.layouts.camera,
            &hbgi_pyramids,
            &hbgi_options.unwrap_or_default(),
            0,
        );
        let hbgi_pyramid_pipeline = HbgiPyramidPipeline::new(wgpu_context, shader_cache);
        let gi_blur_pipeline = GiBlurPipeline::new(
            wgpu_context,
            shader_cache,
            &bind_groups.layouts.camera,
            &g_buffer_targets,
            &attachments.depth_texture.view,
            &hbgi_reproject_write,
            &hbgi_irradiance_reproject_write,
            &hbgi_texture,
        );
        let deferred_lighting_pipeline = DeferredLightingPipeline::new(
            wgpu_context,
            shader_cache,
            &bind_groups.layouts.camera,
            &bind_groups.layouts.lights,
            &g_buffer_targets,
            &attachments.depth_texture.view,
            &hbgi_texture,
        );
        let hbgi_reproject_pipeline = HbgiReprojectPipeline::new(
            wgpu_context,
            shader_cache,
            &bind_groups.layouts.camera,
            &g_buffer_targets.motion_vectors,
            &hbgi_texture.view,
            &hbgi_texture.irradiance_view,
            &hbgi_reproject_prev,
            &hbgi_irradiance_reproject_prev,
            &attachments.depth_texture.view,
            &g_buffer_targets.normal_roughness,
            &hbgi_reproject_depth_prev,
            &hbgi_reproject_normal_prev,
        );

        Self {
            attachments,
            bind_groups,
            pipelines,
            hbgi_options,
            g_buffer_targets,
            hbgi_texture,
            gi_source_write,
            gi_source_prev,
            hbgi_reproject_write,
            hbgi_reproject_prev,
            hbgi_irradiance_reproject_write,
            hbgi_irradiance_reproject_prev,
            hbgi_reproject_depth_write,
            hbgi_reproject_depth_prev,
            hbgi_reproject_normal_write,
            hbgi_reproject_normal_prev,
            hbgi_pyramids,
            g_buffer_pipeline,
            hbgi_pipeline,
            hbgi_pyramid_pipeline,
            gi_blur_pipeline,
            deferred_lighting_pipeline,
            hbgi_reproject_pipeline,
            hbgi_reproject_valid: false,
            skinned_instances,
            placeholders,
            brdf_lut,
            static_instances,
            pose_storage,
            prev_motion_view_proj: None,
        }
    }

    pub fn apply_options(
        &mut self,
        options: RendererOptions,
        _wgpu_context: &WgpuContext,
        _shader_cache: &mut ShaderCache,
    ) {
        self.hbgi_options = options.hbgi;
        self.hbgi_pipeline.update_input_bindgroups(
            &_wgpu_context.device,
            &self.hbgi_pyramids,
            &self.hbgi_options.unwrap_or_default(),
            0,
        );
        self.hbgi_reproject_valid = false;
    }

    pub fn receive_poses(
        &mut self,
        anim_pose_task_results: crate::workers::anim_pose::PoseJobResult,
    ) {
        self.pose_storage.receive_poses(anim_pose_task_results);
    }

    pub fn render(
        &mut self,
        snaps: &FixedSnapshotGuard,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        sampler_cache: &mut SamplerCache,
        frame_idx: u32,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        camera_pair: &CameraSnapshotPair,
    ) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(snaps.curr_timestamp);
        let interval = snaps
            .curr_timestamp
            .saturating_duration_since(snaps.prev_timestamp);
        let t = if interval.is_zero() {
            1.0
        } else {
            (elapsed.as_secs_f32() / interval.as_secs_f32()).clamp(0.0, 1.0)
        };
        let prepared_camera = prepare_camera(
            &mut self.bind_groups.camera,
            camera_pair,
            now,
            self.prev_motion_view_proj.as_ref(),
            &wgpu_context.queue,
            &wgpu_context.surface_config,
        );
        let prev_motion_view_proj = self
            .prev_motion_view_proj
            .unwrap_or(prepared_camera.view_proj);
        let prev_inverse_view_proj = prev_motion_view_proj.inverse();
        prepare_lights(
            &snaps,
            &mut self.bind_groups.lights,
            self.brdf_lut,
            render_resources,
            sampler_cache,
            wgpu_context,
            &self.bind_groups.layouts.lights,
            &self.attachments.sun_shadow.array_view,
        );
        let prepared_sun_shadow = prepare_sun_shadow(
            &prepared_camera,
            snaps.curr.lights.sun.direction,
            &self.bind_groups.lights,
            &wgpu_context.queue,
        );

        self.pipelines.skybox.render(
            encoder,
            &self.attachments.skybox_output.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
        );

        let (skinned_opaque_pass, skinned_transparent_pass) = resolve_skinned_draw(
            &mut self.bind_groups.bones,
            &self.bind_groups.layouts.bones,
            &self.bind_groups.layouts.motion_bones,
            &mut self.skinned_instances,
            render_resources,
            &snaps,
            t,
            &wgpu_context.device,
            &wgpu_context.queue,
            &mut self.pose_storage,
            frame_idx,
        );
        let (static_opaque_pass, static_transparent_pass) = resolve_static_draw(
            &mut self.static_instances,
            render_resources,
            &snaps,
            t,
            &wgpu_context.device,
            &wgpu_context.queue,
            &mut self.pose_storage,
            frame_idx,
        );
        self.bind_groups.static_instances.update(
            &self.static_instances.buffer,
            &self.bind_groups.layouts.instance_storage,
            &wgpu_context.device,
        );

        for ((cascade, cascade_view), cascade_bind_group) in prepared_sun_shadow
            .cascades
            .iter()
            .take(prepared_sun_shadow.cascade_count)
            .zip(self.attachments.sun_shadow.cascade_views.iter())
            .zip(self.bind_groups.sun_shadow_matrices.iter())
        {
            cascade_bind_group.update(&cascade.light_view_proj, &wgpu_context.queue);
            self.pipelines.sun_shadow.render(
                &skinned_opaque_pass,
                &static_opaque_pass,
                encoder,
                cascade_view,
                &cascade_bind_group.bind_group,
                &self.bind_groups.bones.bind_group,
                &self.bind_groups.static_instances.bind_group,
                render_resources,
            );
        }

        self.render_deferred_opaque(
            &skinned_opaque_pass,
            &static_opaque_pass,
            encoder,
            render_resources,
            &wgpu_context.device,
            &wgpu_context.queue,
            &prev_inverse_view_proj,
            frame_idx,
        );

        self.pipelines.skinned_transparent.render(
            &skinned_transparent_pass,
            encoder,
            &self.attachments.hdr_color.view,
            &self.attachments.depth_texture.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
            &self.bind_groups.bones.motion_bind_group,
            render_resources,
        );

        self.pipelines.static_transparent.render(
            &static_transparent_pass,
            encoder,
            &self.attachments.hdr_color.view,
            &self.attachments.depth_texture.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
            &self.bind_groups.static_instances.bind_group,
            render_resources,
        );

        self.pipelines.post.render(encoder, output_view);
        self.prev_motion_view_proj = Some(prepared_camera.view_proj);
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.attachments.resize(wgpu_context);
        self.pipelines.post.update_input_bindgroup(
            &wgpu_context.device,
            &self.attachments.skybox_output,
            &self.attachments.hdr_color,
        );
        self.resize_deferred(wgpu_context);
    }

    pub fn upload_material(
        &mut self,
        request: UploadMaterialRequest<'_>,
        render_resources: &RenderAssetStore,
        sampler_cache: &mut SamplerCache,
        wgpu_context: &WgpuContext,
    ) -> Result<MaterialBinding, ()> {
        let textures_gpu = &render_resources.textures;
        let base_color_view = &textures_gpu
            .get(
                request
                    .base_color_texture
                    .unwrap_or(self.placeholders.base_color)
                    .into(),
            )
            .unwrap()
            .texture_view;
        let emissive_view = &textures_gpu
            .get(
                request
                    .emissive_texture
                    .unwrap_or(self.placeholders.emissive)
                    .into(),
            )
            .unwrap()
            .texture_view;
        let metallic_roughness_view = &textures_gpu
            .get(
                request
                    .metallic_roughness_texture
                    .unwrap_or(self.placeholders.metallic_roughness)
                    .into(),
            )
            .unwrap()
            .texture_view;
        let normal_view = &textures_gpu
            .get(
                request
                    .normal_texture
                    .unwrap_or(self.placeholders.normals)
                    .into(),
            )
            .unwrap()
            .texture_view;
        let occlusion_view = &textures_gpu
            .get(
                request
                    .occlusion_texture
                    .unwrap_or(self.placeholders.occlusion)
                    .into(),
            )
            .unwrap()
            .texture_view;

        Ok(MaterialBinding::upload(
            request.manifest,
            base_color_view,
            emissive_view,
            metallic_roughness_view,
            normal_view,
            occlusion_view,
            &self.bind_groups.layouts.material,
            wgpu_context,
            sampler_cache,
        ))
    }
}

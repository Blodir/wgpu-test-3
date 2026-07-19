use std::{array, time::Instant};

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt as _;
use wgpu::BindGroup;

use super::super::sampler_cache::SamplerCache;
use super::super::shader_cache::ShaderCache;
use super::anim_pose_store::AnimPoseStore;
use super::attachments::color::HdrColorTexture;
use super::attachments::deferred::{GBufferTargets, HbgiTexture};
use super::attachments::depth::DepthTexture;
use super::attachments::hbgi_pyramid::{FloatPyramidTexture, HbgiPyramidTextures};
use super::attachments::skybox::SkyboxOutputTexture;
use super::attachments::sun_shadow::SunShadowTexture;
use super::bg_layouts::BGLayouts;
use super::bindgroups::bones::BonesBinding;
use super::bindgroups::camera::CameraBinding;
use super::bindgroups::instance_storage::InstanceStorageBinding;
use super::bindgroups::lights::LightsBinding;
use super::bindgroups::material::MaterialBinding;
use super::bindgroups::sun_shadow_matrix::SunShadowMatrixBindGroup;
use super::buffers::skinned_instance::SkinnedInstances;
use super::gpu_context::WorldGpuContext;
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
use super::prepare::mesh::{resolve_skinned_draw, PassDrawContext};
use super::prepare::sun_shadow::prepare_sun_shadow;
use super::{
    bindgroups::hbgi_settings::HbgiSettingsUniform,
    passes::{
        render_deferred_lighting_pass, render_gbuffer_skinned_opaque_pass,
        render_gbuffer_static_opaque_pass, render_gi_blur_pass, render_hbgi_pass,
        render_hbgi_pyramid_pass, render_hbgi_reproject_pass, render_post_processing_pass,
        render_skinned_transparent_pass, render_skybox_pass, render_static_transparent_pass,
        render_sun_shadow_pass,
    },
};

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
    layouts: BGLayouts,
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
        let layouts = BGLayouts::new(wgpu_context);
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
        layouts: &BGLayouts,
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

pub struct WorldRenderer {
    attachments: WorldAttachments,
    bind_groups: WorldBindGroups,
    gpu_context: WorldGpuContext,
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
        self.gpu_context.refresh_temporal_bind_groups(device);
    }

    fn clear_temporal_inputs(&self, encoder: &mut wgpu::CommandEncoder) {
        for (label, view, clear_color) in [
            (
                "Previous GI Source Clear Pass",
                &self
                    .gpu_context
                    .descriptors
                    .texture_views
                    .lighting_target
                    .diffuse_radiance_ao,
                wgpu::Color::TRANSPARENT,
            ),
            (
                "Previous HBGI Reproject Clear Pass",
                self.gpu_context
                    .descriptors
                    .texture_views
                    .reproject
                    .bent_ao
                    .get_read_view(&self.gpu_context.resources.textures.reproject.bent_ao),
                wgpu::Color::TRANSPARENT,
            ),
            (
                "Previous HBGI Irradiance Reproject Clear Pass",
                self.gpu_context
                    .descriptors
                    .texture_views
                    .reproject
                    .near_field_irradiance
                    .get_read_view(
                        &self
                            .gpu_context
                            .resources
                            .textures
                            .reproject
                            .near_field_irradiance,
                    ),
                wgpu::Color::TRANSPARENT,
            ),
            (
                "Previous HBGI Reproject Depth Clear Pass",
                self.gpu_context
                    .descriptors
                    .texture_views
                    .reproject
                    .depth_history
                    .get_read_view(&self.gpu_context.resources.textures.reproject.depth_history),
                wgpu::Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            ),
            (
                "Previous HBGI Reproject Normal Clear Pass",
                self.gpu_context
                    .descriptors
                    .texture_views
                    .reproject
                    .normal_history
                    .get_read_view(&self.gpu_context.resources.textures.reproject.normal_history),
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
        self.gpu_context.rotate_temporal_resources(device);
    }

    fn render_hbgi_pyramid(&self, encoder: &mut wgpu::CommandEncoder) {
        let context = &self.gpu_context;
        let pyramids = &context.descriptors.texture_views.pyramids;
        let bind_groups = &context.descriptors.bind_groups.hbgi_pyramid;

        render_hbgi_pyramid_pass(
            encoder,
            "HBGI Pyramid Base Pass",
            &context.pipelines.hbgi_pyramid.base_pipeline,
            &bind_groups.base,
            &pyramids.diffuse_radiance_ao_mips[0],
            &pyramids.depth_mips[0],
            &pyramids.normal_mips[0],
            context,
        );

        for (dst_mip, bind_group) in bind_groups.downsample.iter().enumerate() {
            let mip_level = dst_mip + 1;
            render_hbgi_pyramid_pass(
                encoder,
                "HBGI Pyramid Downsample Pass",
                &context.pipelines.hbgi_pyramid.downsample_pipeline,
                bind_group,
                &pyramids.diffuse_radiance_ao_mips[mip_level],
                &pyramids.depth_mips[mip_level],
                &pyramids.normal_mips[mip_level],
                context,
            );
        }
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
        render_gbuffer_skinned_opaque_pass(
            encoder,
            skinned_opaque_pass,
            render_resources,
            &self.gpu_context,
        );
        render_gbuffer_static_opaque_pass(
            encoder,
            static_opaque_pass,
            render_resources,
            &self.gpu_context,
        );
        if !self.hbgi_reproject_valid {
            self.clear_temporal_inputs(encoder);
        }
        if let Some(hbgi_options) = &self.hbgi_options {
            self.gpu_context.resources.buffers.update_hbgi_settings(
                &HbgiSettingsUniform::from_options(hbgi_options, frame_idx),
                queue,
            );
            self.render_hbgi_pyramid(encoder);
            render_hbgi_pass(encoder, &self.gpu_context);
            self.gpu_context
                .resources
                .buffers
                .update_hbgi_reproject_settings(prev_inverse_view_proj, queue);
            render_hbgi_reproject_pass(encoder, &self.gpu_context);
            render_gi_blur_pass(encoder, &self.gpu_context);
            self.hbgi_reproject_valid = true;
        } else {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HBGI Disabled Clear Pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.gpu_context.descriptors.texture_views.hbgi.bent_ao,
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
                        view: &self
                            .gpu_context
                            .descriptors
                            .texture_views
                            .hbgi
                            .near_field_irradiance,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.gpu_context.descriptors.texture_views.gi_blur.bent_ao,
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
                        view: &self
                            .gpu_context
                            .descriptors
                            .texture_views
                            .gi_blur
                            .near_field_irradiance,
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
        render_deferred_lighting_pass(encoder, &self.gpu_context);
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
        let gpu_context = WorldGpuContext::new(
            wgpu_context,
            shader_cache,
            render_resources,
            &placeholders,
            brdf_lut,
            hbgi_options.as_ref(),
        );
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
            gpu_context,
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
        if let Some(hbgi_options) = &self.hbgi_options {
            self.gpu_context.resources.buffers.update_hbgi_settings(
                &HbgiSettingsUniform::from_options(hbgi_options, 0),
                &_wgpu_context.queue,
            );
        }
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
        let right = prepared_camera.state.rotation * Vec3::X;
        let up = prepared_camera.state.rotation * Vec3::Y;
        let forward = prepared_camera.state.rotation * -Vec3::Z;
        self.gpu_context.resources.buffers.camera.update(
            &prepared_camera.view_proj.to_cols_array(),
            &prepared_camera.state.position.to_array(),
            &prepared_camera.view_proj.inverse().to_cols_array(),
            &forward.to_array(),
            &[
                [right.x, right.y, right.z, 0.0],
                [up.x, up.y, up.z, 0.0],
                [forward.x, forward.y, forward.z, 0.0],
            ],
            &prev_motion_view_proj.to_cols_array(),
            &wgpu_context.queue,
        );
        self.gpu_context.update_lights(
            &snaps.curr.lights.sun,
            snaps.curr.lights.environment_map_intensity,
            &snaps.curr.lights.point_lights,
            snaps
                .curr
                .lights
                .environment_map
                .as_ref()
                .map(|env| (env.prefiltered, env.di)),
            render_resources,
            &self.placeholders,
            self.brdf_lut,
            wgpu_context,
        );
        let prepared_sun_shadow =
            prepare_sun_shadow(&prepared_camera, snaps.curr.lights.sun.direction);
        self.gpu_context
            .resources
            .buffers
            .lights
            .sun
            .update_shadow(&prepared_sun_shadow.uniform, &wgpu_context.queue);

        render_skybox_pass(encoder, &self.gpu_context);

        let skinned_draw = resolve_skinned_draw(
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
        let static_draw = resolve_static_draw(
            &mut self.static_instances,
            render_resources,
            &snaps,
            t,
            &wgpu_context.device,
            &wgpu_context.queue,
            &mut self.pose_storage,
            frame_idx,
        );
        self.gpu_context.upload_skinned_draw(
            &skinned_draw.instance_data,
            &skinned_draw.joint_palette,
            wgpu_context,
        );
        self.gpu_context
            .upload_static_draw(&static_draw.instance_data, wgpu_context);

        for ((cascade, cascade_view), cascade_bind_group) in prepared_sun_shadow
            .prepared
            .cascades
            .iter()
            .take(prepared_sun_shadow.prepared.cascade_count)
            .zip(
                self.gpu_context
                    .descriptors
                    .texture_views
                    .sun_shadow
                    .cascades
                    .iter(),
            )
            .zip(self.bind_groups.sun_shadow_matrices.iter())
        {
            cascade_bind_group.update(&cascade.light_view_proj, &wgpu_context.queue);
            render_sun_shadow_pass(
                encoder,
                &skinned_draw.opaque,
                &static_draw.opaque,
                cascade_view,
                &cascade_bind_group.bind_group,
                render_resources,
                &self.gpu_context,
            );
        }

        self.render_deferred_opaque(
            &skinned_draw.opaque,
            &static_draw.opaque,
            encoder,
            render_resources,
            &wgpu_context.device,
            &wgpu_context.queue,
            &prev_inverse_view_proj,
            frame_idx,
        );

        render_skinned_transparent_pass(
            encoder,
            &skinned_draw.transparent,
            render_resources,
            &self.gpu_context,
        );

        render_static_transparent_pass(
            encoder,
            &static_draw.transparent,
            render_resources,
            &self.gpu_context,
        );

        render_post_processing_pass(encoder, output_view, &self.gpu_context);
        self.prev_motion_view_proj = Some(prepared_camera.view_proj);
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.gpu_context.resize(wgpu_context);
        self.hbgi_reproject_valid = false;
        self.prev_motion_view_proj = None;
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

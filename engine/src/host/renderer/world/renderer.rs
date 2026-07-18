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
use super::world_context::{GiBlurBindGroup, HbgiPyramidBindGroups};

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
    pub gi_blur: wgpu::BindGroupLayout,
    pub hbgi_pyramid_base: wgpu::BindGroupLayout,
    pub hbgi_pyramid_downsample: wgpu::BindGroupLayout,
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
        let gi_blur = wgpu_context
            .device
            .create_bind_group_layout(&GiBlurBindGroup::desc());
        let hbgi_pyramid_base = wgpu_context
            .device
            .create_bind_group_layout(&HbgiPyramidBindGroups::base_desc());
        let hbgi_pyramid_downsample = wgpu_context
            .device
            .create_bind_group_layout(&HbgiPyramidBindGroups::downsample_desc());

        Self {
            camera,
            lights,
            sun_shadow_matrix,
            material,
            bones,
            motion_bones,
            instance_storage,
            pbr_material,
            gi_blur,
            hbgi_pyramid_base,
            hbgi_pyramid_downsample,
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

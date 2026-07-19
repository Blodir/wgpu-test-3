use std::{array, time::Instant};

use glam::Mat4;

use super::super::sampler_cache::SamplerCache;
use super::super::shader_cache::ShaderCache;
use super::anim_pose_store::AnimPoseStore;
use super::bg_layouts::BGLayouts;
use super::gpu_context::{SunShadowMatrixBindGroup, WorldGpuContext};
use super::instance_links::SnapshotInstanceLinks;
use super::material::MaterialBindGroup;
use super::prepare::camera::prepare_camera;
use super::prepare::mesh::{resolve_skinned_draw, PassDrawContext};
use super::prepare::sun_shadow::prepare_sun_shadow;
use super::{
    hbgi_settings::HbgiSettingsUniform,
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
use crate::host::renderer::{HbgiOptions, RendererOptions};
use crate::host::wgpu_context::WgpuContext;
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

struct WorldBindGroups {
    layouts: BGLayouts,
    sun_shadow_matrices: [SunShadowMatrixBindGroup; SUN_SHADOW_MAX_CASCADE_COUNT],
}
impl WorldBindGroups {
    fn new(wgpu_context: &WgpuContext) -> Self {
        let layouts = BGLayouts::new(wgpu_context);
        let sun_shadow_matrices = array::from_fn(|_| {
            SunShadowMatrixBindGroup::new(&wgpu_context.device, &layouts.sun_shadow_matrix)
        });
        Self {
            layouts,
            sun_shadow_matrices,
        }
    }
}

pub struct WorldRenderer {
    bind_groups: WorldBindGroups,
    gpu_context: WorldGpuContext,
    hbgi_options: Option<HbgiOptions>,
    hbgi_reproject_valid: bool,
    placeholders: PlaceholderTextureIds,
    brdf_lut: TextureRenderId,
    skinned_instance_links: SnapshotInstanceLinks,
    static_instance_links: SnapshotInstanceLinks,
    pose_storage: AnimPoseStore,
    prev_motion_view_proj: Option<Mat4>,
}
impl WorldRenderer {
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

    pub fn new(
        wgpu_context: &WgpuContext,
        placeholders: PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        _sampler_cache: &mut SamplerCache,
        shader_cache: &mut ShaderCache,
        render_resources: &RenderAssetStore,
        options: RendererOptions,
    ) -> Self {
        let skinned_instance_links = SnapshotInstanceLinks::default();
        let static_instance_links = SnapshotInstanceLinks::default();
        let hbgi_options = options.hbgi;
        let gpu_context = WorldGpuContext::new(
            wgpu_context,
            shader_cache,
            render_resources,
            &placeholders,
            brdf_lut,
            hbgi_options.as_ref(),
        );
        let bind_groups = WorldBindGroups::new(wgpu_context);
        let pose_storage = AnimPoseStore::new();

        Self {
            bind_groups,
            gpu_context,
            hbgi_options,
            hbgi_reproject_valid: false,
            skinned_instance_links,
            static_instance_links,
            placeholders,
            brdf_lut,
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
        _sampler_cache: &mut SamplerCache,
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
            &self.gpu_context.resources.buffers.camera,
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
            &mut self.skinned_instance_links,
            render_resources,
            &snaps,
            t,
            &mut self.pose_storage,
            frame_idx,
        );
        let static_draw = resolve_static_draw(
            &mut self.static_instance_links,
            render_resources,
            &snaps,
            t,
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
    ) -> Result<MaterialBindGroup, ()> {
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

        Ok(MaterialBindGroup::upload(
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

use std::time::Instant;

use super::super::sampler_cache::SamplerCache;
use super::super::shader_cache::ShaderCache;
use super::anim_pose_store::AnimPoseStore;
use super::external::MaterialBindGroup;
use super::gpu_context::{External, WorldGpuContext};
use super::instance_links::SnapshotInstanceLinks;
use super::prepare::camera::{prepare_camera, PreparedCamera};
use super::prepare::mesh::{resolve_skinned_draw, ResolvedSkinnedDraw, ResolvedStaticDraw};
use super::prepare::sun_shadow::{prepare_sun_shadow, PreparedSunShadowResult};
use super::{
    gpu_context::HbgiSettingsUniform,
    passes::{
        render_clear_temporal_inputs_passes, render_deferred_lighting_pass, render_gbuffer_skinned_opaque_pass,
        render_gbuffer_static_opaque_pass, render_hbgi_history_fix_pass, render_hbgi_pass,
        render_hbgi_pyramid_passes, render_hbgi_reproject_pass, render_hbgi_svgf_pass,
        render_gi_source_mips, render_skinned_transparent_pass, render_skybox_pass,
        render_ssr_composite_pass, render_ssr_pass, render_static_transparent_pass,
        render_sun_shadow_pass, render_taa_resolve_pass, render_tonemap_pass,
    },
};

use crate::host::assets::io::asset_formats::materialfile;
use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
use crate::host::renderer::HbgiSettings;
use crate::host::wgpu_context::WgpuContext;
use crate::host::world::prepare::mesh::resolve_static_draw;
use crate::{fixed_snapshot::FixedSnapshotGuard, var_snapshot::CameraSnapshotPair};

fn compute_t(now: Instant, prev_timestamp: Instant, curr_timestamp: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(curr_timestamp);
    let interval = curr_timestamp.saturating_duration_since(prev_timestamp);
    if interval.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f32() / interval.as_secs_f32()).clamp(0.0, 1.0)
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldRendererSettings {
    pub hbgi: HbgiSettings,
}
impl Default for WorldRendererSettings {
    fn default() -> Self {
        Self {
            hbgi: HbgiSettings::default(),
        }
    }
}

struct PreviousRenderData {
    camera: PreparedCamera,
    lights_environment_map: Option<(TextureRenderId, TextureRenderId)>,
}

struct PreparedRender<'a> {
    render_data: PreviousRenderData,
    prepared_sun_shadow: PreparedSunShadowResult,
    skinned_draw: ResolvedSkinnedDraw<'a>,
    static_draw: ResolvedStaticDraw<'a>,
}

pub struct WorldRenderer {
    gpu_context: WorldGpuContext,
    settings: WorldRendererSettings,
    hbgi_reproject_valid: bool,
    placeholders: PlaceholderTextureIds,
    skinned_instance_links: SnapshotInstanceLinks,
    static_instance_links: SnapshotInstanceLinks,
    pose_storage: AnimPoseStore,
    previous_render_data: Option<PreviousRenderData>,
}
impl WorldRenderer {
    fn rotate_temporal_buffers(&mut self, device: &wgpu::Device) {
        self.gpu_context.rotate_temporal_resources(device);
    }

    pub fn new(
        wgpu_context: &WgpuContext,
        placeholders: PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        blue_noise: TextureRenderId,
        shader_cache: &mut ShaderCache,
        render_resources: &RenderAssetStore,
        settings: WorldRendererSettings,
    ) -> Self {
        let skinned_instance_links = SnapshotInstanceLinks::default();
        let static_instance_links = SnapshotInstanceLinks::default();
        let gpu_context = WorldGpuContext::new(
            wgpu_context,
            shader_cache,
            render_resources,
            &placeholders,
            External {
                brdf_lut,
                blue_noise,
            },
            &settings.hbgi,
        );
        let pose_storage = AnimPoseStore::new();

        Self {
            gpu_context,
            settings,
            hbgi_reproject_valid: false,
            skinned_instance_links,
            static_instance_links,
            placeholders,
            pose_storage,
            previous_render_data: None,
        }
    }

    fn prepare_render<'a>(
        &mut self,
        snaps: &'a FixedSnapshotGuard,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        frame_idx: u32,
        camera_pair: &CameraSnapshotPair,
    ) -> PreparedRender<'a> {
        let now = Instant::now();
        let fixed_ss_t = compute_t(now, snaps.prev_timestamp, snaps.curr_timestamp);
        let var_ss_t = compute_t(now, camera_pair.prev_timestamp, camera_pair.curr_timestamp);
        let prepared_camera = prepare_camera(
            camera_pair,
            var_ss_t,
            frame_idx,
            &wgpu_context.surface_config,
        );
        let prev_camera = self
            .previous_render_data
            .as_ref()
            .map(|data| &data.camera)
            .unwrap_or(&prepared_camera);
        let prev_inverse_view_proj = prev_camera.inverse_view_proj;
        self.gpu_context
            .resources
            .buffers
            .camera
            .update(&prepared_camera, prev_camera, &wgpu_context.queue);
        let prepared_sun_shadow = prepare_sun_shadow(
            &prepared_camera,
            snaps.curr.lights.sun.direction,
            camera_pair.curr.sun_shadow_cascade_count,
            camera_pair.curr.sun_shadow_cascade_split_ratios,
        );
        self.gpu_context
            .resources
            .buffers
            .lights
            .sun
            .update_shadow(&prepared_sun_shadow.uniform, &wgpu_context.queue);

        let skinned_draw = resolve_skinned_draw(
            &mut self.skinned_instance_links,
            render_resources,
            &snaps,
            fixed_ss_t,
            &mut self.pose_storage,
            frame_idx,
        );
        let static_draw = resolve_static_draw(
            &mut self.static_instance_links,
            render_resources,
            &snaps,
            fixed_ss_t,
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

        for (cascade, cascade_bind_group) in prepared_sun_shadow
            .prepared
            .cascades
            .iter()
            .take(prepared_sun_shadow.prepared.cascade_count)
            .zip(
                self.gpu_context
                    .descriptors
                    .bind_groups
                    .sun_shadow_matrices
                    .iter(),
            )
        {
            cascade_bind_group.update(&cascade.light_view_proj, &wgpu_context.queue);
        }

        let environment_map = snaps
            .curr
            .lights
            .environment_map
            .as_ref()
            .map(|env| (env.prefiltered, env.di));
        self.gpu_context.update_lights(
            &snaps.curr.lights.sun,
            snaps.curr.lights.environment_map_intensity,
            &snaps.curr.lights.point_lights,
            environment_map,
            self.previous_render_data
                .as_ref()
                .map(|data| data.lights_environment_map)
                != Some(environment_map),
            render_resources,
            wgpu_context,
        );
        self.gpu_context.resources.buffers.update_hbgi_settings(
            &HbgiSettingsUniform::from_settings(&self.settings.hbgi, frame_idx),
            &wgpu_context.queue,
        );
        self.gpu_context
            .resources
            .buffers
            .update_hbgi_reproject_settings(&prev_inverse_view_proj, &wgpu_context.queue);

        PreparedRender {
            render_data: PreviousRenderData {
                camera: prepared_camera,
                lights_environment_map: environment_map,
            },
            prepared_sun_shadow,
            skinned_draw,
            static_draw,
        }
    }

    pub fn render(
        &mut self,
        snaps: &FixedSnapshotGuard,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        frame_idx: u32,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        camera_pair: &CameraSnapshotPair,
    ) {
        let PreparedRender {
            render_data,
            prepared_sun_shadow,
            skinned_draw,
            static_draw,
        } = self.prepare_render(
            snaps,
            wgpu_context,
            render_resources,
            frame_idx,
            camera_pair,
        );

        render_skybox_pass(encoder, &self.gpu_context);

        for (cascade_view, cascade_bind_group) in self
            .gpu_context
            .descriptors
            .texture_views
            .sun_shadow
            .cascades
            .iter()
            .take(prepared_sun_shadow.prepared.cascade_count)
            .zip(
                self.gpu_context
                    .descriptors
                    .bind_groups
                    .sun_shadow_matrices
                    .iter(),
            )
        {
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

        render_gbuffer_skinned_opaque_pass(
            encoder,
            &skinned_draw.opaque,
            render_resources,
            &self.gpu_context,
        );
        render_gbuffer_static_opaque_pass(
            encoder,
            &static_draw.opaque,
            render_resources,
            &self.gpu_context,
        );
        if !self.hbgi_reproject_valid {
            render_clear_temporal_inputs_passes(encoder, &self.gpu_context);
        }
        render_hbgi_pyramid_passes(encoder, &self.gpu_context);
        render_hbgi_pass(encoder, &self.gpu_context);
        render_hbgi_reproject_pass(encoder, &self.gpu_context);
        render_hbgi_history_fix_pass(encoder, &self.gpu_context);
        render_hbgi_svgf_pass(encoder, &self.gpu_context);
        self.hbgi_reproject_valid = true;
        render_deferred_lighting_pass(encoder, &self.gpu_context);
        render_gi_source_mips(encoder, &self.gpu_context);
        render_ssr_pass(encoder, &self.gpu_context);
        render_ssr_composite_pass(encoder, &self.gpu_context);

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

        render_taa_resolve_pass(encoder, &self.gpu_context);
        render_tonemap_pass(encoder, output_view, &self.gpu_context);
        self.rotate_temporal_buffers(&wgpu_context.device);
        self.previous_render_data = Some(render_data);
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext, render_resources: &RenderAssetStore) {
        self.gpu_context
            .resize(wgpu_context, render_resources, &self.placeholders);
        self.hbgi_reproject_valid = false;
        self.previous_render_data = None;
    }

    pub fn upload_settings(
        &mut self,
        settings: WorldRendererSettings,
        wgpu_context: &WgpuContext,
    ) {
        self.settings = settings;
        self.gpu_context.resources.buffers.update_hbgi_settings(
            &HbgiSettingsUniform::from_settings(&self.settings.hbgi, 0),
            &wgpu_context.queue,
        );
        self.hbgi_reproject_valid = false;
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
            &self.gpu_context.descriptors.bind_group_layouts.material,
            wgpu_context,
            sampler_cache,
        ))
    }

    pub fn store_anim_poses(
        &mut self,
        anim_pose_task_results: crate::workers::anim_pose::PoseJobResult,
    ) {
        self.pose_storage.receive_poses(anim_pose_task_results);
    }
}

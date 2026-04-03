use std::sync::Arc;
use std::time::Instant;

use super::super::sampler_cache::SamplerCache;
use super::super::shader_cache::ShaderCache;
use super::anim_pose_store::AnimPoseStore;
use super::attachments::depth::DepthTexture;
use super::attachments::msaa::MSAATextures;
use super::attachments::skybox::SkyboxOutputTexture;
use super::bindgroups::bones::BonesBinding;
use super::bindgroups::camera::CameraBinding;
use super::bindgroups::lights::LightsBinding;
use super::bindgroups::material::MaterialBinding;
use super::buffers::skinned_instance::SkinnedInstances;
use super::pipelines::post_processing::PostProcessingPipeline;
use super::pipelines::skinned_pbr::SkinnedPbrPipeline;
use super::pipelines::skybox::SkyboxPipeline;
use super::prepare::camera::prepare_camera;
use super::prepare::lights::prepare_lights;
use super::prepare::mesh::resolve_skinned_draw;

use crate::host::assets::io::asset_formats::materialfile;
use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
use crate::host::wgpu_context::WgpuContext;
use crate::host::world::buffers::static_instance::StaticInstances;
use crate::host::world::pipelines::static_pbr::StaticPbrPipeline;
use crate::host::world::prepare::mesh::resolve_static_draw;
use crate::{
    fixed_snapshot_handoff::FixedSnapshotHandoff, var_snapshot_handoff::CameraSnapshotPair,
};

pub struct Layouts {
    pub camera: wgpu::BindGroupLayout,
    pub lights: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub bones: wgpu::BindGroupLayout,
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
        let material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());
        let bones = wgpu_context
            .device
            .create_bind_group_layout(&BonesBinding::desc());
        let pbr_material = wgpu_context
            .device
            .create_bind_group_layout(&MaterialBinding::desc());

        Self {
            camera,
            lights,
            material,
            bones,
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

pub struct WorldRenderer {
    skybox_output: SkyboxOutputTexture,
    depth_texture: DepthTexture,
    msaa_textures: MSAATextures,
    skybox_pipeline: SkyboxPipeline,
    skinned_pipeline: SkinnedPbrPipeline,
    static_pipeline: StaticPbrPipeline,
    post_pipeline: PostProcessingPipeline,
    fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
    pub layouts: Layouts,
    pub placeholders: PlaceholderTextureIds,
    pub brdf_lut: TextureRenderId,
    bones: BonesBinding,
    pub camera: CameraBinding,
    lights: LightsBinding,
    skinned_instances: SkinnedInstances,
    static_instances: StaticInstances,
    pose_storage: AnimPoseStore,
}
impl WorldRenderer {
    pub fn new(
        wgpu_context: &WgpuContext,
        fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
        placeholders: PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        sampler_cache: &mut SamplerCache,
        shader_cache: &mut ShaderCache,
        render_resources: &RenderAssetStore,
    ) -> Self {
        let layouts = Layouts::new(&wgpu_context);
        let lights = LightsBinding::new(
            render_resources,
            brdf_lut,
            sampler_cache,
            &placeholders,
            wgpu_context,
            &layouts.lights,
        );

        let camera = CameraBinding::new(&wgpu_context.device, &layouts.camera);
        let bones = BonesBinding::new(&layouts.bones, &wgpu_context.device);
        let skinned_instances = SkinnedInstances::new(wgpu_context);
        let static_instances = StaticInstances::new(wgpu_context);
        let pose_storage = AnimPoseStore::new();

        let skybox_output =
            SkyboxOutputTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let msaa_textures = MSAATextures::new(&wgpu_context.device, &wgpu_context.surface_config);

        let skybox_pipeline = SkyboxPipeline::new(
            &wgpu_context,
            shader_cache,
            &layouts.camera,
            &layouts.lights,
        );
        let skinned_pipeline = SkinnedPbrPipeline::new(
            &wgpu_context,
            shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
            &layouts.bones,
        );
        let static_pipeline = StaticPbrPipeline::new(
            &wgpu_context,
            shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
        );
        let post_pipeline = PostProcessingPipeline::new(
            &wgpu_context,
            shader_cache,
            &skybox_output,
            &msaa_textures,
        );

        Self {
            skybox_output,
            depth_texture,
            msaa_textures,
            skybox_pipeline,
            skinned_pipeline,
            post_pipeline,
            fixed_snapshot_handoff,
            layouts,
            bones,
            camera,
            lights,
            skinned_instances,
            placeholders,
            brdf_lut,
            static_instances,
            static_pipeline,
            pose_storage,
        }
    }

    pub fn receive_poses(
        &mut self,
        anim_pose_task_results: crate::workers::worker_pool::AnimPoseTaskResult,
    ) {
        self.pose_storage.receive_poses(anim_pose_task_results);
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        sampler_cache: &mut SamplerCache,
        frame_idx: u32,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        camera_pair: Option<&CameraSnapshotPair>,
    ) {
        let snaps = self.fixed_snapshot_handoff.load();
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
        if let Some(camera_pair) = camera_pair {
            prepare_camera(
                &mut self.camera,
                camera_pair,
                now,
                &wgpu_context.queue,
                &wgpu_context.surface_config,
            );
        }
        prepare_lights(
            &snaps,
            &mut self.lights,
            self.brdf_lut,
            render_resources,
            sampler_cache,
            wgpu_context,
            &self.layouts.lights,
        );

        self.skybox_pipeline.render(
            encoder,
            &self.skybox_output.view,
            &self.camera.bind_group,
            &self.lights.bind_group,
        );

        let skinned_draw_context = resolve_skinned_draw(
            &mut self.bones,
            &self.layouts.bones,
            &mut self.skinned_instances,
            render_resources,
            &snaps,
            t,
            &wgpu_context.device,
            &wgpu_context.queue,
            &mut self.pose_storage,
            frame_idx,
        );
        let static_draw_context = resolve_static_draw(
            &mut self.static_instances,
            render_resources,
            &snaps,
            t,
            &wgpu_context.device,
            &wgpu_context.queue,
            &mut self.pose_storage,
            frame_idx,
        );

        self.skinned_pipeline.render(
            skinned_draw_context,
            &self.skinned_instances.buffer,
            encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.depth_texture.view,
            &self.camera.bind_group,
            &self.lights.bind_group,
            &self.bones.bind_group,
            render_resources,
        );

        self.static_pipeline.render(
            static_draw_context,
            &self.static_instances.buffer,
            encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            &self.camera.bind_group,
            &self.lights.bind_group,
            render_resources,
        );

        self.post_pipeline.render(encoder, output_view);
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.skybox_output =
            SkyboxOutputTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.msaa_textures = MSAATextures::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.post_pipeline.update_input_bindgroup(
            &wgpu_context.device,
            &self.skybox_output,
            &self.msaa_textures,
        );
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
            &self.layouts.material,
            wgpu_context,
            sampler_cache,
        ))
    }
}

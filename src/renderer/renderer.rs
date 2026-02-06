use std::sync::Arc;
use std::time::Instant;

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
use super::pose_storage::{self, PoseStorage};
use super::prepare::camera::prepare_camera;
use super::prepare::lights::prepare_lights;
use super::prepare::mesh::resolve_skinned_draw;
use super::sampler_cache::SamplerCache;
use super::shader_cache::ShaderCache;
use super::wgpu_context::WgpuContext;

use crate::render_snapshot::{CameraSnapshot, SnapshotHandoff};
use crate::renderer::buffers::static_instance::StaticInstances;
use crate::renderer::pipelines::static_pbr::StaticPbrPipeline;
use crate::renderer::prepare::mesh::resolve_static_draw;
use crate::resource_system::file_formats::materialfile;
use crate::resource_system::render_resources::{self, PlaceholderTextureIds, RenderResources, TextureRenderId};

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

pub struct Renderer {
    skybox_output: SkyboxOutputTexture,
    depth_texture: DepthTexture,
    msaa_textures: MSAATextures,
    skybox_pipeline: SkyboxPipeline,
    skinned_pipeline: SkinnedPbrPipeline,
    static_pipeline: StaticPbrPipeline,
    post_pipeline: PostProcessingPipeline,
    snapshot_handoff: Arc<SnapshotHandoff>,
    pub layouts: Layouts,
    pub placeholders: PlaceholderTextureIds,
    sampler_cache: SamplerCache,
    shader_cache: ShaderCache,
    bones: BonesBinding,
    pub camera: CameraBinding,
    lights: LightsBinding,
    skinned_instances: SkinnedInstances,
    static_instances: StaticInstances,
}
impl Renderer {
    pub fn new(
        wgpu_context: &WgpuContext,
        snapshot_handoff: Arc<SnapshotHandoff>,
        placeholders: PlaceholderTextureIds,
        render_resources: &RenderResources,
    ) -> Self {
        let layouts = Layouts::new(&wgpu_context);
        let mut sampler_cache = SamplerCache::new();
        let mut shader_cache = ShaderCache::new();
        let lights = LightsBinding::new(
            render_resources,
            &mut sampler_cache,
            &placeholders,
            wgpu_context,
            &layouts.lights,
        );

        let camera = CameraBinding::new(
            &wgpu_context.device,
            &layouts.camera,
        );
        let bones = BonesBinding::new(&layouts.bones, &wgpu_context.device);
        let skinned_instances = SkinnedInstances::new(wgpu_context);
        let static_instances = StaticInstances::new(wgpu_context);

        let skybox_output =
            SkyboxOutputTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let msaa_textures = MSAATextures::new(&wgpu_context.device, &wgpu_context.surface_config);

        let skybox_pipeline = SkyboxPipeline::new(
            &wgpu_context,
            &mut shader_cache,
            &layouts.camera,
            &layouts.lights,
        );
        let skinned_pipeline = SkinnedPbrPipeline::new(
            &wgpu_context,
            &mut shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
            &layouts.bones,
        );
        let static_pipeline = StaticPbrPipeline::new(
            &wgpu_context,
            &mut shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
        );
        let post_pipeline = PostProcessingPipeline::new(
            &wgpu_context,
            &mut shader_cache,
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
            snapshot_handoff,
            layouts,
            bones,
            camera,
            lights,
            skinned_instances,
            placeholders,
            sampler_cache,
            shader_cache,
            static_instances,
            static_pipeline,
        }
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        render_resources: &RenderResources,
        pose_storage: &mut PoseStorage,
        frame_idx: u32,
    ) -> Result<(), wgpu::SurfaceError> {
        let snaps = self.snapshot_handoff.load();
        let now = Instant::now();
        let t = (now - snaps.curr_timestamp)
            .div_duration_f32(snaps.curr_timestamp - snaps.prev_timestamp);
        //let t = ease_smoothstep(t_raw); // or ease_smootherstep / ease_in_out_sine
        prepare_camera(
            &mut self.camera,
            &snaps,
            t,
            &wgpu_context.queue,
            &wgpu_context.surface_config,
        );
        prepare_lights(&snaps, &mut self.lights, render_resources, &mut self.sampler_cache, wgpu_context, &self.layouts.lights);

        let mut encoder =
            wgpu_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        self.skybox_pipeline.render(
            &mut encoder,
            &self.skybox_output.view,
            &self.camera.bind_group,
            &self.lights.bind_group,
        );

        let skinned_draw_context = resolve_skinned_draw(&mut self.bones, &self.layouts.bones, &mut self.skinned_instances, &snaps, t, &wgpu_context.device, &wgpu_context.queue, pose_storage, frame_idx);
        let static_draw_context = resolve_static_draw(&mut self.static_instances, &snaps, t, &wgpu_context.device, &wgpu_context.queue);

        self.skinned_pipeline.render(
            skinned_draw_context,
            &self.skinned_instances.buffer,
            &mut encoder,
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
            &mut encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            &self.camera.bind_group,
            &self.lights.bind_group,
            render_resources,
        );

        let output_surface_texture = wgpu_context.surface.get_current_texture()?;
        let output_view = output_surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.post_pipeline.render(&mut encoder, &output_view)?;

        wgpu_context.queue.submit(Some(encoder.finish()));
        output_surface_texture.present();

        Ok(())
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

    pub fn upload_material(&mut self, manifest: &materialfile::Material, normal_texture: &Option<TextureRenderId>, occlusion_texture: &Option<TextureRenderId>, emissive_texture: &Option<TextureRenderId>, base_color_texture: &Option<TextureRenderId>, metallic_roughness_texture: &Option<TextureRenderId>, render_resources: &RenderResources, wgpu_context: &WgpuContext) -> Result<MaterialBinding, ()> {
        let textures_gpu = &render_resources.textures;
        let base_color_view = &textures_gpu.get(base_color_texture.unwrap_or(self.placeholders.base_color).into()).unwrap().texture_view;
        let emissive_view = &textures_gpu.get(emissive_texture.unwrap_or(self.placeholders.emissive).into()).unwrap().texture_view;
        let metallic_roughness_view = &textures_gpu.get(metallic_roughness_texture.unwrap_or(self.placeholders.metallic_roughness).into()).unwrap().texture_view;
        let normal_view = &textures_gpu.get(normal_texture.unwrap_or(self.placeholders.normals).into()).unwrap().texture_view;
        let occlusion_view = &textures_gpu.get(occlusion_texture.unwrap_or(self.placeholders.occlusion).into()).unwrap().texture_view;
        Ok(
            MaterialBinding::upload(manifest, base_color_view, emissive_view, metallic_roughness_view, normal_view, occlusion_view, &self.layouts.material, wgpu_context, &mut self.sampler_cache)
        )
    }
}

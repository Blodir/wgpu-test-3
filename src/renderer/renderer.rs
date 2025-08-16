use std::collections::HashMap;
use std::sync::Arc;

use super::render_snapshot::accumulate_model_transforms;
use super::render_snapshot::{self, SnapshotGuard};
use super::{render_resources::ModelHandle, render_snapshot::SnapshotHandoff};
use generational_arena::Index;
use glam::Mat4;

use crate::{
    renderer::{
        pipelines::{
            model::pipeline::ModelPipeline,
            post_processing::PostProcessingPipeline,
            resources::{
                depth_texture::DepthTexture, msaa_textures::MSAATextures,
                skybox_output::SkyboxOutputTexture,
            },
            skybox::SkyboxPipeline,
        },
        render_resources::RenderResources,
        wgpu_context::WgpuContext,
    },
    scene_tree::{RenderDataType, Scene},
};

pub struct Renderer {
    skybox_output: SkyboxOutputTexture,
    depth_texture: DepthTexture,
    msaa_textures: MSAATextures,
    skybox_pipeline: SkyboxPipeline,
    model_pipeline: ModelPipeline,
    post_pipeline: PostProcessingPipeline,
    snapshot_handoff: Arc<SnapshotHandoff>,
}
impl Renderer {
    pub fn new(
        wgpu_context: &WgpuContext,
        render_resourcess: &RenderResources,
        snapshot_handoff: Arc<SnapshotHandoff>,
    ) -> Self {
        let skybox_output =
            SkyboxOutputTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let msaa_textures = MSAATextures::new(&wgpu_context.device, &wgpu_context.surface_config);

        let skybox_pipeline = SkyboxPipeline::new(
            &wgpu_context.device,
            &render_resourcess.layouts.camera,
            &render_resourcess.layouts.environment_map,
        );
        let model_pipeline = ModelPipeline::new(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            &render_resourcess.layouts.camera,
            &render_resourcess.layouts.sun,
            &render_resourcess.layouts.environment_map,
        );
        let post_pipeline = PostProcessingPipeline::new(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            &skybox_output,
            &msaa_textures,
        );
        Self {
            skybox_output,
            depth_texture,
            msaa_textures,
            skybox_pipeline,
            model_pipeline,
            post_pipeline,
            snapshot_handoff,
        }
    }

    pub fn render(
        &self,
        render_resources: &mut RenderResources,
        wgpu_context: &WgpuContext,
    ) -> Result<(), wgpu::SurfaceError> {
        let t = 1f32;
        let snap = self.snapshot_handoff.load();
        let models = prepare_models(
            &snap,
            t,
            render_resources,
            &wgpu_context.device,
            &wgpu_context.queue,
        );
        prepare_camera(
            &snap.1,
            render_resources,
            &wgpu_context.queue,
            &wgpu_context.surface_config,
        );
        prepare_sun(&snap.1, render_resources, &wgpu_context.queue);
        prepare_env_map(&snap.1, render_resources, wgpu_context);

        let mut encoder =
            wgpu_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        self.skybox_pipeline.render(
            &mut encoder,
            &self.skybox_output.view,
            &render_resources.camera.bind_group,
            &render_resources
                .environment_maps
                .get(&snap.1.environment_map)
                .expect("Requested environment map is not loaded.")
                .bind_group,
        );

        self.model_pipeline.render(
            &mut encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            render_resources,
            models,
            &snap.1.environment_map,
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
}

pub fn prepare_camera(
    snap: &render_snapshot::RenderSnapshot,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
) {
    if let Some(camera) = &snap.camera {
        render_resources
            .camera
            .update(camera, queue, surface_config);
    }
}

pub fn prepare_sun(
    snap: &render_snapshot::RenderSnapshot,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
    queue: &wgpu::Queue,
) {
    if let Some(sun) = &snap.sun {
        render_resources.sun.update(sun, queue);
    }
}

pub fn prepare_env_map(
    snap: &render_snapshot::RenderSnapshot,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
    wgpu_context: &WgpuContext,
) {
    render_resources.load_environment_map(snap.environment_map.clone(), wgpu_context);
}

pub fn prepare_models<'a>(
    snaps: &'a (SnapshotGuard, SnapshotGuard),
    t: f32,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> impl Iterator<Item = &'a ModelHandle> + 'a {
    for (model_handle, node_transforms) in &snaps.1.model_transforms {
        let mut transforms = vec![];
        for (node_handle, curr_transform) in node_transforms {
            if let Some(prev_transform) = &snaps
                .0
                .model_transforms
                .get(model_handle)
                .and_then(|nodes| nodes.get(node_handle))
            {
                let (s1, r1, t1) = prev_transform.to_scale_rotation_translation();
                let (s2, r2, t2) = curr_transform.to_scale_rotation_translation();
                let s3 = s1.lerp(s2, t);
                let r3 = r1.slerp(r2, t);
                let t3 = t1.lerp(t2, t);
                transforms.push(Mat4::from_scale_rotation_translation(s3, r3, t3));
            } else {
                transforms.push(curr_transform.clone());
            }
        }
        render_resources
            .models
            .get_mut(&model_handle)
            .unwrap()
            .update_instance_buffer(device, queue, &transforms);
    }

    // return model handles that should be rendered
    snaps.1.model_transforms.iter().map(|(handle, _)| handle)
}

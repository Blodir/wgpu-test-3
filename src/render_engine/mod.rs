use std::collections::HashMap;

use glam::Mat4;
use render_resources::ModelHandle;

use crate::{
    render_engine::{
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
    scene_tree::{NodeHandle, RenderDataType, Scene},
};

pub mod pipelines;
pub mod render_resources;
pub mod utils;
pub mod wgpu_context;

pub struct RenderEngine {
    skybox_output: SkyboxOutputTexture,
    depth_texture: DepthTexture,
    msaa_textures: MSAATextures,
    skybox_pipeline: SkyboxPipeline,
    model_pipeline: ModelPipeline,
    post_pipeline: PostProcessingPipeline,
}
impl RenderEngine {
    pub fn new(wgpu_context: &WgpuContext, render_resourcess: &RenderResources) -> Self {
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
        }
    }

    pub fn render(
        &self,
        scene: &Scene,
        render_resources: &mut RenderResources,
        wgpu_context: &WgpuContext,
    ) -> Result<(), wgpu::SurfaceError> {
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
                .get(&scene.environment)
                .expect("Requested environment map is not loaded.")
                .bind_group,
        );

        let models = prepare_models(
            scene,
            render_resources,
            &wgpu_context.device,
            &wgpu_context.queue,
        );

        self.model_pipeline.render(
            &mut encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            render_resources,
            models,
            &scene.environment,
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

fn accumulate_model_transforms(
    scene: &Scene,
    models: &mut HashMap<ModelHandle, Vec<Mat4>>,
    base_transform: &Mat4,
    node_handle: &NodeHandle,
) {
    let node = scene.nodes.get(node_handle).unwrap();
    let RenderDataType::Model(model_handle) = &node.render_data;
    let v = models.entry(model_handle.clone()).or_insert_with(Vec::new);
    let transform = node.transform * base_transform;
    v.push(transform);
    for child in &node.children {
        accumulate_model_transforms(scene, models, &transform, &child);
    }
}

pub fn prepare_models(
    scene: &Scene,
    render_resources: &mut crate::render_engine::render_resources::RenderResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> impl Iterator<Item = ModelHandle> {
    let mut models = HashMap::<ModelHandle, Vec<Mat4>>::new();
    accumulate_model_transforms(scene, &mut models, &Mat4::IDENTITY, &scene.root);

    for (handle, transforms) in &models {
        render_resources
            .models
            .get_mut(&handle)
            .unwrap()
            .update_instance_buffer(device, queue, &transforms);
    }

    models.into_iter().map(|(handle, _)| handle.clone())
}

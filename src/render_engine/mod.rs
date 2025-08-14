use crate::{
    render_engine::wgpu_context::WgpuContext,
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
    },
    scene_tree::Scene,
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
        render_resources: &RenderResources,
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

        self.model_pipeline.render(
            &wgpu_context.queue,
            &mut encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            render_resources,
            scene,
        );

        let output_surface_texture = wgpu_context.surface.get_current_texture()?;
        let output_view = output_surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.post_pipeline
            .render(&wgpu_context.device, &wgpu_context.queue, &output_view)?;

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

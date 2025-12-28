use std::f32;
use std::sync::Arc;
use std::time::Instant;

use super::attachments::depth::DepthTexture;
use super::attachments::msaa::MSAATextures;
use super::attachments::skybox::SkyboxOutputTexture;
use super::bindgroups::bones::BonesBinding;
use super::bindgroups::camera::CameraBinding;
use super::bindgroups::lights::LightsBinding;
use super::bindgroups::material::MaterialBinding;
use super::buffers::instance::Instances;
use super::pipelines::post_processing::PostProcessingPipeline;
use super::pipelines::skinned::ModelPipeline;
use super::pipelines::skybox::SkyboxPipeline;
use super::prepare::camera::prepare_camera;
use super::prepare::lights::prepare_lights;
use super::prepare::mesh::resolve_skinned_draw;
use super::sampler_cache::SamplerCache;
use super::shader_cache::ShaderCache;
use super::wgpu_context::WgpuContext;
use generational_arena::Index;

use crate::render_snapshot::{CameraSnapshot, SnapshotHandoff};
use crate::resource_manager::gpu_resources::PlaceholderTextureIds;
use crate::resource_manager::registry::GpuState;
use crate::resource_manager::resource_manager::ResourceManager;

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
    model_pipeline: ModelPipeline,
    post_pipeline: PostProcessingPipeline,
    snapshot_handoff: Arc<SnapshotHandoff>,
    pub layouts: Layouts,
    pub placeholders: PlaceholderTextureIds,
    sampler_cache: SamplerCache,
    shader_cache: ShaderCache,
    bones: BonesBinding,
    pub camera: CameraBinding,
    lights: LightsBinding,
    instances: Instances,
}
impl Renderer {
    pub fn new(
        wgpu_context: &WgpuContext,
        snapshot_handoff: Arc<SnapshotHandoff>,
        placeholders: PlaceholderTextureIds,
        resource_manager: &Arc<ResourceManager>,
    ) -> Self {
        let layouts = Layouts::new(&wgpu_context);
        let mut sampler_cache = SamplerCache::new();
        let mut shader_cache = ShaderCache::new();
        let lights = LightsBinding::new(
            resource_manager,
            &mut sampler_cache,
            &placeholders,
            wgpu_context,
            &layouts.lights,
        );

        let camera = CameraBinding::new(
            &CameraSnapshot::default(),
            &wgpu_context.device,
            &wgpu_context.surface_config,
            &layouts.camera,
        );
        let bones = BonesBinding::new(&layouts.bones, &wgpu_context.device);
        let instances = Instances::new(wgpu_context);

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
        let model_pipeline = ModelPipeline::new(
            &wgpu_context,
            &mut shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
            &layouts.bones,
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
            model_pipeline,
            post_pipeline,
            snapshot_handoff,
            layouts,
            bones,
            camera,
            lights,
            instances,
            placeholders,
            sampler_cache,
            shader_cache,
        }
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        resource_manager: &Arc<ResourceManager>,
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
        prepare_lights(&snaps, &mut self.lights, resource_manager, &mut self.sampler_cache, wgpu_context, &self.layouts.lights);

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

        let draw_context = resolve_skinned_draw(&mut self.bones, &self.layouts.bones, &mut self.instances, &snaps, t, resource_manager, &wgpu_context.device, &wgpu_context.queue);

        self.model_pipeline.render(
            draw_context,
            &self.instances.buffer,
            &mut encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            &self.camera.bind_group,
            &self.lights.bind_group,
            &self.bones.bind_group,
            resource_manager,
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

    pub fn upload_material(&mut self, entry_cpu_idx: Index, resource_manager: &Arc<ResourceManager>, wgpu_context: &WgpuContext) -> Result<MaterialBinding, ()> {
        let materials_cpu = resource_manager.cpu.materials.lock().unwrap();
        let material_cpu = materials_cpu.get(entry_cpu_idx).unwrap();
        // check if textures uploaded to gpu, re-schedule if not
        let base_color_gpu_idx = {
            if let Some(handle) = material_cpu.base_color_texture.as_ref() {
                let reg = resource_manager.registry.lock().unwrap();
                match reg.get(handle).gpu_state {
                    GpuState::Ready(gpu_idx) => {
                        Some(gpu_idx)
                    },
                    _ => {
                        return Err(());
                    }
                }
            } else {
                None
            }
        };
        let emissive_gpu_idx = {
            if let Some(handle) = material_cpu.emissive_texture.as_ref() {
                let reg = resource_manager.registry.lock().unwrap();
                match reg.get(handle).gpu_state {
                    GpuState::Ready(gpu_idx) => {
                        Some(gpu_idx)
                    },
                    _ => {
                        return Err(());
                    }
                }
            } else {
                None
            }
        };
        let metallic_roughness_gpu_idx = {
            if let Some(handle) = material_cpu.metallic_roughness_texture.as_ref() {
                let reg = resource_manager.registry.lock().unwrap();
                match reg.get(handle).gpu_state {
                    GpuState::Ready(gpu_idx) => {
                        Some(gpu_idx)
                    },
                    _ => {
                        return Err(());
                    }
                }
            } else {
                None
            }
        };
        let normal_gpu_idx = {
            if let Some(handle) = material_cpu.normal_texture.as_ref() {
                let reg = resource_manager.registry.lock().unwrap();
                match reg.get(handle).gpu_state {
                    GpuState::Ready(gpu_idx) => {
                        Some(gpu_idx)
                    },
                    _ => {
                        return Err(());
                    }
                }
            } else {
                None
            }
        };
        let occlusion_gpu_idx = {
            if let Some(handle) = material_cpu.occlusion_texture.as_ref() {
                let reg = resource_manager.registry.lock().unwrap();
                match reg.get(handle).gpu_state {
                    GpuState::Ready(gpu_idx) => {
                        Some(gpu_idx)
                    },
                    _ => {
                        return Err(());
                    }
                }
            } else {
                None
            }
        };
        let textures_gpu = resource_manager.gpu.textures.lock().unwrap();
        let base_color_view = &textures_gpu.get(base_color_gpu_idx.unwrap_or(self.placeholders.base_color)).unwrap().texture_view;
        let emissive_view = &textures_gpu.get(emissive_gpu_idx.unwrap_or(self.placeholders.emissive)).unwrap().texture_view;
        let metallic_roughness_view = &textures_gpu.get(metallic_roughness_gpu_idx.unwrap_or(self.placeholders.metallic_roughness)).unwrap().texture_view;
        let normal_view = &textures_gpu.get(normal_gpu_idx.unwrap_or(self.placeholders.normals)).unwrap().texture_view;
        let occlusion_view = &textures_gpu.get(occlusion_gpu_idx.unwrap_or(self.placeholders.occlusion)).unwrap().texture_view;
        Ok(
            MaterialBinding::upload(&material_cpu.manifest, base_color_view, emissive_view, metallic_roughness_view, normal_view, occlusion_view, &self.layouts.material, wgpu_context, &mut self.sampler_cache)
        )
    }
}

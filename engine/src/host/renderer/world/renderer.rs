use std::{array, time::Instant};

use super::super::sampler_cache::SamplerCache;
use super::super::shader_cache::ShaderCache;
use super::anim_pose_store::AnimPoseStore;
use super::attachments::color::HdrColorTexture;
use super::attachments::deferred::{GBufferTargets, GtaoTexture};
use super::attachments::depth::DepthTexture;
use super::attachments::skybox::SkyboxOutputTexture;
use super::attachments::sun_shadow::SunShadowTexture;
use super::bindgroups::bones::BonesBinding;
use super::bindgroups::camera::CameraBinding;
use super::bindgroups::lights::LightsBinding;
use super::bindgroups::material::MaterialBinding;
use super::bindgroups::sun_shadow_matrix::SunShadowMatrixBindGroup;
use super::buffers::skinned_instance::SkinnedInstances;
use super::pipelines::deferred_lighting::DeferredLightingPipeline;
use super::pipelines::g_buffer::GBufferPipeline;
use super::pipelines::gtao::GtaoPipeline;
use super::pipelines::post_processing::PostProcessingPipeline;
use super::pipelines::skinned_pbr::SkinnedPbrPipeline;
use super::pipelines::skybox::SkyboxPipeline;
use super::pipelines::sun_shadow::SunShadowPipeline;
use super::prepare::camera::prepare_camera;
use super::prepare::lights::prepare_lights;
use super::prepare::mesh::{resolve_skinned_draw, PassDrawContext};
use super::prepare::sun_shadow::prepare_sun_shadow;

use crate::host::assets::io::asset_formats::materialfile;
use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
use crate::host::renderer::{OpaqueRenderPath, RendererOptions};
use crate::host::wgpu_context::WgpuContext;
use crate::host::world::buffers::static_instance::StaticInstances;
use crate::host::world::pipelines::static_pbr::StaticPbrPipeline;
use crate::host::world::prepare::mesh::resolve_static_draw;
use crate::host::world::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT;
use crate::{fixed_snapshot::FixedSnapshotGuard, var_snapshot::CameraSnapshotPair};

pub struct Layouts {
    pub camera: wgpu::BindGroupLayout,
    pub lights: wgpu::BindGroupLayout,
    pub sun_shadow_matrix: wgpu::BindGroupLayout,
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
        let sun_shadow_matrix = wgpu_context
            .device
            .create_bind_group_layout(&SunShadowMatrixBindGroup::desc());
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
            sun_shadow_matrix,
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

struct WorldAttachments {
    skybox_output: SkyboxOutputTexture,
    depth_texture: DepthTexture,
    hdr_color: HdrColorTexture,
    gi_source: HdrColorTexture,
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
            gi_source: HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config),
            sun_shadow: SunShadowTexture::new(&wgpu_context.device),
        }
    }

    fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.skybox_output =
            SkyboxOutputTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.hdr_color = HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.gi_source = HdrColorTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
    }
}

struct WorldBindGroups {
    layouts: Layouts,
    bones: BonesBinding,
    camera: CameraBinding,
    lights: LightsBinding,
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
        let bones = BonesBinding::new(&layouts.bones, &wgpu_context.device);
        Self {
            layouts,
            bones,
            camera,
            lights,
            sun_shadow_matrices,
        }
    }
}

struct WorldPipelines {
    skybox: SkyboxPipeline,
    sun_shadow: SunShadowPipeline,
    skinned_pbr: SkinnedPbrPipeline,
    static_pbr: StaticPbrPipeline,
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
        );
        let skinned_pbr = SkinnedPbrPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
            &layouts.bones,
        );
        let static_pbr = StaticPbrPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.pbr_material,
            &layouts.camera,
            &layouts.lights,
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
            skinned_pbr,
            static_pbr,
            post,
        }
    }
}

struct DeferredOpaqueRenderer {
    gtao_enabled: bool,
    g_buffer_targets: GBufferTargets,
    gtao_texture: GtaoTexture,
    gtao_pipeline: GtaoPipeline,
    g_buffer_pipeline: GBufferPipeline,
    deferred_lighting_pipeline: DeferredLightingPipeline,
}
impl DeferredOpaqueRenderer {
    fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
        gtao_enabled: bool,
    ) -> Self {
        let g_buffer_targets =
            GBufferTargets::new(&wgpu_context.device, &wgpu_context.surface_config);
        let gtao_texture = GtaoTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let gtao_pipeline = GtaoPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.camera,
            &g_buffer_targets,
        );
        let g_buffer_pipeline = GBufferPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.material,
            &layouts.camera,
            &layouts.lights,
            &layouts.bones,
        );
        let deferred_lighting_pipeline = DeferredLightingPipeline::new(
            wgpu_context,
            shader_cache,
            &layouts.camera,
            &layouts.lights,
            &g_buffer_targets,
            &gtao_texture,
        );
        Self {
            gtao_enabled,
            g_buffer_targets,
            gtao_texture,
            gtao_pipeline,
            g_buffer_pipeline,
            deferred_lighting_pipeline,
        }
    }

    fn render<'a>(
        &mut self,
        skinned_opaque_pass: &'a PassDrawContext<'a>,
        static_opaque_pass: &'a PassDrawContext<'a>,
        skinned_instance_buffer: &wgpu::Buffer,
        static_instance_buffer: &wgpu::Buffer,
        encoder: &mut wgpu::CommandEncoder,
        depth_texture_view: &wgpu::TextureView,
        hdr_color_view: &wgpu::TextureView,
        gi_source_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
        bones_bind_group: &wgpu::BindGroup,
        render_resources: &'a RenderAssetStore,
    ) {
        self.g_buffer_pipeline.render_skinned_opaque(
            skinned_opaque_pass,
            skinned_instance_buffer,
            encoder,
            &self.g_buffer_targets,
            depth_texture_view,
            camera_bind_group,
            lights_bind_group,
            bones_bind_group,
            render_resources,
        );
        self.g_buffer_pipeline.render_static_opaque(
            static_opaque_pass,
            static_instance_buffer,
            encoder,
            &self.g_buffer_targets,
            depth_texture_view,
            camera_bind_group,
            lights_bind_group,
            render_resources,
        );
        if self.gtao_enabled {
            self.gtao_pipeline
                .render(encoder, &self.gtao_texture.view, camera_bind_group);
        } else {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GTAO Disabled Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.gtao_texture.view,
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
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
        self.deferred_lighting_pipeline.render(
            encoder,
            hdr_color_view,
            gi_source_view,
            camera_bind_group,
            lights_bind_group,
        );
    }

    fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.g_buffer_targets =
            GBufferTargets::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.gtao_texture = GtaoTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        self.gtao_pipeline
            .update_input_bindgroup(&wgpu_context.device, &self.g_buffer_targets);
        self.deferred_lighting_pipeline.update_input_bindgroup(
            &wgpu_context.device,
            &self.g_buffer_targets,
            &self.gtao_texture,
        );
    }
}

struct CompactDeferredOpaqueRenderer;
impl CompactDeferredOpaqueRenderer {
    fn new(
        _wgpu_context: &WgpuContext,
        _shader_cache: &mut ShaderCache,
        _layouts: &Layouts,
    ) -> Self {
        Self
    }

    fn render<'a>(
        &mut self,
        _skinned_opaque_pass: &'a PassDrawContext<'a>,
        _static_opaque_pass: &'a PassDrawContext<'a>,
        _skinned_instance_buffer: &wgpu::Buffer,
        _static_instance_buffer: &wgpu::Buffer,
        _encoder: &mut wgpu::CommandEncoder,
        _depth_texture_view: &wgpu::TextureView,
        _hdr_color_view: &wgpu::TextureView,
        _gi_source_view: &wgpu::TextureView,
        _camera_bind_group: &wgpu::BindGroup,
        _lights_bind_group: &wgpu::BindGroup,
        _bones_bind_group: &wgpu::BindGroup,
        _render_resources: &'a RenderAssetStore,
    ) {
    }

    fn resize(&mut self, _wgpu_context: &WgpuContext) {}
}

enum OpaqueRenderer {
    Forward,
    Deferred(DeferredOpaqueRenderer),
    CompactDeferred(CompactDeferredOpaqueRenderer),
}

pub struct WorldRenderer {
    attachments: WorldAttachments,
    bind_groups: WorldBindGroups,
    pipelines: WorldPipelines,
    opaque_renderer: OpaqueRenderer,
    placeholders: PlaceholderTextureIds,
    brdf_lut: TextureRenderId,
    skinned_instances: SkinnedInstances,
    static_instances: StaticInstances,
    pose_storage: AnimPoseStore,
}
impl WorldRenderer {
    fn build_opaque_renderer(
        options: RendererOptions,
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        layouts: &Layouts,
    ) -> OpaqueRenderer {
        match options.opaque_render_path {
            OpaqueRenderPath::Forward => OpaqueRenderer::Forward,
            OpaqueRenderPath::Deferred { gtao } => OpaqueRenderer::Deferred(
                DeferredOpaqueRenderer::new(wgpu_context, shader_cache, layouts, gtao),
            ),
            OpaqueRenderPath::CompactDeferred => OpaqueRenderer::CompactDeferred(
                CompactDeferredOpaqueRenderer::new(wgpu_context, shader_cache, layouts),
            ),
        }
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
        let bind_groups = WorldBindGroups::new(
            wgpu_context,
            &placeholders,
            brdf_lut,
            sampler_cache,
            render_resources,
            &attachments.sun_shadow.array_view,
        );
        let skinned_instances = SkinnedInstances::new(wgpu_context);
        let static_instances = StaticInstances::new(wgpu_context);
        let pose_storage = AnimPoseStore::new();
        let pipelines = WorldPipelines::new(
            wgpu_context,
            shader_cache,
            &bind_groups.layouts,
            &attachments,
        );
        let opaque_renderer =
            Self::build_opaque_renderer(options, wgpu_context, shader_cache, &bind_groups.layouts);

        Self {
            attachments,
            bind_groups,
            pipelines,
            opaque_renderer,
            skinned_instances,
            placeholders,
            brdf_lut,
            static_instances,
            pose_storage,
        }
    }

    pub fn apply_options(
        &mut self,
        options: RendererOptions,
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
    ) {
        self.opaque_renderer = Self::build_opaque_renderer(
            options,
            wgpu_context,
            shader_cache,
            &self.bind_groups.layouts,
        );
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
            &wgpu_context.queue,
            &wgpu_context.surface_config,
        );
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
                &self.skinned_instances.buffer,
                &self.static_instances.buffer,
                encoder,
                cascade_view,
                &cascade_bind_group.bind_group,
                &self.bind_groups.bones.bind_group,
                render_resources,
            );
        }

        match &mut self.opaque_renderer {
            OpaqueRenderer::Forward => {
                self.pipelines.skinned_pbr.render_opaque(
                    &skinned_opaque_pass,
                    &self.skinned_instances.buffer,
                    encoder,
                    &self.attachments.hdr_color.view,
                    &self.attachments.depth_texture.view,
                    &self.bind_groups.camera.bind_group,
                    &self.bind_groups.lights.bind_group,
                    &self.bind_groups.bones.bind_group,
                    render_resources,
                );

                self.pipelines.static_pbr.render_opaque(
                    &static_opaque_pass,
                    &self.static_instances.buffer,
                    encoder,
                    &self.attachments.hdr_color.view,
                    &self.attachments.depth_texture.view,
                    &self.bind_groups.camera.bind_group,
                    &self.bind_groups.lights.bind_group,
                    render_resources,
                );
            }
            OpaqueRenderer::Deferred(renderer) => renderer.render(
                &skinned_opaque_pass,
                &static_opaque_pass,
                &self.skinned_instances.buffer,
                &self.static_instances.buffer,
                encoder,
                &self.attachments.depth_texture.view,
                &self.attachments.hdr_color.view,
                &self.attachments.gi_source.view,
                &self.bind_groups.camera.bind_group,
                &self.bind_groups.lights.bind_group,
                &self.bind_groups.bones.bind_group,
                render_resources,
            ),
            OpaqueRenderer::CompactDeferred(renderer) => renderer.render(
                &skinned_opaque_pass,
                &static_opaque_pass,
                &self.skinned_instances.buffer,
                &self.static_instances.buffer,
                encoder,
                &self.attachments.depth_texture.view,
                &self.attachments.hdr_color.view,
                &self.attachments.gi_source.view,
                &self.bind_groups.camera.bind_group,
                &self.bind_groups.lights.bind_group,
                &self.bind_groups.bones.bind_group,
                render_resources,
            ),
        }

        self.pipelines.skinned_pbr.render_transparent(
            &skinned_transparent_pass,
            &self.skinned_instances.buffer,
            encoder,
            &self.attachments.hdr_color.view,
            &self.attachments.depth_texture.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
            &self.bind_groups.bones.bind_group,
            render_resources,
        );

        self.pipelines.static_pbr.render_transparent(
            &static_transparent_pass,
            &self.static_instances.buffer,
            encoder,
            &self.attachments.hdr_color.view,
            &self.attachments.depth_texture.view,
            &self.bind_groups.camera.bind_group,
            &self.bind_groups.lights.bind_group,
            render_resources,
        );

        self.pipelines.post.render(encoder, output_view);
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.attachments.resize(wgpu_context);
        self.pipelines.post.update_input_bindgroup(
            &wgpu_context.device,
            &self.attachments.skybox_output,
            &self.attachments.hdr_color,
        );
        match &mut self.opaque_renderer {
            OpaqueRenderer::Forward => {}
            OpaqueRenderer::Deferred(renderer) => renderer.resize(wgpu_context),
            OpaqueRenderer::CompactDeferred(renderer) => renderer.resize(wgpu_context),
        }
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

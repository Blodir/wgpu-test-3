use std::array;

use wgpu::util::DeviceExt as _;

use super::{
    descriptors::{
        bg_layouts::BGLayouts, BindGroups, BonesBindGroups, CameraBindGroup,
        DeferredLightingBindGroups, Descriptors, GiBlurBindGroup, HbgiBindGroups,
        HbgiPyramidBindGroups, HbgiReprojectBindGroups, InstanceStorageBindGroup, LightsBindGroup,
        PostProcessingBindGroup, SunShadowMatrixBindGroup, TextureViews,
    },
    pipelines::Pipelines,
    resources::{Buffers, Resources, Samplers, Textures},
};
use crate::{
    fixed_snapshot::PointLightSnapshot,
    game::scene_tree::Sun,
    host::{
        assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId},
        renderer::{
            world::{
                gpu_context::HbgiSettingsUniform,
                prepare::mesh::{BoneMat34, SkinnedInstance, StaticInstance},
            },
            HbgiOptions,
        },
        shader_cache::ShaderCache,
        wgpu_context::WgpuContext,
    },
};

pub(crate) struct WorldGpuContext {
    pub(crate) resources: Resources,
    pub(crate) descriptors: Descriptors,
    pub(crate) pipelines: Pipelines,
}
impl WorldGpuContext {
    pub(crate) fn new(
        wgpu_context: &WgpuContext,
        shader_cache: &mut ShaderCache,
        render_resources: &RenderAssetStore,
        placeholders: &PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        hbgi_options: Option<&HbgiOptions>,
    ) -> Self {
        let hbgi_settings =
            hbgi_options.map(|options| HbgiSettingsUniform::from_options(options, 0));
        let resources = Resources {
            textures: Textures::new(wgpu_context, &wgpu_context.surface_config),
            buffers: Buffers::new(wgpu_context, hbgi_settings.as_ref()),
            samplers: Samplers::new(&wgpu_context.device),
        };
        let texture_views = TextureViews::new(&resources.textures);
        let bind_group_layouts = BGLayouts::new(wgpu_context);
        let device = &wgpu_context.device;
        let textures = &render_resources.textures;
        let prefiltered = textures.get(placeholders.prefiltered.into()).unwrap();
        let di = textures.get(placeholders.di.into()).unwrap();
        let brdf = textures.get(brdf_lut.into()).unwrap();

        let bind_groups = BindGroups {
            bones: BonesBindGroups::new(
                &resources.buffers.bones,
                &resources.buffers.skinned_instances,
                &bind_group_layouts.bones,
                &bind_group_layouts.motion_bones,
                device,
            ),
            camera: CameraBindGroup::new(
                &resources.buffers.camera,
                &bind_group_layouts.camera,
                device,
            ),
            hbgi: HbgiBindGroups::new(
                &resources.buffers.hbgi_settings,
                &bind_group_layouts.hbgi_settings,
                &texture_views.pyramids,
                &resources.samplers.hbgi_pyramid_downsample,
                &resources.samplers.hbgi_pyramid_downsample,
                &bind_group_layouts.hbgi_inputs,
                device,
            ),
            deferred_lighting: DeferredLightingBindGroups::new(
                &texture_views.gbuffer.albedo_ao,
                &resources.samplers.nearest,
                &texture_views.gbuffer.normal_roughness,
                &resources.samplers.nearest,
                &texture_views.gbuffer.emissive_metallic,
                &resources.samplers.nearest,
                &texture_views.gbuffer.depth,
                &texture_views.gi_blur.bent_ao,
                &resources.samplers.linear,
                &texture_views.gi_blur.near_field_irradiance,
                &resources.samplers.linear,
                &bind_group_layouts,
                device,
            ),
            hbgi_reproject: HbgiReprojectBindGroups::new(
                &texture_views.gbuffer.motion_vectors,
                &texture_views.hbgi.bent_ao,
                texture_views
                    .reproject
                    .bent_ao
                    .get_read_view(&resources.textures.reproject.bent_ao),
                &texture_views.gbuffer.depth,
                &texture_views.gbuffer.normal_roughness,
                texture_views
                    .reproject
                    .depth_history
                    .get_read_view(&resources.textures.reproject.depth_history),
                texture_views
                    .reproject
                    .normal_history
                    .get_read_view(&resources.textures.reproject.normal_history),
                &texture_views.hbgi.near_field_irradiance,
                texture_views
                    .reproject
                    .near_field_irradiance
                    .get_read_view(&resources.textures.reproject.near_field_irradiance),
                &resources.buffers.hbgi_reproject_settings,
                &bind_group_layouts,
                device,
            ),
            gi_blur: GiBlurBindGroup::new(
                &texture_views.hbgi.bent_ao,
                &resources.samplers.linear,
                &texture_views.hbgi.near_field_irradiance,
                &resources.samplers.linear,
                &texture_views.gbuffer.normal_roughness,
                &resources.samplers.nearest,
                &texture_views.gbuffer.depth,
                &bind_group_layouts.gi_blur,
                device,
            ),
            hbgi_pyramid: HbgiPyramidBindGroups::new(
                device,
                &bind_group_layouts,
                &resources.samplers.hbgi_pyramid_downsample,
                &texture_views.lighting_target.diffuse_radiance_ao,
                &resources.samplers.nearest,
                &texture_views.gbuffer.depth,
                &texture_views.gbuffer.normal_roughness,
                &resources.samplers.nearest,
                &texture_views.pyramids,
            ),
            lights: LightsBindGroup::new(
                &resources.buffers.lights,
                &prefiltered.texture_view,
                &resources.samplers.linear,
                &di.texture_view,
                &resources.samplers.linear,
                &brdf.texture_view,
                &resources.samplers.linear,
                &texture_views.sun_shadow.array,
                &resources.samplers.comparison,
                &bind_group_layouts.lights,
                device,
            ),
            post_processing: PostProcessingBindGroup::new(
                &texture_views.sky,
                &resources.samplers.linear,
                &texture_views.lighting_target.lit_hdr,
                &resources.samplers.nearest,
                &bind_group_layouts.post_processing,
                device,
            ),
            static_instances: InstanceStorageBindGroup::new(
                &resources.buffers.static_instances,
                &bind_group_layouts.instance_storage,
                device,
            ),
            sun_shadow_matrices: array::from_fn(|_| {
                SunShadowMatrixBindGroup::new(device, &bind_group_layouts.sun_shadow_matrix)
            }),
        };
        let descriptors = Descriptors {
            bind_groups,
            bind_group_layouts,
            texture_views,
        };
        let pipelines = Pipelines::new(wgpu_context, shader_cache, &descriptors.bind_group_layouts);

        Self {
            resources,
            descriptors,
            pipelines,
        }
    }

    fn rebuild_surface_bind_groups(&mut self, device: &wgpu::Device) {
        self.descriptors.bind_groups.hbgi = HbgiBindGroups::new(
            &self.resources.buffers.hbgi_settings,
            &self.descriptors.bind_group_layouts.hbgi_settings,
            &self.descriptors.texture_views.pyramids,
            &self.resources.samplers.hbgi_pyramid_downsample,
            &self.resources.samplers.hbgi_pyramid_downsample,
            &self.descriptors.bind_group_layouts.hbgi_inputs,
            device,
        );
        self.descriptors.bind_groups.deferred_lighting = DeferredLightingBindGroups::new(
            &self.descriptors.texture_views.gbuffer.albedo_ao,
            &self.resources.samplers.nearest,
            &self.descriptors.texture_views.gbuffer.normal_roughness,
            &self.resources.samplers.nearest,
            &self.descriptors.texture_views.gbuffer.emissive_metallic,
            &self.resources.samplers.nearest,
            &self.descriptors.texture_views.gbuffer.depth,
            &self.descriptors.texture_views.gi_blur.bent_ao,
            &self.resources.samplers.linear,
            &self.descriptors.texture_views.gi_blur.near_field_irradiance,
            &self.resources.samplers.linear,
            &self.descriptors.bind_group_layouts,
            device,
        );
        self.descriptors.bind_groups.hbgi_pyramid = HbgiPyramidBindGroups::new(
            device,
            &self.descriptors.bind_group_layouts,
            &self.resources.samplers.hbgi_pyramid_downsample,
            &self
                .descriptors
                .texture_views
                .lighting_target
                .diffuse_radiance_ao,
            &self.resources.samplers.nearest,
            &self.descriptors.texture_views.gbuffer.depth,
            &self.descriptors.texture_views.gbuffer.normal_roughness,
            &self.resources.samplers.nearest,
            &self.descriptors.texture_views.pyramids,
        );
        self.descriptors.bind_groups.post_processing = PostProcessingBindGroup::new(
            &self.descriptors.texture_views.sky,
            &self.resources.samplers.linear,
            &self.descriptors.texture_views.lighting_target.lit_hdr,
            &self.resources.samplers.nearest,
            &self.descriptors.bind_group_layouts.post_processing,
            device,
        );
        self.refresh_temporal_bind_groups(device);
    }

    pub(crate) fn upload_skinned_draw(
        &mut self,
        instance_data: &[SkinnedInstance],
        joint_palette: &[BoneMat34],
        wgpu_context: &WgpuContext,
    ) {
        let default_instance = [SkinnedInstance::default()];
        let default_bone = [BoneMat34::default()];
        let instance_bytes = if instance_data.is_empty() {
            bytemuck::cast_slice(&default_instance)
        } else {
            bytemuck::cast_slice(instance_data)
        };
        let bone_bytes = if joint_palette.is_empty() {
            bytemuck::cast_slice(&default_bone)
        } else {
            bytemuck::cast_slice(joint_palette)
        };

        self.resources.buffers.skinned_instances.swap();
        self.resources
            .buffers
            .skinned_instances
            .write(instance_bytes, wgpu_context);
        self.resources.buffers.bones.swap();
        self.resources.buffers.bones.write(bone_bytes, wgpu_context);
        self.descriptors.bind_groups.bones = BonesBindGroups::new(
            &self.resources.buffers.bones,
            &self.resources.buffers.skinned_instances,
            &self.descriptors.bind_group_layouts.bones,
            &self.descriptors.bind_group_layouts.motion_bones,
            &wgpu_context.device,
        );
    }

    pub(crate) fn upload_static_draw(
        &mut self,
        instance_data: &[StaticInstance],
        wgpu_context: &WgpuContext,
    ) {
        let default_instance = [StaticInstance::default()];
        let instance_bytes = if instance_data.is_empty() {
            bytemuck::cast_slice(&default_instance)
        } else {
            bytemuck::cast_slice(instance_data)
        };
        if self.resources.buffers.static_instances.size() >= instance_bytes.len() as u64 {
            wgpu_context.queue.write_buffer(
                &self.resources.buffers.static_instances,
                0,
                instance_bytes,
            );
        } else {
            self.resources.buffers.static_instances =
                wgpu_context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Static instance buffer"),
                        contents: instance_bytes,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    });
        }
        self.descriptors.bind_groups.static_instances = InstanceStorageBindGroup::new(
            &self.resources.buffers.static_instances,
            &self.descriptors.bind_group_layouts.instance_storage,
            &wgpu_context.device,
        );
    }

    pub(crate) fn update_lights(
        &mut self,
        sun: &Sun,
        environment_map_intensity: f32,
        point_lights: &[PointLightSnapshot],
        environment_map: Option<(TextureRenderId, TextureRenderId)>,
        render_resources: &RenderAssetStore,
        placeholders: &PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        wgpu_context: &WgpuContext,
    ) {
        self.resources
            .buffers
            .lights
            .update_sun(sun, &wgpu_context.queue);
        self.resources
            .buffers
            .lights
            .update_environment_map_intensity(environment_map_intensity, &wgpu_context.queue);
        self.resources
            .buffers
            .lights
            .update_point_lights(point_lights, &wgpu_context.queue);

        let (prefiltered_id, di_id) =
            environment_map.unwrap_or((placeholders.prefiltered, placeholders.di));
        let textures = &render_resources.textures;
        let prefiltered = textures.get(prefiltered_id.into()).unwrap();
        let di = textures.get(di_id.into()).unwrap();
        let brdf = textures.get(brdf_lut.into()).unwrap();
        self.descriptors.bind_groups.lights = LightsBindGroup::new(
            &self.resources.buffers.lights,
            &prefiltered.texture_view,
            &self.resources.samplers.linear,
            &di.texture_view,
            &self.resources.samplers.linear,
            &brdf.texture_view,
            &self.resources.samplers.linear,
            &self.descriptors.texture_views.sun_shadow.array,
            &self.resources.samplers.comparison,
            &self.descriptors.bind_group_layouts.lights,
            &wgpu_context.device,
        );
    }

    pub(crate) fn refresh_temporal_bind_groups(&mut self, device: &wgpu::Device) {
        self.descriptors.bind_groups.gi_blur = GiBlurBindGroup::new(
            self.descriptors
                .texture_views
                .reproject
                .bent_ao
                .get_write_view(&self.resources.textures.reproject.bent_ao),
            &self.resources.samplers.linear,
            self.descriptors
                .texture_views
                .reproject
                .near_field_irradiance
                .get_write_view(&self.resources.textures.reproject.near_field_irradiance),
            &self.resources.samplers.linear,
            &self.descriptors.texture_views.gbuffer.normal_roughness,
            &self.resources.samplers.nearest,
            &self.descriptors.texture_views.gbuffer.depth,
            &self.descriptors.bind_group_layouts.gi_blur,
            device,
        );
        self.descriptors.bind_groups.hbgi_reproject = HbgiReprojectBindGroups::new(
            &self.descriptors.texture_views.gbuffer.motion_vectors,
            &self.descriptors.texture_views.hbgi.bent_ao,
            self.descriptors
                .texture_views
                .reproject
                .bent_ao
                .get_read_view(&self.resources.textures.reproject.bent_ao),
            &self.descriptors.texture_views.gbuffer.depth,
            &self.descriptors.texture_views.gbuffer.normal_roughness,
            self.descriptors
                .texture_views
                .reproject
                .depth_history
                .get_read_view(&self.resources.textures.reproject.depth_history),
            self.descriptors
                .texture_views
                .reproject
                .normal_history
                .get_read_view(&self.resources.textures.reproject.normal_history),
            &self.descriptors.texture_views.hbgi.near_field_irradiance,
            self.descriptors
                .texture_views
                .reproject
                .near_field_irradiance
                .get_read_view(&self.resources.textures.reproject.near_field_irradiance),
            &self.resources.buffers.hbgi_reproject_settings,
            &self.descriptors.bind_group_layouts,
            device,
        );
    }

    pub(crate) fn rotate_temporal_resources(&mut self, device: &wgpu::Device) {
        self.resources.textures.reproject.bent_ao.swap();
        self.resources
            .textures
            .reproject
            .near_field_irradiance
            .swap();
        self.resources.textures.reproject.depth_history.swap();
        self.resources.textures.reproject.normal_history.swap();
        self.refresh_temporal_bind_groups(device);
    }

    pub(crate) fn resize(&mut self, wgpu_context: &WgpuContext) {
        let new_textures = Textures::new(wgpu_context, &wgpu_context.surface_config);
        self.resources.textures.gbuffer = new_textures.gbuffer;
        self.resources.textures.hbgi = new_textures.hbgi;
        self.resources.textures.gi_blur = new_textures.gi_blur;
        self.resources.textures.reproject = new_textures.reproject;
        self.resources.textures.pyramids = new_textures.pyramids;
        self.resources.textures.sky = new_textures.sky;
        self.resources.textures.lighting_target = new_textures.lighting_target;

        self.descriptors.texture_views = TextureViews::new(&self.resources.textures);
        self.rebuild_surface_bind_groups(&wgpu_context.device);
    }
}

use wgpu::util::DeviceExt as _;

use super::{
    descriptors::{
        bg_layouts::BGLayouts, BindGroups, BonesBindGroups, CameraBindGroup, Descriptors,
        InstanceStorageBindGroup, LightsBindGroup, SsrtBindGroups, SunShadowMatrixBindGroup,
        TextureViews,
    },
    pipelines::Pipelines,
    resources::{Buffers, Resources, Samplers, Textures},
    SsrtSettingsUniform,
};
use crate::{
    fixed_snapshot::PointLightSnapshot,
    game::scene_tree::Sun,
    host::{
        assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId},
        renderer::{
            world::prepare::mesh::{BoneMat34, SkinnedInstance, StaticInstance},
            HbgiSettings,
        },
        shader_cache::ShaderCache,
        wgpu_context::WgpuContext,
    },
};

pub(crate) struct External {
    pub(crate) brdf_lut: TextureRenderId,
    pub(crate) blue_noise: TextureRenderId,
}
pub(crate) struct WorldGpuContext {
    pub(crate) external: External,
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
        external: External,
        settings: &HbgiSettings,
    ) -> Self {
        let device = &wgpu_context.device;
        let mut resources = Resources {
            textures: Textures::new(wgpu_context, &wgpu_context.surface_config),
            buffers: Buffers::new(
                wgpu_context,
                &SsrtSettingsUniform::from_settings(settings, 0),
            ),
            samplers: Samplers::new(device),
        };
        let bind_group_layouts = BGLayouts::new(wgpu_context);
        let pipelines = Pipelines::new(wgpu_context, shader_cache, &bind_group_layouts);
        let texture_views = TextureViews::new(
            &resources.textures,
            render_resources,
            placeholders,
            external.brdf_lut,
            external.blue_noise,
        );
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
            lights: LightsBindGroup::new(
                &resources.buffers.lights,
                &texture_views.external.placeholder_prefiltered,
                &resources.samplers.linear,
                &texture_views.external.placeholder_di,
                &resources.samplers.linear,
                &texture_views.external.brdf_lut,
                &resources.samplers.linear,
                &texture_views.sun_shadow.array,
                &resources.samplers.comparison,
                &bind_group_layouts.lights,
                device,
            ),
            ssrt: SsrtBindGroups::new(
                device,
                &texture_views,
                &mut resources.textures.ssrt,
                &resources.buffers,
                &resources.samplers,
                &bind_group_layouts,
                &pipelines.ssrt,
            ),
            static_instances: InstanceStorageBindGroup::new(
                &resources.buffers.static_instances,
                &bind_group_layouts.instance_storage,
                device,
            ),
            sun_shadow_matrices: std::array::from_fn(|_| {
                SunShadowMatrixBindGroup::new(device, &bind_group_layouts.sun_shadow_matrix)
            }),
        };
        let descriptors = Descriptors {
            bind_groups,
            bind_group_layouts,
            texture_views,
        };
        Self {
            external,
            resources,
            descriptors,
            pipelines,
        }
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
        environment_map_changed: bool,
        render_resources: &RenderAssetStore,
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

        if !environment_map_changed {
            return;
        }
        let (prefiltered, di) = self
            .descriptors
            .texture_views
            .external
            .resolve_environment_views(render_resources, environment_map);
        self.descriptors.bind_groups.lights = LightsBindGroup::new(
            &self.resources.buffers.lights,
            prefiltered,
            &self.resources.samplers.linear,
            di,
            &self.resources.samplers.linear,
            &self.descriptors.texture_views.external.brdf_lut,
            &self.resources.samplers.linear,
            &self.descriptors.texture_views.sun_shadow.array,
            &self.resources.samplers.comparison,
            &self.descriptors.bind_group_layouts.lights,
            &wgpu_context.device,
        );
    }

    pub(crate) fn rotate_temporal_resources(&mut self) {
        self.resources.textures.ssrt.rotate();
    }

    pub(crate) fn resize(
        &mut self,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        placeholders: &PlaceholderTextureIds,
    ) {
        self.resources.textures.gbuffer = super::resources::textures::GBufferTextures::new(
            wgpu_context,
            &wgpu_context.surface_config,
        );
        self.resources.textures.depth_pyramid =
            super::resources::textures::DepthPyramidTextures::new(
                wgpu_context,
                &wgpu_context.surface_config,
            );
        self.resources.textures.ssrt = super::resources::textures::SsrtTextures::new(
            wgpu_context,
            &wgpu_context.surface_config,
        );
        self.descriptors.texture_views = TextureViews::new(
            &self.resources.textures,
            render_resources,
            placeholders,
            self.external.brdf_lut,
            self.external.blue_noise,
        );
        self.descriptors.bind_groups.ssrt = SsrtBindGroups::new(
            &wgpu_context.device,
            &self.descriptors.texture_views,
            &mut self.resources.textures.ssrt,
            &self.resources.buffers,
            &self.resources.samplers,
            &self.descriptors.bind_group_layouts,
            &self.pipelines.ssrt,
        );
    }
}

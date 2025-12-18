use std::cmp::Ordering;
use std::collections::HashMap;
use std::f32;
use std::sync::Arc;
use std::time::Instant;

use super::pipelines::model::instance::Instance;
use super::pipelines::model::material_binding::MaterialBinding;
use super::pipelines::model::pipeline::DrawContext;
use super::render_resources::animation::{AnimationClip, Channel, Track};
use super::render_resources::{MaterialHandle, SkeletonHandle, TextureHandle};
use super::render_snapshot::{self, SnapshotGuard};
use super::wgpu_context;
use super::{render_resources::ModelHandle, render_snapshot::SnapshotHandoff};
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use wgpu::util::DeviceExt as _;

use crate::animator::{self, TimeWrapMode};
use crate::renderer::pipelines::model::pipeline::{MaterialBatch, MeshBatch, ResolvedPrimitive};
use crate::scene_tree::{self, Camera};
use crate::renderer::{
        pipelines::{
            model::pipeline::ModelPipeline,
            post_processing::PostProcessingPipeline,
            resources::{
                depth_texture::DepthTexture, msaa_textures::MSAATextures,
                skybox_output::SkyboxOutputTexture,
            },
            skybox::SkyboxPipeline,
        },
        render_resources::{RenderResources, skeletonfile},
        wgpu_context::WgpuContext,
    };

pub struct Layouts {
    pub camera: wgpu::BindGroupLayout,
    pub lights: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub bones: wgpu::BindGroupLayout,
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
        Self {
            camera,
            lights,
            material,
            bones
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoneMat34 {
    pub mat: [[f32; 4]; 3],
}
impl Default for BoneMat34 {
    fn default() -> Self {
        Self {
            mat: [
                [1f32, 0f32, 0f32, 0f32],
                [0f32, 1f32, 0f32, 0f32],
                [0f32, 0f32, 1f32, 0f32],
            ]
        }
    }
}

pub struct BonesBinding {
    pub bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
}
impl BonesBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Bones Bind Group Layout"),
        }
    }
    pub fn new(layout: &wgpu::BindGroupLayout, device: &wgpu::Device) -> Self {
        let data: Vec<BoneMat34> = vec![BoneMat34::default(); 1024];
        // TODO allocate extra space
        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bones SSBO"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bones Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }]
        });
        Self {
            bind_group,
            buffer: storage_buffer,
        }
    }
    pub fn update(&mut self, data: Vec<BoneMat34>, queue: &wgpu::Queue) {
        // TODO check if there's enough space?
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&data));
    }
}

pub struct CameraMatrices {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub inverse_view_proj_rot: [[f32; 4]; 4],
}

pub struct CameraBinding {
    view_proj_buffer: wgpu::Buffer,
    position_buffer: wgpu::Buffer,
    inverse_view_proj_rot_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl CameraBinding {
    pub fn new(
        camera: &Camera,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let matrices = CameraBinding::camera_to_matrices(camera, surface_config);
        let view_proj = matrices.view_proj;
        let position = matrices.position;
        let inverse_view_proj_rot = matrices.inverse_view_proj_rot;
        let view_proj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Projection Buffer"),
            contents: bytemuck::cast_slice(&view_proj),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let position_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Position Buffer"),
            contents: bytemuck::cast_slice(&position),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let inverse_view_proj_rot_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Inverse View Projection Buffer"),
                contents: bytemuck::cast_slice(&inverse_view_proj_rot),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_proj_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: position_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inverse_view_proj_rot_buffer.as_entire_binding(),
                },
            ],
            label: Some("Camera Bind Group"),
        });

        CameraBinding {
            bind_group,
            view_proj_buffer,
            position_buffer,
            inverse_view_proj_rot_buffer,
        }
    }

    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Camera Bind Group Layout"),
        }
    }

    fn camera_to_matrices(
        cam: &Camera,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> CameraMatrices {
        let rot = Quat::from_rotation_y((cam.rot_x).to_radians())
            * Quat::from_rotation_x((cam.rot_y).to_radians());
        let eye_rotated: Vec3 = rot * cam.eye;
        let view = Mat4::look_at_rh(eye_rotated, cam.target, cam.up);

        let aspect = surface_config.width as f32 / surface_config.height as f32;
        // cam.fovy expected in radians (use cam.fovy.to_radians() if it’s degrees).
        let proj = Mat4::perspective_rh(cam.fovy, aspect, cam.znear, cam.zfar);

        let view_proj: Mat4 = wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;

        // Upper-left 3×3, inverted (no transpose here; cgmath code didn’t transpose either)
        let m3 = Mat3::from_mat4(view_proj).inverse();

        // Rebuild a 4×4 with zeroed last row/col (to match your cgmath layout exactly).
        let inverse_view_proj_rot = Mat4::from_cols(
            Vec4::new(m3.x_axis.x, m3.x_axis.y, m3.x_axis.z, 0.0),
            Vec4::new(m3.y_axis.x, m3.y_axis.y, m3.y_axis.z, 0.0),
            Vec4::new(m3.z_axis.x, m3.z_axis.y, m3.z_axis.z, 0.0),
            Vec4::ZERO,
        );

        CameraMatrices {
            view_proj: view_proj.to_cols_array_2d(),
            position: eye_rotated.to_array(),
            inverse_view_proj_rot: inverse_view_proj_rot.to_cols_array_2d(),
        }
    }

    pub fn update(
        &self,
        camera: &Camera,
        queue: &wgpu::Queue,
        surface_config: &wgpu::SurfaceConfiguration,
    ) {
        let matrices = CameraBinding::camera_to_matrices(camera, surface_config);
        queue.write_buffer(
            &self.view_proj_buffer,
            0,
            bytemuck::cast_slice(&matrices.view_proj),
        );
        queue.write_buffer(
            &self.position_buffer,
            0,
            bytemuck::cast_slice(&matrices.position),
        );
        queue.write_buffer(
            &self.inverse_view_proj_rot_buffer,
            0,
            bytemuck::cast_slice(&matrices.inverse_view_proj_rot),
        );
    }
}

pub struct LightsBinding {
    sun_direction_buffer: wgpu::Buffer,
    sun_color_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl LightsBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // sun dir
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // sun color
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // prefiltered
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Diffuse irradiance
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // BRDF LUT
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Lights Group Layout"),
        }
    }

    pub fn new(
        sun: &scene_tree::Sun,
        wgpu_context: &WgpuContext,
        render_resources: &RenderResources,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let direction_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Direction Buffer"),
            contents: bytemuck::cast_slice(&sun.direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let color_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Color Buffer"),
            contents: bytemuck::cast_slice(&sun.color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let prefiltered = &render_resources.sampled_textures.get(&RenderResources::get_prefiltered_placeholder()).unwrap();
        let di = &render_resources.sampled_textures.get(&RenderResources::get_di_placeholder()).unwrap();
        let brdf = &render_resources.sampled_textures.get(&RenderResources::get_brdf_placeholder()).unwrap();
        let bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: direction_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&prefiltered.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&prefiltered.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&di.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&di.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&brdf.view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&brdf.sampler),
                },
            ],
            label: Some("Lights Bind Group"),
        });

        Self {
            sun_direction_buffer: direction_buffer,
            sun_color_buffer: color_buffer,
            bind_group,
        }
    }

    pub fn update_sun(&self, sun: &scene_tree::Sun, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.sun_direction_buffer,
            0,
            bytemuck::cast_slice(&sun.direction),
        );
        queue.write_buffer(&self.sun_color_buffer, 0, bytemuck::cast_slice(&sun.color));
    }

    pub fn update_environment_map(
        &mut self,
        wgpu_context: &WgpuContext,
        bind_group_layout: &wgpu::BindGroupLayout,
        prefiltered_view: &wgpu::TextureView,
        prefiltered_sampler: &wgpu::Sampler,
        di_view: &wgpu::TextureView,
        di_sampler: &wgpu::Sampler,
        brdf_view: &wgpu::TextureView,
        brdf_sampler: &wgpu::Sampler,
    ) {
        self.bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.sun_direction_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.sun_color_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&prefiltered_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&prefiltered_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&di_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&di_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&brdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&brdf_sampler),
                },
            ],
            label: Some("Lights Bind Group"),
        });
    }
}

pub struct Instances {
    pub buffer: wgpu::Buffer,
}
impl Instances {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let instance_buffer = wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance buffer"),
                contents: bytemuck::cast_slice(&vec![Mat4::IDENTITY]),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        Self {
            buffer: instance_buffer,
        }
    }

    pub fn update(&mut self, data: Vec<Instance>, queue: &wgpu::Queue, device: &wgpu::Device) {
        let instance_bytes: &[u8] = bytemuck::cast_slice(&data);
        if self.buffer.size() >= instance_bytes.len() as u64 {
            queue.write_buffer(&self.buffer, 0, instance_bytes);
        } else {
            self.buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance buffer"),
                contents: instance_bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
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
    layouts: Layouts,
    bones: BonesBinding,
    pub camera: CameraBinding,
    lights: LightsBinding,
    instances: Instances,
}
impl Renderer {
    pub fn new(
        wgpu_context: &WgpuContext,
        render_resources: &RenderResources,
        snapshot_handoff: Arc<SnapshotHandoff>,
        layouts: Layouts, // temporarily initializing layouts outside of renderer for preloading...
    ) -> Self {
        let mut lights = LightsBinding::new(
            &scene_tree::Sun::default(),
            wgpu_context,
            render_resources,
            &layouts.lights,
        );
        // TODO: read environment map from initial scene
        let prefiltered = render_resources.sampled_textures.get(&TextureHandle("assets/kloofendal_overcast_puresky_8k.prefiltered.dds".to_string())).unwrap();
        let di = render_resources.sampled_textures.get(&TextureHandle("assets/kloofendal_overcast_puresky_8k.di.dds".to_string())).unwrap();
        let brdf = render_resources.sampled_textures.get(&TextureHandle("assets/brdf_lut.png".to_string())).unwrap();
        lights.update_environment_map(wgpu_context, &layouts.lights, &prefiltered.view, &prefiltered.sampler, &di.view, &di.sampler, &brdf.view, &brdf.sampler);

        let camera = CameraBinding::new(
            &Camera::default(),
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
            &wgpu_context.device,
            &layouts.camera,
            &layouts.lights,
        );
        let model_pipeline = ModelPipeline::new(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            &layouts.camera,
            &layouts.lights,
            &layouts.bones,
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
            layouts,
            bones,
            camera,
            lights,
            instances,
        }
    }

    pub fn render(
        &mut self,
        render_resources: &mut RenderResources,
        wgpu_context: &WgpuContext,
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
        prepare_lights(&snaps.curr, render_resources, &wgpu_context.queue);

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

        let draw_context = resolve_model_draw(&mut self.bones, &mut self.instances, &snaps, t, render_resources, &wgpu_context.device, &wgpu_context.queue);

        self.model_pipeline.render(
            draw_context,
            render_resources,
            &self.instances.buffer,
            &mut encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            &self.camera.bind_group,
            &self.lights.bind_group,
            &self.bones.bind_group,
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

fn lerpf32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation on a wrapped 0..1 range.
/// `a`, `b` in [0,1); `t` in [0,1].
fn lerp_wrap_unit(a: f32, b: f32, t: f32) -> f32 {
    let mut delta = b - a;
    // Pick shortest direction around the wrap
    if delta > 0.5 {
        delta -= 1.0;
    } else if delta < -0.5 {
        delta += 1.0;
    }
    // Step and wrap back into [0,1)
    (a + delta * t).rem_euclid(1.0)
}

pub fn prepare_camera(
    camera: &mut CameraBinding,
    snaps: &SnapshotGuard,
    t: f32,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
) {
    let prev = &snaps.prev.camera;
    let curr = &snaps.curr.camera;
    let interpolated_camera = Camera {
        eye: prev.eye.lerp(curr.eye, t),
        target: prev.target.lerp(curr.target, t),
        up: prev.up.lerp(curr.up, t),
        fovy: lerpf32(prev.fovy, curr.fovy, t),
        znear: lerpf32(prev.znear, curr.znear, t),
        zfar: lerpf32(prev.zfar, curr.zfar, t),
        rot_x: lerpf32(prev.rot_x, curr.rot_x, t),
        rot_y: lerpf32(prev.rot_y, curr.rot_y, t),
    };
    camera.update(&interpolated_camera, queue, surface_config);
}

pub fn prepare_lights(
    snap: &render_snapshot::RenderSnapshot,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
    queue: &wgpu::Queue,
) {
    // TODO
    /*
    if let Some(sun) = &snap.sun {
        render_resources.lights.update_sun(sun, queue);
    }
    render_resources.lights.update_environment_map(&snap.environment_map, queue);
    */
}

struct UnresolvedPrimitive {
    transforms: Vec<Mat4>,
    palette_offset: u32,
    index_start: u32,
    index_count: u32,
    base_vertex: i32,
}

pub fn resolve_model_draw(
    bones: &mut BonesBinding,
    instances: &mut Instances,
    snaps: &SnapshotGuard,
    t: f32,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> DrawContext {
    // Joints are sorted per model-instance, and each primitive-instance refers to the base joint offset
    // of the matching model-instance
    let mut joint_palette: Vec<BoneMat34> = vec![];

    // instances are sorted in draw-order
    // material > model > primitive > primitive-instance
    let mut instance_data = vec![];

    let mut unresolved = HashMap::<MaterialHandle, HashMap<ModelHandle, Vec<UnresolvedPrimitive>>>::new();

    // write joint_palette, since it's in model-order
    // collect transforms and joint palette offsets so they can be written in draw-order
    // into the instance buffer
    for (model_handle, model_instances) in &snaps.curr.model_instances {
        let model = render_resources.models.get(&model_handle).unwrap();
        for (node_idx, curr_instance) in model_instances {
            let model_instance_world =
                if let Some(prev_transform) = &snaps
                    .prev
                    .model_instances
                    .get(model_handle)
                    .and_then(|nodes| nodes.get(node_idx))
                    .map(|node| node.transform)
                {
                    let (s1, r1, t1) = prev_transform.to_scale_rotation_translation();
                    let (s2, r2, t2) = curr_instance.transform.to_scale_rotation_translation();
                    let s3 = s1.lerp(s2, t);
                    let r3 = r1.slerp(r2, t);
                    let t3 = t1.lerp(t2, t);
                    Mat4::from_scale_rotation_translation(s3, r3, t3)
                } else {
                    curr_instance.transform.clone()
                };

            let joint_matrices = {
                let skeleton_handle = SkeletonHandle(
                    model
                        .json
                        .skeletonfile_path
                        .clone(),
                );
                let skeleton = render_resources.skeletons.get(&skeleton_handle).unwrap();
                let anim_snapshot = curr_instance.animation.as_ref().unwrap();
                let anim_data = match &anim_snapshot {
                    crate::animator::AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) => {
                        let anim_handle = &model.animations[animation_state_snapshot.clip_idx as usize];
                        let clip = render_resources.animations.get(anim_handle).unwrap();
                        let clip_time = &snaps.prev.model_instances
                            .get(model_handle)
                            .and_then(|nodes| nodes.get(node_idx))
                            .and_then(|node| node.animation.as_ref())
                            .map(|prev_anim_snap| match prev_anim_snap {
                                crate::animator::AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) =>
                                    animation_state_snapshot.animation_time,
                                crate::animator::AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) =>
                                    animation_transition_snapshot.to_time,
                            })
                            .map(|prev_time| lerpf32(prev_time, animation_state_snapshot.animation_time, t))
                            .unwrap_or(0f32);
                        AnimationData::Single(SingleAnimationData { clip, time: *clip_time, time_wrap_mode: animation_state_snapshot.time_wrap })
                    },
                    crate::animator::AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) => {
                        let from_handle = &model.animations[animation_transition_snapshot.from_clip_idx as usize];
                        let from_clip = render_resources.animations.get(from_handle).unwrap();
                        let to_handle = &model.animations[animation_transition_snapshot.to_clip_idx as usize];
                        let to_clip = render_resources.animations.get(to_handle).unwrap();
                        let blend_time = animation_transition_snapshot.blend_time;

                        let prev_times = &snaps.prev.model_instances
                            .get(model_handle)
                            .and_then(|nodes| nodes.get(node_idx))
                            .and_then(|node| node.animation.as_ref())
                            .map(|prev_anim_snap| match prev_anim_snap {
                                crate::animator::AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) =>
                                    (animation_state_snapshot.animation_time, 0.0),
                                crate::animator::AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) =>
                                    (animation_transition_snapshot.from_time, animation_transition_snapshot.to_time)
                            }).unwrap_or((0.0, 0.0));
                        let from_time = lerpf32(prev_times.0, animation_transition_snapshot.from_time, t);
                        let to_time = lerpf32(prev_times.1, animation_transition_snapshot.to_time, t);

                        let from_time_wrap_mode = animation_transition_snapshot.from_time_wrap;
                        let to_time_wrap_mode = animation_transition_snapshot.to_time_wrap;

                        AnimationData::Blend(BlendAnimationData { from_clip, to_clip, from_time, to_time, blend_time, to_time_wrap_mode, from_time_wrap_mode })
                    },
                };
                compute_joint_matrices(skeleton, anim_data)
            };
            let palette_offset = joint_palette.len() as u32;
            joint_palette.extend_from_slice(&joint_matrices);

            // primitive-instances
            for prim in &model.json.primitives {
                let mat = &model.materials[prim.material as usize];
                let mut transforms = vec![];
                for prim_inst in &prim.instances {
                    let prim_inst_m4 = Mat4::from_cols_array_2d(prim_inst);
                    let prim_inst_world = prim_inst_m4 * model_instance_world;
                    transforms.push(prim_inst_world);
                }
                let u = UnresolvedPrimitive {
                    transforms,
                    palette_offset,
                    index_start: prim.index_byte_offset / 4,
                    index_count: prim.index_byte_length / 4,
                    base_vertex: prim.base_vertex as i32,
                };
                if let Some(um) = unresolved.get_mut(&mat) {
                    if let Some(p) = um.get_mut(&model_handle) {
                        p.push(u);
                    } else {
                        um.insert(model_handle.clone(), vec![u]);
                    }
                } else {
                    unresolved.insert(mat.clone(), {
                        let mut um = HashMap::new();
                        um.insert(model_handle.clone(), vec![u]);
                        um
                    });
                }
            }
        }
    }

    let mut draws = vec![];
    let mut mesh_batches = vec![];
    let mut material_batches = vec![];

    for (material, model_map) in unresolved {
        let material_batch = MaterialBatch {
            material: material.clone(),
            mesh_range: mesh_batches.len()..mesh_batches.len()+model_map.len()
        };
        material_batches.push(material_batch);
        for (model_handle, unresolved_prims) in model_map {
            let mesh_batch = MeshBatch {
                mesh: model_handle.clone(),
                draw_range: draws.len()..draws.len()+unresolved_prims.len()
            };
            mesh_batches.push(mesh_batch);
            for prim in unresolved_prims {
                let resolved_prim = ResolvedPrimitive {
                    index_start: prim.index_start,
                    index_count: prim.index_count,
                    base_vertex: prim.base_vertex,
                    instance_base: instance_data.len() as u32,
                    instance_count: prim.transforms.len() as u32,
                };
                draws.push(resolved_prim);
                for transform in &prim.transforms {
                    let instance = Instance::new(*transform, prim.palette_offset);
                    instance_data.push(instance);
                }
            }
        }
    }

    bones.update(joint_palette, queue);
    instances.update(instance_data, queue, device);

    DrawContext {
        draws, material_batches, mesh_batches
    }
}

fn mat4_to_bone_mat34(m: Mat4) -> BoneMat34 {
    let cols = m.to_cols_array_2d();
    BoneMat34 {
        mat: [
            [cols[0][0], cols[1][0], cols[2][0], cols[3][0]],
            [cols[0][1], cols[1][1], cols[2][1], cols[3][1]],
            [cols[0][2], cols[1][2], cols[2][2], cols[3][2]],
        ],
    }
}

fn bin_search_anim_indices(times: &[f32], val: f32) -> (usize, usize) {
    let n = times.len();
    if n == 0 {
        return (0, 0);
    }
    if n == 1 {
        return (0, 0);
    }

    match times.binary_search_by(|x| x.partial_cmp(&val).unwrap_or(Ordering::Greater)) {
        Ok(i) => (i, i),          // exact hit, no blend
        Err(0) => (0, 0),         // before first, clamp
        Err(i) if i >= n => (n - 1, n - 1), // after last, clamp
        Err(i) => (i - 1, i),     // between i-1 and i
    }
}

fn compute_keyframe_values<'a, T>(times: &Box<[f32]>, values: &'a Box<[T]>, t: f32) -> (&'a T, &'a T, f32) {
    let (i0, i1) = bin_search_anim_indices(times, t);
    let (t0, t1) = (times[i0], times[i1]);
    let (v0, v1) = (&values[i0], &values[i1]);
    let alpha = if i0 == i1 || (t1 - t0).abs() < f32::EPSILON {
        0.0
    } else {
        (t - t0) / (t1 - t0) // normalized interpolation factor
    };
    (v0, v1, alpha)
}

fn interpolate_channel_value_vec3(track: &Track,channel: &Channel<Vec3>, t: f32) -> Vec3 {
    let times = channel.times.as_ref().or(track.shared_times.as_ref()).unwrap();
    let values = &channel.values;
    let (v0, v1, alpha) = compute_keyframe_values(times, values, t);
    match channel.interpolation {
        super::render_resources::animationfile::Interpolation::Linear => v0.lerp(*v1, alpha),
        super::render_resources::animationfile::Interpolation::Step => *v0,
        super::render_resources::animationfile::Interpolation::CubicSpline => todo!(),
    }
}

fn interpolate_channel_value_quat(track: &Track,channel: &Channel<Quat>, t: f32) -> Quat {
    let times = channel.times.as_ref().or(track.shared_times.as_ref()).unwrap();
    let values = &channel.values;
    let (v0, v1, alpha) = compute_keyframe_values(times, values, t);
    match channel.interpolation {
        super::render_resources::animationfile::Interpolation::Linear => v0.lerp(*v1, alpha),
        super::render_resources::animationfile::Interpolation::Step => *v0,
        super::render_resources::animationfile::Interpolation::CubicSpline => todo!(),
    }
}

/// SRT form
fn compute_animated_pose(animation: &AnimationClip, skeleton: &skeletonfile::Skeleton, base_locals: &Vec<(Vec3, Quat, Vec3)>, animation_time: f32, time_wrap_mode: &animator::TimeWrapMode) -> Vec<Option<(Vec3, Quat, Vec3)>> {
    let mut joints: Vec<Option<(Vec3, Quat, Vec3)>> = vec![None; skeleton.joints.len()];
    for track in &animation.tracks {
        let t = if animation.duration <= f32::EPSILON {
            0.0
        } else {
            match time_wrap_mode {
                animator::TimeWrapMode::Clamp => animation_time.clamp(0.0, animation.duration),
                animator::TimeWrapMode::Repeat => animation_time.rem_euclid(animation.duration),
                animator::TimeWrapMode::PingPong => {
                    let period = animation.duration * 2.0;
                    let t2 = animation_time.rem_euclid(period);
                    if t2 <= animation.duration { t2 } else { period - t2 }
                },
            }
        };
        let translation = track.translation.as_ref().map(|channel| interpolate_channel_value_vec3(track, channel, t));
        let rotation = track.rotation.as_ref().map(|channel| interpolate_channel_value_quat(track, channel, t));
        let scale = track.scale.as_ref().map(|channel| interpolate_channel_value_vec3(track, channel, t));

        match track.target {
            super::render_resources::animationfile::Target::PrimitiveGroup(_) => todo!(),
            super::render_resources::animationfile::Target::SkeletonJoint(idx) => {
                let base = base_locals[idx as usize];
                joints[idx as usize] = Some(
                    (
                        scale.unwrap_or(base.0),
                        rotation.unwrap_or(base.1),
                        translation.unwrap_or(base.2)
                    )
                );
            },
        }
    }
    joints
}

struct SingleAnimationData<'a> {
    clip: &'a AnimationClip,
    time: f32,
    time_wrap_mode: TimeWrapMode,
}

struct BlendAnimationData<'a> {
    from_clip: &'a AnimationClip,
    to_clip: &'a AnimationClip,
    from_time: f32,
    to_time: f32,
    blend_time: f32,
    to_time_wrap_mode: TimeWrapMode,
    from_time_wrap_mode: TimeWrapMode,
}

enum AnimationData<'a> {
    Single(SingleAnimationData<'a>),
    Blend(BlendAnimationData<'a>)
}
fn compute_joint_matrices<'a>(skeleton: &skeletonfile::Skeleton, animation: AnimationData<'a>) -> Vec<BoneMat34> {
    let joint_count = skeleton.joints.len();
    if joint_count == 0 {
        return vec![];
    }

    // TODO add a "roots" array to skeletonfile so we can get rid of this step entirely
    // this parents vec is only used for identifying roots
    let mut parents: Vec<Option<usize>> = vec![None; joint_count];
    for (idx, joint) in skeleton.joints.iter().enumerate() {
        for child in &joint.children {
            parents[*child as usize] = Some(idx);
        }
    }

    let mut global_transforms = vec![Mat4::IDENTITY; joint_count];
    let mut stack: Vec<(usize, Mat4)> = parents
        .iter()
        .enumerate()
        .filter_map(|(idx, parent)| if parent.is_none() { Some((idx, Mat4::IDENTITY)) } else { None })
        .collect();

    // Base joint local matrices (rest pose) in SRT form for fallback when a channel is missing.
    let base_locals: Vec<(Vec3, Quat, Vec3)> = skeleton
        .joints
        .iter()
        .map(|joint| Mat4::from_cols_array_2d(&joint.trs).to_scale_rotation_translation())
        .collect();

    let mut joint_matrices: Vec<_> = skeleton.joints.iter().map(|joint| Mat4::from_cols_array_2d(&joint.trs)).collect();
    match animation {
        AnimationData::Single(SingleAnimationData { clip, time, time_wrap_mode }) => {
            let pose = compute_animated_pose(clip, skeleton, &base_locals, time, &time_wrap_mode);
            for (idx, maybe_joint) in pose.iter().enumerate() {
                if let Some(joint) = maybe_joint {
                    joint_matrices[idx] = Mat4::from_scale_rotation_translation(joint.0, joint.1, joint.2);
                }
            }
        },
        AnimationData::Blend(BlendAnimationData { from_clip, to_clip, from_time, to_time, blend_time, to_time_wrap_mode, from_time_wrap_mode }) => {
            let pose_1 = compute_animated_pose(from_clip, skeleton, &base_locals, from_time, &from_time_wrap_mode);
            let pose_2 = compute_animated_pose(to_clip, skeleton, &base_locals, to_time, &to_time_wrap_mode);
            let blend_t = (to_time / blend_time).min(1.0);
            for idx in 0..skeleton.joints.len() {
                let maybe_joint_1 = pose_1[idx];
                let maybe_joint_2 = pose_2[idx];
                if let Some(joint_1) = maybe_joint_1 {
                    if let Some(joint_2) = maybe_joint_2 {
                        let s = joint_1.0.lerp(joint_2.0, blend_t);
                        let r = joint_1.1.slerp(joint_2.1, blend_t);
                        let t = joint_1.2.lerp(joint_2.2, blend_t);
                        joint_matrices[idx] = Mat4::from_scale_rotation_translation(s, r, t);
                    } else {
                        joint_matrices[idx] = Mat4::from_scale_rotation_translation(joint_1.0, joint_1.1, joint_1.2);
                    }
                } else if let Some(joint_2) = maybe_joint_2 {
                    joint_matrices[idx] = Mat4::from_scale_rotation_translation(joint_2.0, joint_2.1, joint_2.2);
                }
            }
        },
    }

    while let Some((idx, parent_mat)) = stack.pop() {
        let joint = &skeleton.joints[idx];
        let world = parent_mat * joint_matrices[idx];
        global_transforms[idx] = world;

        for child in &joint.children {
            stack.push((*child as usize, world));
        }
    }

    global_transforms
        .iter()
        .enumerate()
        .map(|(idx, global)| {
            let inv_bind = Mat4::from_cols_array_2d(&skeleton.joints[idx].inverse_bind_matrix);
            let skinned = *global * inv_bind;
            mat4_to_bone_mat34(skinned)
        })
        .collect()
}

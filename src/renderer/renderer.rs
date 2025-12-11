use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use super::render_resources::animation::AnimationClip;
use super::render_resources::{BoneMat34, SkeletonHandle};
use super::render_snapshot::accumulate_model_transforms;
use super::render_snapshot::{self, SnapshotGuard};
use super::{render_resources::ModelHandle, render_snapshot::SnapshotHandoff};
use generational_arena::Index;
use glam::{Mat4, Quat, Vec3, Vec4};

use crate::scene_tree::Camera;
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
        render_resources::{RenderResources, skeletonfile},
        wgpu_context::WgpuContext,
    },
    scene_tree::{RenderDataType, Scene},
};

// Common easing curves on [0,1] -> [0,1]
#[inline]
fn ease_smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
} // C2
#[inline]
fn ease_smootherstep(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
} // C3
#[inline]
fn ease_in_out_sine(t: f32) -> f32 {
    0.5 - 0.5 * (std::f32::consts::PI * t).cos()
}

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
        render_resources: &RenderResources,
        snapshot_handoff: Arc<SnapshotHandoff>,
    ) -> Self {
        let skybox_output =
            SkyboxOutputTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let depth_texture = DepthTexture::new(&wgpu_context.device, &wgpu_context.surface_config);
        let msaa_textures = MSAATextures::new(&wgpu_context.device, &wgpu_context.surface_config);

        let skybox_pipeline = SkyboxPipeline::new(
            &wgpu_context.device,
            &render_resources.layouts.camera,
            &render_resources.layouts.lights,
        );
        let model_pipeline = ModelPipeline::new(
            &wgpu_context.device,
            &wgpu_context.surface_config,
            &render_resources.layouts.camera,
            &render_resources.layouts.lights,
            &render_resources.layouts.bones,
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
        let snaps = self.snapshot_handoff.load();
        let now = Instant::now();
        let t = (now - snaps.curr_timestamp)
            .div_duration_f32(snaps.curr_timestamp - snaps.prev_timestamp);
        //let t = ease_smoothstep(t_raw); // or ease_smootherstep / ease_in_out_sine
        let models = prepare_models(
            &snaps,
            t,
            render_resources,
            &wgpu_context.device,
            &wgpu_context.queue,
        );
        prepare_camera(
            &snaps,
            t,
            render_resources,
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

        if let Some(lights) = render_resources.lights.as_ref() {
            self.skybox_pipeline.render(
                &mut encoder,
                &self.skybox_output.view,
                &render_resources.camera.bind_group,
                &lights.bind_group,
            );
        }

        self.model_pipeline.render(
            &mut encoder,
            &self.msaa_textures.msaa_texture_view,
            &self.msaa_textures.resolve_texture_view,
            &self.depth_texture.view,
            render_resources,
            models,
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
    snaps: &SnapshotGuard,
    t: f32,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
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
    render_resources
        .camera
        .update(&interpolated_camera, queue, surface_config);
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

pub fn prepare_models<'a>(
    snaps: &'a SnapshotGuard,
    t: f32,
    render_resources: &mut crate::renderer::render_resources::RenderResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> impl Iterator<Item = &'a ModelHandle> + 'a {
    let mut joint_palette: Vec<BoneMat34> = vec![];
    let mut palette_offset = 0u32;

    for (model_handle, model_instances) in &snaps.curr.model_instances {
        let mut instance_data = vec![];

        for (node_idx, curr_instance) in model_instances {
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
                instance_data.push((Mat4::from_scale_rotation_translation(s3, r3, t3), palette_offset));
            } else {
                instance_data.push((curr_instance.transform.clone(), palette_offset));
            }

            let joint_matrices = {
                let model_data = render_resources.models.get(model_handle).unwrap();
                let skeleton_handle = SkeletonHandle(
                    model_data
                        .json
                        .skeletonfile_path
                        .clone(),
                );
                let skeleton = render_resources.skeletons.get(&skeleton_handle).unwrap();
                let anim_instance = curr_instance.animation.as_ref().unwrap();
                let anim_handle = &model_data.animations[anim_instance.clip_idx as usize];
                let anim_resource = render_resources.animations.get(anim_handle).unwrap();
                let clip_time = &snaps.prev.model_instances
                    .get(model_handle)
                    .and_then(|nodes| nodes.get(node_idx))
                    .and_then(|node| node.animation.as_ref())
                    .map(|prev_anim| lerp_wrap_unit(prev_anim.clip_time, anim_instance.clip_time, t))
                    .unwrap_or(0f32);
                calculate_joint_matrices(skeleton, anim_resource, *clip_time)
            };

            joint_palette.extend_from_slice(&joint_matrices);
            palette_offset += joint_matrices.len() as u32;
        }
        render_resources
            .models
            .get_mut(&model_handle)
            .unwrap()
            .update_instance_buffer(device, queue, &instance_data);
    }

    render_resources.bones.update(joint_palette, queue);

    // return model handles that should be rendered
    snaps.curr.model_instances.iter().map(|(handle, _)| handle)
}

fn mat4_from_row_major(m: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(m[0][0], m[1][0], m[2][0], m[3][0]),
        Vec4::new(m[0][1], m[1][1], m[2][1], m[3][1]),
        Vec4::new(m[0][2], m[1][2], m[2][2], m[3][2]),
        Vec4::new(m[0][3], m[1][3], m[2][3], m[3][3]),
    )
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

fn calculate_joint_matrices(skeleton: &skeletonfile::Skeleton, animation: &AnimationClip, current_animation_time: f32) -> Vec<BoneMat34> {
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
    for track in &animation.tracks {
        // current_animation_time is normalized 0..1; wrap to duration and clamp to last key
        let t = (current_animation_time % 1.0) * animation.duration;
        let translation = track.translation.as_ref().map(|channel| {
            let times = channel.times.as_ref().unwrap_or(track.shared_times.as_ref().unwrap());
            let values = &channel.values;
            let (i0, i1) = bin_search_anim_indices(times, t);
            let (t0, t1) = (times[i0], times[i1]);
            let (v0, v1) = (values[i0], values[i1]);
            let alpha = if i0 == i1 || (t1 - t0).abs() < f32::EPSILON {
                0.0
            } else {
                (t - t0) / (t1 - t0) // normalized interpolation factor
            };
            match channel.interpolation {
                super::render_resources::animationfile::Interpolation::Linear => v0.lerp(v1, alpha),
                super::render_resources::animationfile::Interpolation::Step => v0,
                super::render_resources::animationfile::Interpolation::CubicSpline => todo!(),
            }
        });

        let rotation = track.rotation.as_ref().map(|channel| {
            let times = channel.times.as_ref().unwrap_or(track.shared_times.as_ref().unwrap());
            let values = &channel.values;
            let (i0, i1) = bin_search_anim_indices(times, t);
            let (t0, t1) = (times[i0], times[i1]);
            let (v0, v1) = (values[i0], values[i1]);
            let alpha = if i0 == i1 || (t1 - t0).abs() < f32::EPSILON {
                0.0
            } else {
                (t - t0) / (t1 - t0) // normalized interpolation factor
            };
            match channel.interpolation {
                super::render_resources::animationfile::Interpolation::Linear => v0.slerp(v1, alpha),
                super::render_resources::animationfile::Interpolation::Step => v0,
                super::render_resources::animationfile::Interpolation::CubicSpline => todo!(),
            }
        });

        let scale = track.scale.as_ref().map(|channel| {
            let times = channel.times.as_ref().unwrap_or(track.shared_times.as_ref().unwrap());
            let values = &channel.values;
            let (i0, i1) = bin_search_anim_indices(times, t);
            let (t0, t1) = (times[i0], times[i1]);
            let (v0, v1) = (values[i0], values[i1]);
            let alpha = if i0 == i1 || (t1 - t0).abs() < f32::EPSILON {
                0.0
            } else {
                (t - t0) / (t1 - t0) // normalized interpolation factor
            };
            match channel.interpolation {
                super::render_resources::animationfile::Interpolation::Linear => v0.lerp(v1, alpha),
                super::render_resources::animationfile::Interpolation::Step => v0,
                super::render_resources::animationfile::Interpolation::CubicSpline => todo!(),
            }
        });

        match track.target {
            super::render_resources::animationfile::Target::PrimitiveGroup(_) => todo!(),
            super::render_resources::animationfile::Target::SkeletonJoint(idx) => {
                let base = base_locals[idx as usize];
                joint_matrices[idx as usize] = Mat4::from_scale_rotation_translation(
                    scale.unwrap_or(base.0),
                    rotation.unwrap_or(base.1),
                    translation.unwrap_or(base.2)
                );
            },
        }
    }

    while let Some((idx, parent_mat)) = stack.pop() {
        let joint = &skeleton.joints[idx];
        let local = joint_matrices[idx]; // Mat4::from_cols_array_2d(&joint.trs);
        let world = parent_mat * local;
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

use std::{cmp::Ordering, sync::Arc};

use glam::{Mat4, Quat, Vec3};

use crate::{job_system::worker_pool::{AnimPoseTask, AnimPoseTaskResult, BlendPoseTask, RenderResponse, SinglePoseTask}, main::{pose_storage::{PoseData, TRS}, utils::QuatExt}, resource_system::{animation::{AnimationClip, Channel, Track}, file_formats::{animationfile, skeletonfile}}, game::{animator, scene_tree::SceneNodeId}};

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
        animationfile::Interpolation::Linear => v0.lerp(*v1, alpha),
        animationfile::Interpolation::Step => *v0,
        animationfile::Interpolation::CubicSpline => todo!(),
    }
}

fn interpolate_channel_value_quat(track: &Track,channel: &Channel<Quat>, t: f32) -> Quat {
    let times = channel.times.as_ref().or(track.shared_times.as_ref()).unwrap();
    let values = &channel.values;
    let (v0, v1, alpha) = compute_keyframe_values(times, values, t);
    match channel.interpolation {
        animationfile::Interpolation::Linear => v0.lerp(*v1, alpha),
        animationfile::Interpolation::Step => *v0,
        animationfile::Interpolation::CubicSpline => todo!(),
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
            animationfile::Target::PrimitiveGroup(_) => todo!(),
            animationfile::Target::SkeletonJoint(idx) => {
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

fn compute_joint_trs<'a>(task: AnimPoseTask) -> Vec<TRS> {
    let skeleton = match task {
        AnimPoseTask::Single(ref single_pose_task) => single_pose_task.skeleton.clone(),
        AnimPoseTask::Blend(ref blend_pose_task) => blend_pose_task.skeleton.clone(),
    };
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
    match task {
        AnimPoseTask::Single(SinglePoseTask { instance_time, skeleton, clip, time_wrap, boundary_mode, local_time }) => {
            let pose = compute_animated_pose(&clip, &skeleton, &base_locals, local_time, &time_wrap);
            for (idx, maybe_joint) in pose.iter().enumerate() {
                if let Some(joint) = maybe_joint {
                    joint_matrices[idx] = Mat4::from_scale_rotation_translation(joint.0, joint.1, joint.2);
                }
            }
        },
        AnimPoseTask::Blend(BlendPoseTask { instance_time, skeleton, from_clip, to_clip, blend_time, from_time, to_time, from_time_wrap, to_time_wrap }) => {
            let pose_1 = compute_animated_pose(&from_clip, &skeleton, &base_locals, from_time, &from_time_wrap);
            let pose_2 = compute_animated_pose(&to_clip, &skeleton, &base_locals, to_time, &to_time_wrap);
            let blend_t = (to_time / blend_time).min(1.0);
            for idx in 0..skeleton.joints.len() {
                let maybe_joint_1 = pose_1[idx];
                let maybe_joint_2 = pose_2[idx];
                if let Some(joint_1) = maybe_joint_1 {
                    if let Some(joint_2) = maybe_joint_2 {
                        let s = joint_1.0.lerp(joint_2.0, blend_t);
                        let r = joint_1.1.nlerp(joint_2.1, blend_t);
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
            let (s, r, t) = skinned.to_scale_rotation_translation();
            TRS { t, r, s }
        })
        .collect()
}

pub fn execute_pose_tasks(node_id: SceneNodeId, tasks: Vec<AnimPoseTask>, render_tx: &mut crossbeam::channel::Sender<RenderResponse>) {
    if render_tx.send(
        RenderResponse::Pose(
            AnimPoseTaskResult {
                node_id,
                data: tasks.into_iter().map(|task| {
                    let time = match task {
                        AnimPoseTask::Single(ref t) => t.instance_time,
                        AnimPoseTask::Blend(ref t) => t.instance_time,
                    };
                    let joints = compute_joint_trs(task);
                    PoseData {
                        time,
                        joints,
                    }
                }).collect()
            }
        )
    ).is_err() {
        todo!();
    }
}

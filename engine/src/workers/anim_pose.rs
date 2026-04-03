use std::cmp::Ordering;

use crossbeam::channel as cbch;
use glam::{Mat4, Quat, Vec3};

use crate::game::assets::runtime_formats::animation::{AnimationClip, Channel, Track};
use crate::{
    game::{animator, scene_tree::SceneNodeId},
    workers::worker_pool::{
        AnimPoseTask, AnimPoseTaskResult, BlendPoseTask, RenderResponse, SinglePoseTask,
    },
    main::{
        assets::io::asset_formats::{animationfile, rigfile},
        utils::QuatExt,
        world::anim_pose_store::PoseData,
    },
};

fn bin_search_anim_indices(times: &[f32], val: f32) -> (usize, usize) {
    let n = times.len();
    if n == 0 {
        return (0, 0);
    }
    if n == 1 {
        return (0, 0);
    }

    match times.binary_search_by(|x| x.partial_cmp(&val).unwrap_or(Ordering::Greater)) {
        Ok(i) => (i, i),                    // exact hit, no blend
        Err(0) => (0, 0),                   // before first, clamp
        Err(i) if i >= n => (n - 1, n - 1), // after last, clamp
        Err(i) => (i - 1, i),               // between i-1 and i
    }
}

fn compute_keyframe_values<'a, T>(
    times: &'a [f32],
    values: &'a [T],
    t: f32,
) -> (&'a T, &'a T, f32) {
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

fn interpolate_channel_value_vec3(track: &Track, channel: &Channel<Vec3>, t: f32) -> Vec3 {
    let times = channel
        .times
        .as_ref()
        .or(track.shared_times.as_ref())
        .unwrap();
    let values = &channel.values;
    let (v0, v1, alpha) = compute_keyframe_values(times, values, t);
    match channel.interpolation {
        animationfile::Interpolation::Linear => v0.lerp(*v1, alpha),
        animationfile::Interpolation::Step => *v0,
        animationfile::Interpolation::CubicSpline => todo!(),
    }
}

fn interpolate_channel_value_quat(track: &Track, channel: &Channel<Quat>, t: f32) -> Quat {
    let times = channel
        .times
        .as_ref()
        .or(track.shared_times.as_ref())
        .unwrap();
    let values = &channel.values;
    let (v0, v1, alpha) = compute_keyframe_values(times, values, t);
    match channel.interpolation {
        animationfile::Interpolation::Linear => v0.lerp(*v1, alpha),
        animationfile::Interpolation::Step => *v0,
        animationfile::Interpolation::CubicSpline => todo!(),
    }
}

fn compute_animated_locals(
    animation: &AnimationClip,
    rig: &rigfile::Rig,
    base_locals: &[(Vec3, Quat, Vec3)],
    animation_time: f32,
    time_wrap_mode: &animator::TimeWrapMode,
) -> Vec<Option<(Vec3, Quat, Vec3)>> {
    let mut nodes: Vec<Option<(Vec3, Quat, Vec3)>> = vec![None; rig.nodes.len()];
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
                    if t2 <= animation.duration {
                        t2
                    } else {
                        period - t2
                    }
                }
            }
        };
        let translation = track
            .translation
            .as_ref()
            .map(|channel| interpolate_channel_value_vec3(track, channel, t));
        let rotation = track
            .rotation
            .as_ref()
            .map(|channel| interpolate_channel_value_quat(track, channel, t));
        let scale = track
            .scale
            .as_ref()
            .map(|channel| interpolate_channel_value_vec3(track, channel, t));

        match track.target {
            animationfile::Target::RigNode(idx) => {
                if idx as usize >= base_locals.len() {
                    continue;
                }
                let base = base_locals[idx as usize];
                nodes[idx as usize] = Some((
                    scale.unwrap_or(base.0),
                    rotation.unwrap_or(base.1),
                    translation.unwrap_or(base.2),
                ));
            }
        }
    }
    nodes
}

fn compute_node_srt(task: AnimPoseTask) -> Vec<rigfile::SRT> {
    let rig = match task {
        AnimPoseTask::Single(ref single_pose_task) => single_pose_task.rig.clone(),
        AnimPoseTask::Blend(ref blend_pose_task) => blend_pose_task.rig.clone(),
    };
    if rig.nodes.is_empty() {
        return vec![];
    }

    let base_locals: Vec<(Vec3, Quat, Vec3)> = rig
        .nodes
        .iter()
        .map(|node| node.transform.to_scale_rotation_translation())
        .collect();
    let mut local_mats: Vec<_> = rig
        .nodes
        .iter()
        .map(|node| node.transform.to_mat4())
        .collect();
    match task {
        AnimPoseTask::Single(SinglePoseTask {
            rig,
            clip,
            time_wrap,
            local_time,
            ..
        }) => {
            let pose = compute_animated_locals(&clip, &rig, &base_locals, local_time, &time_wrap);
            for (idx, maybe_node) in pose.iter().enumerate() {
                if let Some(node) = maybe_node {
                    local_mats[idx] = Mat4::from_scale_rotation_translation(node.0, node.1, node.2);
                }
            }
        }
        AnimPoseTask::Blend(BlendPoseTask {
            rig,
            from_clip,
            to_clip,
            blend_time,
            from_time,
            to_time,
            from_time_wrap,
            to_time_wrap,
            ..
        }) => {
            let pose_1 =
                compute_animated_locals(&from_clip, &rig, &base_locals, from_time, &from_time_wrap);
            let pose_2 =
                compute_animated_locals(&to_clip, &rig, &base_locals, to_time, &to_time_wrap);
            let blend_t = if blend_time <= f32::EPSILON {
                1.0
            } else {
                (to_time / blend_time).min(1.0)
            };
            for idx in 0..rig.nodes.len() {
                let maybe_node_1 = pose_1[idx];
                let maybe_node_2 = pose_2[idx];
                if let Some(node_1) = maybe_node_1 {
                    if let Some(node_2) = maybe_node_2 {
                        let s = node_1.0.lerp(node_2.0, blend_t);
                        let r = node_1.1.nlerp(node_2.1, blend_t);
                        let t = node_1.2.lerp(node_2.2, blend_t);
                        local_mats[idx] = Mat4::from_scale_rotation_translation(s, r, t);
                    } else {
                        local_mats[idx] =
                            Mat4::from_scale_rotation_translation(node_1.0, node_1.1, node_1.2);
                    }
                } else if let Some(node_2) = maybe_node_2 {
                    local_mats[idx] =
                        Mat4::from_scale_rotation_translation(node_2.0, node_2.1, node_2.2);
                }
            }
        }
    }

    let mut world_mats = vec![Mat4::IDENTITY; rig.nodes.len()];
    for (idx, node) in rig.nodes.iter().enumerate() {
        world_mats[idx] = if let Some(parent_idx) = node.parent {
            if let Some(parent_world) = world_mats.get(parent_idx as usize) {
                *parent_world * local_mats[idx]
            } else {
                local_mats[idx]
            }
        } else {
            local_mats[idx]
        };
    }

    world_mats
        .into_iter()
        .map(|world| {
            let (s, r, t) = world.to_scale_rotation_translation();
            rigfile::SRT::new(s, r, t)
        })
        .collect()
}

pub fn execute_pose_tasks(
    node_id: SceneNodeId,
    tasks: Vec<AnimPoseTask>,
    render_tx: &mut cbch::Sender<RenderResponse>,
) {
    if tasks.is_empty() {
        return;
    }
    if render_tx
        .send(RenderResponse::Pose(AnimPoseTaskResult {
            node_id,
            data: tasks
                .into_iter()
                .map(|task| {
                    let time = match task {
                        AnimPoseTask::Single(ref t) => t.instance_time,
                        AnimPoseTask::Blend(ref t) => t.instance_time,
                    };
                    let nodes = compute_node_srt(task);
                    PoseData { time, nodes }
                })
                .collect(),
        }))
        .is_err()
    {
        todo!();
    }
}

use std::{cmp::Ordering, collections::HashMap, ops::Range, sync::Arc};

use glam::{Mat4, Quat, Vec3};

use crate::{render_snapshot::{AnimationSnapshot, SnapshotGuard}, renderer::{bindgroups::bones::{BoneMat34, BonesBinding}, buffers::instance::{Instance, Instances}, pipelines::skinned::DrawContext, pose_storage::{self, PoseStorage, TRS}, utils::{lerpf32, lerpu64}}, resource_system::{animation::{AnimationClip, Channel, Track}, file_formats::{animationfile, skeletonfile}}, sim::{animator, scene_tree::SceneNodeId}};

pub fn resolve_skinned_draw<'a>(
    bones: &mut BonesBinding,
    bones_layout: &wgpu::BindGroupLayout,
    instances: &mut Instances,
    snaps: &'a SnapshotGuard,
    t: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pose_storage: &mut PoseStorage,
    frame_idx: u32,
) -> DrawContext<'a> {
    let mut joint_palette: Vec<BoneMat34> = vec![];
    let mut instance_data = vec![];
    let mut instance_ranges = vec![0..0; snaps.curr.skinned_draw_snapshot.submesh_batches.len()];

    let mut node_to_palette_offset: HashMap<SceneNodeId, u32> = HashMap::new();
    for (node_id, curr_node_inst) in &snaps.curr.model_instances {
        let prev_node_inst = snaps.prev.model_instances.get(node_id);
        let palette_offset = joint_palette.len() as u32;
        if let Some(anim_snap) = &curr_node_inst.animation {
            let snap_time = prev_node_inst
                .and_then(|node| node.animation.as_ref())
                .map(|prev| lerpu64(prev.0, anim_snap.0, t))
                .unwrap_or(anim_snap.0);
            let poses = pose_storage.get(node_id, snap_time.clone(), frame_idx);
            let joints: Vec<TRS> = match poses {
                pose_storage::GetPoseResponse::One(pose_data) => pose_data.to_vec(),
                pose_storage::GetPoseResponse::Two(time0, joints0, time1, joints1) => {
                    let nom = snap_time.saturating_sub(time0);
                    let denom = time1.saturating_sub(time0);
                    if denom == 0 {
                        joints0.to_vec()
                    } else {
                        let a = (nom as f32 / denom as f32).min(1.0).max(0.0);
                        joints0.iter().zip(joints1)
                            .map(|(trs0, trs1)| TRS { t: trs0.t.lerp(trs1.t, a), r: trs0.r.slerp(trs1.r, a), s: trs0.s.lerp(trs1.s, a) }).collect()
                    }
                }
                // don't render if animation is missing... maybe in the future fill with temp bind pose?
                pose_storage::GetPoseResponse::Nothing => continue,
            };

            for joint in joints {
                joint_palette.push(mat4_to_bone_mat34(Mat4::from_scale_rotation_translation(joint.s, joint.r, joint.t)));
            }
        } else {
            // no animation
            todo!()
        }
        node_to_palette_offset.insert(*node_id, palette_offset);
    }

    for mat_batch in &snaps.curr.skinned_draw_snapshot.material_batches {
        for mesh_batch in &snaps.curr.skinned_draw_snapshot.mesh_batches[mat_batch.mesh_range.clone()] {
            for draw_idx in mesh_batch.submesh_range.clone() {
                let curr_draw = &snaps.curr.skinned_draw_snapshot.submesh_batches[draw_idx];
                let inst_start = instance_data.len();
                for node_id in &curr_draw.instances {
                    let curr_node_inst = snaps.curr.model_instances.get(node_id).unwrap();
                    let prev_node_inst = snaps.prev.model_instances.get(node_id);

                    let curr_transforms = &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                    let maybe_prev_transforms = prev_node_inst.map(|p| &p.submesh_transforms[curr_draw.submesh_idx]);

                    if let Some(palette_offset) = node_to_palette_offset.get(node_id) {
                        if let Some(prev_transforms) = maybe_prev_transforms {
                            for idx in 0..curr_transforms.len() {
                                let TRS { t: t1, r: r1, s: s1 } = prev_transforms[idx];
                                let TRS { t: t2, r: r2, s: s2 } = curr_transforms[idx];
                                let s3 = s1.lerp(s2, t);
                                let r3 = r1.slerp(r2, t);
                                let t3 = t1.lerp(t2, t);
                                let transform = Mat4::from_scale_rotation_translation(s3, r3, t3);
                                let instance = Instance::new(transform, *palette_offset);
                                instance_data.push(instance);
                            }
                        } else {
                            for transform in curr_transforms {
                                let instance = Instance::new(Mat4::from_scale_rotation_translation(transform.s, transform.r, transform.t), *palette_offset);
                                instance_data.push(instance);
                            }
                        };
                    }
                }

                instance_ranges[draw_idx] = inst_start as u32..instance_data.len() as u32;
            }
        }
    }

    bones.update(joint_palette, bones_layout, device, queue);
    instances.update(instance_data, queue, device);

    DrawContext {
        snap: &snaps.curr.skinned_draw_snapshot,
        instance_ranges,
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

struct SingleAnimationData<'a> {
    clip: &'a AnimationClip,
    time: f32,
    time_wrap_mode: animator::TimeWrapMode,
}

struct BlendAnimationData<'a> {
    from_clip: &'a AnimationClip,
    to_clip: &'a AnimationClip,
    from_time: f32,
    to_time: f32,
    blend_time: f32,
    to_time_wrap_mode: animator::TimeWrapMode,
    from_time_wrap_mode: animator::TimeWrapMode,
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

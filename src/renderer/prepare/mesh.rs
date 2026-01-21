use std::{cmp::Ordering, collections::HashMap, ops::Range, sync::Arc};

use glam::{Mat4, Quat, Vec3};

use crate::{render_snapshot::{AnimationSnapshot, SnapshotGuard}, renderer::{bindgroups::bones::{BoneMat34, BonesBinding}, buffers::instance::{Instance, Instances}, pipelines::skinned::DrawContext, utils::lerpf32}, resource_system::{animation::{AnimationClip, Channel, Track}, file_formats::{animationfile, skeletonfile}, registry::{GameState, MaterialId, MeshId, ModelId}, render_resources::{self, MaterialRenderId, MeshRenderId, ModelRenderId, RenderResources}, resource_manager::ResourceManager}, sim::animator};

struct UnresolvedSubmesh {
    transforms: Vec<Mat4>,
    palette_offset: u32,
    index_range: Range<u32>,
    base_vertex: i32,
}

pub fn resolve_skinned_draw<'a>(
    bones: &mut BonesBinding,
    bones_layout: &wgpu::BindGroupLayout,
    instances: &mut Instances,
    snaps: &'a SnapshotGuard,
    t: f32,
    render_resources: &RenderResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> DrawContext<'a> {
    let mut joint_palette: Vec<BoneMat34> = vec![];
    let mut instance_data = vec![];
    let mut instance_ranges = vec![0..0; snaps.curr.skinned_draw_snapshot.draws.len()];

    for mat_batch in &snaps.curr.skinned_draw_snapshot.material_batches {
        for mesh_batch in &snaps.curr.skinned_draw_snapshot.mesh_batches[mat_batch.mesh_range.clone()] {
            for draw_idx in mesh_batch.draw_range.clone() {
                let node = &snaps.curr.skinned_draw_snapshot.draws[draw_idx].node;
                let curr_node_inst = snaps.curr.model_instances.get(node).unwrap();
                let prev_node_inst = snaps.prev.model_instances.get(node);

                // append joint_palette
                let joint_matrices = {
                    if let Some(anim_snap) = &curr_node_inst.animation {
                        let skeleton = render_resources.skeletons.get(curr_node_inst.skeleton.into()).unwrap();
                        let anim_data = match &anim_snap {
                            AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) => {
                                let anim_clip = render_resources.animation_clips.get(animation_state_snapshot.clip_id.into()).unwrap();
                                let anim = render_resources.animations.get(anim_clip.animation.into()).unwrap();
                                let clip_time = prev_node_inst
                                    .and_then(|node| node.animation.as_ref())
                                    .map(|prev_anim_snap| match prev_anim_snap {
                                        AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) =>
                                            animation_state_snapshot.animation_time,
                                        AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) =>
                                            animation_transition_snapshot.to_time,
                                    })
                                    .map(|prev_time| lerpf32(prev_time, animation_state_snapshot.animation_time, t))
                                    .unwrap_or(0f32);
                                AnimationData::Single(SingleAnimationData { clip: anim, time: clip_time, time_wrap_mode: animation_state_snapshot.time_wrap })
                            },
                            AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) => {
                                let from_clip = render_resources.animation_clips.get(animation_transition_snapshot.from_clip_id.into()).unwrap();
                                let to_clip = render_resources.animation_clips.get(animation_transition_snapshot.to_clip_id.into()).unwrap();
                                let from_anim = render_resources.animations.get(from_clip.animation.into()).unwrap();
                                let to_anim = render_resources.animations.get(to_clip.animation.into()).unwrap();
                                let blend_time = animation_transition_snapshot.blend_time;

                                let prev_times = &prev_node_inst
                                    .and_then(|node| node.animation.as_ref())
                                    .map(|prev_anim_snap| match prev_anim_snap {
                                        AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) =>
                                            (animation_state_snapshot.animation_time, 0.0),
                                        AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) =>
                                            (animation_transition_snapshot.from_time, animation_transition_snapshot.to_time)
                                    }).unwrap_or((0.0, 0.0));
                                let from_time = lerpf32(prev_times.0, animation_transition_snapshot.from_time, t);
                                let to_time = lerpf32(prev_times.1, animation_transition_snapshot.to_time, t);

                                let from_time_wrap_mode = animation_transition_snapshot.from_time_wrap;
                                let to_time_wrap_mode = animation_transition_snapshot.to_time_wrap;

                                AnimationData::Blend(BlendAnimationData { from_clip: from_anim, to_clip: to_anim, from_time, to_time, blend_time, to_time_wrap_mode, from_time_wrap_mode })
                            },
                        };
                        compute_joint_matrices(&skeleton, anim_data)
                    } else {
                        // no animation
                        todo!()
                    }
                };
                let palette_offset = joint_palette.len() as u32;
                joint_palette.extend_from_slice(&joint_matrices);
                // append instance_data
                // update instance_ranges
            }
        }
    }

    bones.update(joint_palette, bones_layout, device, queue);
    instances.update(instance_data, queue, device);

    DrawContext {
        snap: &snaps.curr.skinned_draw_snapshot,
        instance_ranges,
    }

    /*

    // Joints are sorted per model-instance, and each submesh-instance refers to the base joint offset
    // of the matching model-instance
    let mut joint_palette: Vec<BoneMat34> = vec![];

    // instances are sorted in draw-order
    // material > model > submesh > submesh-instance
    let mut instance_data = vec![];

    let mut unresolved = HashMap::<MaterialRenderId, HashMap<ModelRenderId, (Vec<UnresolvedSubmesh>, MeshRenderId, u32)>>::new();

    // write joint_palette, since it's in model-order
    // collect transforms and joint palette offsets so they can be written in draw-order
    // into the instance buffer
    let models = &render_resources.models;
    let skeletons = &render_resources.skeletons;
    let anim_clips = &render_resources.animation_clips;
    let anims = &render_resources.animations;
    for (model_render_id, model_instances) in &snaps.curr.model_instances {
        let model = models.get((*model_render_id).into()).unwrap();
        let prev_instances = &snaps.prev.model_instances.get(model_render_id);
        for (node_idx, curr_instance) in model_instances {
            let prev_instance = prev_instances.and_then(|nodes| nodes.get(node_idx));
            let model_instance_world =
                if let Some(prev_transform) = prev_instance
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
                let skeleton = skeletons.get(model.skeleton.into()).unwrap();
                let anim_snapshot = curr_instance.animation.as_ref().unwrap();
                let anim_data = match &anim_snapshot {
                    AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) => {
                        let anim_clip = {
                            let anim_clip_id = &model.anim_clips[animation_state_snapshot.clip_idx as usize];
                            anim_clips.get((*anim_clip_id).into()).unwrap()
                        };
                        let anim = {
                            let anim_id = &anim_clip.animation;
                            anims.get((*anim_id).into()).unwrap()
                        };
                        let clip_time = prev_instance
                            .and_then(|node| node.animation.as_ref())
                            .map(|prev_anim_snap| match prev_anim_snap {
                                AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) =>
                                    animation_state_snapshot.animation_time,
                                AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) =>
                                    animation_transition_snapshot.to_time,
                            })
                            .map(|prev_time| lerpf32(prev_time, animation_state_snapshot.animation_time, t))
                            .unwrap_or(0f32);
                        AnimationData::Single(SingleAnimationData { clip: anim, time: clip_time, time_wrap_mode: animation_state_snapshot.time_wrap })
                    },
                    AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) => {
                        let from_clip = {
                            let anim_clip_id = &model.anim_clips[animation_transition_snapshot.from_clip_idx as usize];
                            anim_clips.get((*anim_clip_id).into()).unwrap()
                        };
                        let to_clip = {
                            let anim_clip_id = &model.anim_clips[animation_transition_snapshot.to_clip_idx as usize];
                            anim_clips.get((*anim_clip_id).into()).unwrap()
                        };
                        let from_anim = {
                            let anim_id = &from_clip.animation;
                            anims.get((*anim_id).into()).unwrap()
                        };
                        let to_anim = {
                            let anim_id = &to_clip.animation;
                            anims.get((*anim_id).into()).unwrap()
                        };
                        let blend_time = animation_transition_snapshot.blend_time;

                        let prev_times = &snaps.prev.model_instances
                            .get(model_render_id)
                            .and_then(|nodes| nodes.get(node_idx))
                            .and_then(|node| node.animation.as_ref())
                            .map(|prev_anim_snap| match prev_anim_snap {
                                AnimationSnapshot::AnimationStateSnapshot(animation_state_snapshot) =>
                                    (animation_state_snapshot.animation_time, 0.0),
                                AnimationSnapshot::AnimationTransitionSnapshot(animation_transition_snapshot) =>
                                    (animation_transition_snapshot.from_time, animation_transition_snapshot.to_time)
                            }).unwrap_or((0.0, 0.0));
                        let from_time = lerpf32(prev_times.0, animation_transition_snapshot.from_time, t);
                        let to_time = lerpf32(prev_times.1, animation_transition_snapshot.to_time, t);

                        let from_time_wrap_mode = animation_transition_snapshot.from_time_wrap;
                        let to_time_wrap_mode = animation_transition_snapshot.to_time_wrap;

                        AnimationData::Blend(BlendAnimationData { from_clip: from_anim, to_clip: to_anim, from_time, to_time, blend_time, to_time_wrap_mode, from_time_wrap_mode })
                    },
                };
                compute_joint_matrices(&skeleton, anim_data)
            };
            let palette_offset = joint_palette.len() as u32;
            joint_palette.extend_from_slice(&joint_matrices);

            // submesh-instances
            for submesh in &model.submeshes {
                let mut transforms = vec![];
                for sub_inst in &submesh.instances {
                    let sub_inst_m4 = Mat4::from_cols_array_2d(sub_inst);
                    let sub_inst_world = sub_inst_m4 * model_instance_world;
                    transforms.push(sub_inst_world);
                }
                let u = UnresolvedSubmesh {
                    transforms,
                    palette_offset,
                    index_range: submesh.index_range.clone(),
                    base_vertex: submesh.base_vertex as i32,
                };
                if let Some(um) = unresolved.get_mut(&submesh.material) {
                    if let Some((p, _, _)) = um.get_mut(&model_render_id) {
                        p.push(u);
                    } else {
                        um.insert(model_render_id.clone(), (vec![u], model.mesh, model.vertex_buffer_start_offset));
                    }
                } else {
                    unresolved.insert(submesh.material, {
                        let mut um = HashMap::new();
                        um.insert(model_render_id.clone(), (vec![u], model.mesh, model.vertex_buffer_start_offset));
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
        for (model_id, (unresolved_submeshes, mesh_id, vertex_buffer_start_offset)) in model_map {
            let mesh_batch = MeshBatch {
                mesh: mesh_id,
                draw_range: draws.len()..draws.len()+unresolved_submeshes.len(),
                vertex_buffer_start_offset: vertex_buffer_start_offset as u64,
            };
            mesh_batches.push(mesh_batch);
            for sub in unresolved_submeshes {
                let resolved_submesh = ResolvedSubmesh {
                    index_range: sub.index_range,
                    base_vertex: sub.base_vertex,
                    instance_range: instance_data.len() as u32..instance_data.len() as u32 + sub.transforms.len() as u32,
                };
                draws.push(resolved_submesh);
                for transform in &sub.transforms {
                    let instance = Instance::new(*transform, sub.palette_offset);
                    instance_data.push(instance);
                }
            }
        }
    }

    bones.update(joint_palette, bones_layout, device, queue);
    instances.update(instance_data, queue, device);

    DrawContext {
        draws, material_batches, mesh_batches
    }
    */
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

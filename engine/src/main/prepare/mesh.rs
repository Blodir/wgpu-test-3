use std::{cmp::Ordering, collections::HashMap, ops::Range, sync::Arc};

use glam::{Mat4, Quat, Vec3};

use crate::{render_snapshot::{AnimationSnapshot, MeshDrawSnapshot, SnapshotGuard}, main::{bindgroups::bones::{BoneMat34, BonesBinding}, buffers::{skinned_instance::{SkinnedInstance, SkinnedInstances}, static_instance::{StaticInstance, StaticInstances}}, pose_storage::{self, PoseStorage, TRS}, utils::{QuatExt, lerpf32, lerpu64}}, resource_system::{animation::{AnimationClip, Channel, Track}, file_formats::{animationfile, skeletonfile}}, game::{animator, scene_tree::SceneNodeId}};

pub struct DrawContext<'a> {
    pub snap: &'a MeshDrawSnapshot,
    pub instance_ranges: Vec<Range<u32>>,
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

pub fn resolve_skinned_draw<'a>(
    bones: &mut BonesBinding,
    bones_layout: &wgpu::BindGroupLayout,
    instances: &mut SkinnedInstances,
    snaps: &'a SnapshotGuard,
    t: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pose_storage: &mut PoseStorage,
    frame_idx: u32,
) -> DrawContext<'a> {
    // TODO perf: maybe reuse this vec from previous frame to reduce resizing? Could call vec.clear();
    let mut joint_palette: Vec<BoneMat34> = vec![];
    let mut instance_data = vec![];
    let mut instance_ranges = vec![0..0; snaps.curr.mesh_draw_snapshot.submesh_batches.len()];

    let mut node_to_palette_offset: HashMap<SceneNodeId, u32> = HashMap::new();
    for (node_id, curr_node_inst) in &snaps.curr.mesh_draw_snapshot.skinned_instances {
        let prev_node_inst = snaps.prev.mesh_draw_snapshot.skinned_instances.get(node_id);
        let palette_offset = joint_palette.len() as u32;
        if let Some(anim_snap) = &curr_node_inst.animation {
            let snap_time = prev_node_inst
                .and_then(|node| node.animation.as_ref())
                .map(|prev| lerpu64(prev.0, anim_snap.0, t))
                .unwrap_or(anim_snap.0);
            let poses = pose_storage.get(node_id, snap_time.clone(), frame_idx);
            match poses {
                pose_storage::GetPoseResponse::One(pose_data) => joint_palette.extend(
                    pose_data.iter().map(|joint| mat4_to_bone_mat34(Mat4::from_scale_rotation_translation(joint.s, joint.r, joint.t)))
                ),
                pose_storage::GetPoseResponse::Two(time0, joints0, time1, joints1) => {
                    let nom = snap_time.saturating_sub(time0);
                    let denom = time1.saturating_sub(time0);
                    if denom == 0 {
                        joint_palette.extend(
                            joints0.iter().map(|joint| mat4_to_bone_mat34(Mat4::from_scale_rotation_translation(joint.s, joint.r, joint.t)))
                        )
                    } else {
                        let a = (nom as f32 / denom as f32).min(1.0).max(0.0);
                        joint_palette.extend(
                            joints0.iter().zip(joints1).map(|(trs0, trs1)| mat4_to_bone_mat34(
                                Mat4::from_scale_rotation_translation(
                                    trs0.s.lerp(trs1.s, a),
                                    trs0.r.nlerp(trs1.r, a),
                                    trs0.t.lerp(trs1.t, a),
                                )
                            ))
                        );
                    }
                }
                // don't render if animation is missing... maybe in the future fill with temp bind pose?
                pose_storage::GetPoseResponse::Nothing => continue,
            };
        } else {
            // no animation
            todo!()
        }
        node_to_palette_offset.insert(*node_id, palette_offset);
    }

    for mat_batch in &snaps.curr.mesh_draw_snapshot.material_batches[snaps.curr.mesh_draw_snapshot.skinned_batch.clone()] {
        for mesh_batch in &snaps.curr.mesh_draw_snapshot.mesh_batches[mat_batch.mesh_range.clone()] {
            for draw_idx in mesh_batch.submesh_range.clone() {
                let curr_draw = &snaps.curr.mesh_draw_snapshot.submesh_batches[draw_idx];
                let inst_start = instance_data.len();
                for node_id in &curr_draw.instances {
                    let curr_node_inst = snaps.curr.mesh_draw_snapshot.skinned_instances.get(node_id).unwrap();

                    // no need to interpolate if transform hasn't changed
                    if !curr_node_inst.dirty {
                        if let Some(palette_offset) = node_to_palette_offset.get(node_id) {
                            let curr_transforms = &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                            for transform in curr_transforms {
                                let instance = SkinnedInstance::new(Mat4::from_scale_rotation_translation(transform.s, transform.r, transform.t), *palette_offset);
                                instance_data.push(instance);
                            }
                        }
                        continue;
                    }

                    // otherwise: interpolate
                    let prev_node_inst = snaps.prev.mesh_draw_snapshot.skinned_instances.get(node_id);

                    let curr_transforms = &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                    let maybe_prev_transforms = prev_node_inst.map(|p| &p.submesh_transforms[curr_draw.submesh_idx]);

                    if let Some(palette_offset) = node_to_palette_offset.get(node_id) {
                        if let Some(prev_transforms) = maybe_prev_transforms {
                            for idx in 0..curr_transforms.len() {
                                let TRS { t: t1, r: r1, s: s1 } = prev_transforms[idx];
                                let TRS { t: t2, r: r2, s: s2 } = curr_transforms[idx];
                                let s3 = s1.lerp(s2, t);
                                let r3 = r1.nlerp(r2, t);
                                let t3 = t1.lerp(t2, t);
                                let transform = Mat4::from_scale_rotation_translation(s3, r3, t3);
                                let instance = SkinnedInstance::new(transform, *palette_offset);
                                instance_data.push(instance);
                            }
                        } else {
                            for transform in curr_transforms {
                                let instance = SkinnedInstance::new(Mat4::from_scale_rotation_translation(transform.s, transform.r, transform.t), *palette_offset);
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
        snap: &snaps.curr.mesh_draw_snapshot,
        instance_ranges,
    }
}

pub fn resolve_static_draw<'a>(
    instances: &mut StaticInstances,
    snaps: &'a SnapshotGuard,
    t: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> DrawContext<'a> {
    // TODO perf: maybe reuse this vec from previous frame to reduce resizing? Could call vec.clear();
    let mut instance_data = vec![];
    let mut instance_ranges = vec![0..0; snaps.curr.mesh_draw_snapshot.submesh_batches.len()];

    for mat_batch in &snaps.curr.mesh_draw_snapshot.material_batches[snaps.curr.mesh_draw_snapshot.static_batch.clone()] {
        for mesh_batch in &snaps.curr.mesh_draw_snapshot.mesh_batches[mat_batch.mesh_range.clone()] {
            for draw_idx in mesh_batch.submesh_range.clone() {
                let curr_draw = &snaps.curr.mesh_draw_snapshot.submesh_batches[draw_idx];
                let inst_start = instance_data.len();
                for node_id in &curr_draw.instances {
                    let curr_node_inst = snaps.curr.mesh_draw_snapshot.static_instances.get(node_id).unwrap();
                    // no need to interpolate if transform hasn't changed
                    if !curr_node_inst.dirty {
                        let curr_transforms = &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                        for transform in curr_transforms {
                            let instance = StaticInstance::new(Mat4::from_scale_rotation_translation(transform.s, transform.r, transform.t));
                            instance_data.push(instance);
                        }
                        continue;
                    }

                    // otherwise: interpolate
                    let prev_node_inst = snaps.prev.mesh_draw_snapshot.static_instances.get(node_id);

                    let curr_transforms = &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                    let maybe_prev_transforms = prev_node_inst.map(|p| &p.submesh_transforms[curr_draw.submesh_idx]);

                    if let Some(prev_transforms) = maybe_prev_transforms {
                        for idx in 0..curr_transforms.len() {
                            let TRS { t: t1, r: r1, s: s1 } = prev_transforms[idx];
                            let TRS { t: t2, r: r2, s: s2 } = curr_transforms[idx];
                            let s3 = s1.lerp(s2, t);
                            let r3 = r1.nlerp(r2, t);
                            let t3 = t1.lerp(t2, t);
                            let transform = Mat4::from_scale_rotation_translation(s3, r3, t3);
                            let instance = StaticInstance::new(transform);
                            instance_data.push(instance);
                        }
                    } else {
                        for transform in curr_transforms {
                            let instance = StaticInstance::new(Mat4::from_scale_rotation_translation(transform.s, transform.r, transform.t));
                            instance_data.push(instance);
                        }
                    };
                }

                instance_ranges[draw_idx] = inst_start as u32..instance_data.len() as u32;
            }
        }
    }

    instances.update(instance_data, queue, device);

    DrawContext {
        snap: &snaps.curr.mesh_draw_snapshot,
        instance_ranges,
    }
}

use std::{collections::HashMap, ops::Range};

use glam::Mat4;

use crate::game::build_snapshot::MeshDrawSnapshot;
use crate::snapshot_handoff::SnapshotGuard;
use crate::{
    game::scene_tree::SceneNodeId,
    main::{
        assets::store::RenderAssetStore,
        utils::{lerpu64, QuatExt},
        world::{
            anim_pose_store::{self, AnimPoseStore},
            bindgroups::bones::{BoneMat34, BonesBinding},
            buffers::{
                skinned_instance::{SkinnedInstance, SkinnedInstances},
                static_instance::{StaticInstance, StaticInstances},
            },
        },
    },
};

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
    render_resources: &RenderAssetStore,
    snaps: &'a SnapshotGuard,
    t: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pose_storage: &mut AnimPoseStore,
    frame_idx: u32,
) -> DrawContext<'a> {
    // TODO perf: maybe reuse this vec from previous frame to reduce resizing? Could call vec.clear();
    let mut joint_palette: Vec<BoneMat34> = vec![];
    let mut instance_data = vec![];
    let mut instance_ranges = vec![0..0; snaps.curr.mesh_draw_snapshot.submesh_batches.len()];

    let mut node_to_palette_offset: HashMap<SceneNodeId, u32> = HashMap::new();

    for mat_batch in &snaps.curr.mesh_draw_snapshot.material_batches
        [snaps.curr.mesh_draw_snapshot.skinned_batch.clone()]
    {
        for mesh_batch in &snaps.curr.mesh_draw_snapshot.mesh_batches[mat_batch.mesh_range.clone()]
        {
            let model = render_resources
                .models
                .get(mesh_batch.model_id.into())
                .unwrap();
            for draw_idx in mesh_batch.submesh_range.clone() {
                let curr_draw = &snaps.curr.mesh_draw_snapshot.submesh_batches[draw_idx];
                let inst_start = instance_data.len();
                for node_id in &curr_draw.instances {
                    let curr_node_inst = snaps
                        .curr
                        .mesh_draw_snapshot
                        .skinned_instances
                        .get(node_id)
                        .unwrap();
                    let palette_offset = if let Some(offset) = node_to_palette_offset.get(node_id) {
                        *offset
                    } else {
                        let prev_node_inst =
                            snaps.prev.mesh_draw_snapshot.skinned_instances.get(node_id);
                        let anim_snap = match &curr_node_inst.animation {
                            Some(a) => a,
                            None => continue,
                        };
                        let snap_time = prev_node_inst
                            .and_then(|node| node.animation.as_ref())
                            .map(|prev| lerpu64(prev.0, anim_snap.0, t))
                            .unwrap_or(anim_snap.0);
                        let palette_offset = joint_palette.len() as u32;
                        let poses = pose_storage.get(node_id, snap_time, frame_idx);
                        debug_assert_eq!(
                            model.joint_nodes.len(),
                            model.inverse_bind_matrices.len()
                        );
                        match poses {
                            anim_pose_store::GetPoseResponse::One(nodes) => {
                                for (node_idx, inverse_bind) in model
                                    .joint_nodes
                                    .iter()
                                    .zip(model.inverse_bind_matrices.iter())
                                {
                                    let node_mat = if let Some(node) = nodes.get(*node_idx as usize)
                                    {
                                        node.to_mat4()
                                    } else {
                                        Mat4::IDENTITY
                                    };
                                    let skin = node_mat * *inverse_bind;
                                    joint_palette.push(mat4_to_bone_mat34(skin));
                                }
                            }
                            anim_pose_store::GetPoseResponse::Two(time0, nodes0, time1, nodes1) => {
                                let nom = snap_time.saturating_sub(time0);
                                let denom = time1.saturating_sub(time0);
                                let a = if denom == 0 {
                                    0.0
                                } else {
                                    (nom as f32 / denom as f32).clamp(0.0, 1.0)
                                };
                                for (node_idx, inverse_bind) in model
                                    .joint_nodes
                                    .iter()
                                    .zip(model.inverse_bind_matrices.iter())
                                {
                                    let world = match (
                                        nodes0.get(*node_idx as usize),
                                        nodes1.get(*node_idx as usize),
                                    ) {
                                        (Some(node0), Some(node1)) => {
                                            let (s0, r0, t0) =
                                                node0.to_scale_rotation_translation();
                                            let (s1, r1, t1) =
                                                node1.to_scale_rotation_translation();
                                            Mat4::from_scale_rotation_translation(
                                                s0.lerp(s1, a),
                                                r0.nlerp(r1, a),
                                                t0.lerp(t1, a),
                                            )
                                        }
                                        (Some(node0), None) => node0.to_mat4(),
                                        (None, Some(node1)) => node1.to_mat4(),
                                        (None, None) => Mat4::IDENTITY,
                                    };
                                    let skin = world * *inverse_bind;
                                    joint_palette.push(mat4_to_bone_mat34(skin));
                                }
                            }
                            anim_pose_store::GetPoseResponse::Nothing => continue,
                        }
                        node_to_palette_offset.insert(*node_id, palette_offset);
                        palette_offset
                    };

                    // no need to interpolate if transform hasn't changed
                    if !curr_node_inst.dirty {
                        let curr_transforms =
                            &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                        for transform in curr_transforms {
                            let (s, r, t) = transform.to_scale_rotation_translation();
                            let instance = SkinnedInstance::new(
                                Mat4::from_scale_rotation_translation(s, r, t),
                                palette_offset,
                            );
                            instance_data.push(instance);
                        }
                        continue;
                    }

                    // otherwise: interpolate
                    let prev_node_inst =
                        snaps.prev.mesh_draw_snapshot.skinned_instances.get(node_id);

                    let curr_transforms = &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                    let maybe_prev_transforms =
                        prev_node_inst.map(|p| &p.submesh_transforms[curr_draw.submesh_idx]);

                    if let Some(prev_transforms) = maybe_prev_transforms {
                        for idx in 0..curr_transforms.len() {
                            let (s1, r1, t1) = prev_transforms[idx].to_scale_rotation_translation();
                            let (s2, r2, t2) = curr_transforms[idx].to_scale_rotation_translation();
                            let s3 = s1.lerp(s2, t);
                            let r3 = r1.nlerp(r2, t);
                            let t3 = t1.lerp(t2, t);
                            let transform = Mat4::from_scale_rotation_translation(s3, r3, t3);
                            let instance = SkinnedInstance::new(transform, palette_offset);
                            instance_data.push(instance);
                        }
                    } else {
                        for transform in curr_transforms {
                            let (s, r, t) = transform.to_scale_rotation_translation();
                            let instance = SkinnedInstance::new(
                                Mat4::from_scale_rotation_translation(s, r, t),
                                palette_offset,
                            );
                            instance_data.push(instance);
                        }
                    };
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

    for mat_batch in &snaps.curr.mesh_draw_snapshot.material_batches
        [snaps.curr.mesh_draw_snapshot.static_batch.clone()]
    {
        for mesh_batch in &snaps.curr.mesh_draw_snapshot.mesh_batches[mat_batch.mesh_range.clone()]
        {
            for draw_idx in mesh_batch.submesh_range.clone() {
                let curr_draw = &snaps.curr.mesh_draw_snapshot.submesh_batches[draw_idx];
                let inst_start = instance_data.len();
                for node_id in &curr_draw.instances {
                    let curr_node_inst = snaps
                        .curr
                        .mesh_draw_snapshot
                        .static_instances
                        .get(node_id)
                        .unwrap();
                    // no need to interpolate if transform hasn't changed
                    if !curr_node_inst.dirty {
                        let curr_transforms =
                            &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                        for transform in curr_transforms {
                            let (s, r, t) = transform.to_scale_rotation_translation();
                            let instance =
                                StaticInstance::new(Mat4::from_scale_rotation_translation(s, r, t));
                            instance_data.push(instance);
                        }
                        continue;
                    }

                    // otherwise: interpolate
                    let prev_node_inst =
                        snaps.prev.mesh_draw_snapshot.static_instances.get(node_id);

                    let curr_transforms = &curr_node_inst.submesh_transforms[curr_draw.submesh_idx];
                    let maybe_prev_transforms =
                        prev_node_inst.map(|p| &p.submesh_transforms[curr_draw.submesh_idx]);

                    if let Some(prev_transforms) = maybe_prev_transforms {
                        for idx in 0..curr_transforms.len() {
                            let (s1, r1, t1) = prev_transforms[idx].to_scale_rotation_translation();
                            let (s2, r2, t2) = curr_transforms[idx].to_scale_rotation_translation();
                            let s3 = s1.lerp(s2, t);
                            let r3 = r1.nlerp(r2, t);
                            let t3 = t1.lerp(t2, t);
                            let transform = Mat4::from_scale_rotation_translation(s3, r3, t3);
                            let instance = StaticInstance::new(transform);
                            instance_data.push(instance);
                        }
                    } else {
                        for transform in curr_transforms {
                            let (s, r, t) = transform.to_scale_rotation_translation();
                            let instance =
                                StaticInstance::new(Mat4::from_scale_rotation_translation(s, r, t));
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

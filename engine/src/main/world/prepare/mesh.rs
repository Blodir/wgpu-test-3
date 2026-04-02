use std::ops::Range;

use glam::Mat4;

use crate::fixed_snapshot_handoff::FixedSnapshotGuard;
use crate::game::build_snapshot::{MeshDrawSnapshot, PassBatches};
use crate::{
    game::scene_tree::SceneNodeId,
    main::{
        assets::{io::asset_formats::rigfile::SRT, store::RenderAssetStore},
        utils::{safe_lerpu64, QuatExt},
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
    pub opaque: PassDrawContext<'a>,
    pub transparent: PassDrawContext<'a>,
}

pub struct PassDrawContext<'a> {
    pub batch: &'a PassBatches,
    pub instance_ranges: Vec<Range<u32>>,
}

enum PoseNodesCacheEntry {
    Pending,
    Missing,
    Ready(Vec<Mat4>),
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

fn lerp_srt_to_mat4(from: &SRT, to: &SRT, t: f32) -> Mat4 {
    let (s0, r0, t0) = from.to_scale_rotation_translation();
    let (s1, r1, t1) = to.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(s0.lerp(s1, t), r0.nlerp(r1, t), t0.lerp(t1, t))
}

fn resolve_model_transform(curr: &SRT, prev: Option<&SRT>, dirty: bool, t: f32) -> Mat4 {
    if dirty {
        if let Some(prev) = prev {
            return lerp_srt_to_mat4(prev, curr, t);
        }
    }
    curr.to_mat4()
}

fn sample_pose_nodes(
    pose_storage: &mut AnimPoseStore,
    node_id: &SceneNodeId,
    snap_time: u64,
    frame_idx: u32,
) -> Option<Vec<Mat4>> {
    match pose_storage.get(node_id, snap_time, frame_idx) {
        anim_pose_store::GetPoseResponse::One(nodes) => {
            Some(nodes.iter().map(|n| n.to_mat4()).collect())
        }
        anim_pose_store::GetPoseResponse::Two(time0, nodes0, time1, nodes1) => {
            let nom = snap_time.saturating_sub(time0);
            let denom = time1.saturating_sub(time0);
            let a = if denom == 0 {
                0.0
            } else {
                (nom as f32 / denom as f32).clamp(0.0, 1.0)
            };
            Some(
                nodes0
                    .iter()
                    .zip(nodes1.iter())
                    .map(|(n0, n1)| lerp_srt_to_mat4(n0, n1, a))
                    .collect(),
            )
        }
        anim_pose_store::GetPoseResponse::Nothing => None,
    }
}

fn resolve_skinned_pass(
    pass_batch: &PassBatches,
    render_resources: &RenderAssetStore,
    snaps: &FixedSnapshotGuard,
    t: f32,
    pose_storage: &mut AnimPoseStore,
    frame_idx: u32,
    instance_data: &mut Vec<SkinnedInstance>,
    node_world_cache: &mut Vec<PoseNodesCacheEntry>,
    node_to_palette_offset: &mut Vec<Option<u32>>,
    joint_palette: &mut Vec<BoneMat34>,
) -> Vec<Range<u32>> {
    let mut instance_ranges = vec![0..0; pass_batch.submesh_batches.len()];
    let mat_batches = &pass_batch.material_batches[pass_batch.skinned_batch.clone()];
    for mat_batch in mat_batches {
        let mesh_batches = &pass_batch.mesh_batches[mat_batch.mesh_range.clone()];
        for mesh_batch in mesh_batches {
            let model = render_resources
                .models
                .get(mesh_batch.model_id.into())
                .unwrap();
            for submesh_batch_idx in mesh_batch.submesh_range.clone() {
                let submesh_batch = &pass_batch.submesh_batches[submesh_batch_idx];
                let instance_data_start_idx = instance_data.len();
                for instance_idx in &submesh_batch.instances {
                    let instance_idx = *instance_idx as usize;
                    let curr_inst_snap =
                        &snaps.curr.mesh_draw_snapshot.skinned_instances[instance_idx];
                    let maybe_prev_inst_snap = curr_inst_snap.prev_index.map(|prev_idx| {
                        &snaps.prev.mesh_draw_snapshot.skinned_instances[prev_idx as usize]
                    });
                    if matches!(node_world_cache[instance_idx], PoseNodesCacheEntry::Pending) {
                        let maybe_nodes = if let Some(anim_snap) = curr_inst_snap.animation {
                            let snap_time = maybe_prev_inst_snap
                                .and_then(|n| n.animation.as_ref())
                                .map(|prev| safe_lerpu64(prev.0, anim_snap.0, t))
                                .unwrap_or(anim_snap.0);
                            sample_pose_nodes(
                                pose_storage,
                                &curr_inst_snap.node_id,
                                snap_time,
                                frame_idx,
                            )
                        } else {
                            Some(model.rig.bind_matrices.clone())
                        };
                        node_world_cache[instance_idx] = match maybe_nodes {
                            Some(nodes) => PoseNodesCacheEntry::Ready(nodes),
                            None => PoseNodesCacheEntry::Missing,
                        };
                    }
                    let node_worlds = match &node_world_cache[instance_idx] {
                        PoseNodesCacheEntry::Ready(nodes) => nodes,
                        PoseNodesCacheEntry::Missing => continue,
                        PoseNodesCacheEntry::Pending => unreachable!(),
                    };

                    let palette_offset = if let Some(offset) = node_to_palette_offset[instance_idx]
                    {
                        offset
                    } else {
                        let offset = joint_palette.len() as u32;
                        for (joint_node_idx, inverse_bind) in model
                            .rig
                            .joint_nodes
                            .iter()
                            .zip(model.rig.inverse_bind_matrices.iter())
                        {
                            let node_mat = node_worlds
                                .get(*joint_node_idx as usize)
                                .copied()
                                .unwrap_or(Mat4::IDENTITY);
                            joint_palette.push(mat4_to_bone_mat34(node_mat * *inverse_bind));
                        }
                        node_to_palette_offset[instance_idx] = Some(offset);
                        offset
                    };

                    let model_transform = resolve_model_transform(
                        &curr_inst_snap.model_transform,
                        maybe_prev_inst_snap.map(|p| &p.model_transform),
                        curr_inst_snap.dirty,
                        t,
                    );
                    let instance_nodes = &model.submeshes[submesh_batch.submesh_idx].instance_nodes;
                    for instance_node_idx in instance_nodes {
                        let node_mat = node_worlds
                            .get(*instance_node_idx as usize)
                            .copied()
                            .unwrap_or(Mat4::IDENTITY);
                        instance_data.push(SkinnedInstance::new(
                            model_transform * node_mat,
                            palette_offset,
                        ));
                    }
                }

                instance_ranges[submesh_batch_idx] =
                    instance_data_start_idx as u32..instance_data.len() as u32;
            }
        }
    }

    instance_ranges
}

fn resolve_static_pass(
    pass_batch: &PassBatches,
    render_resources: &RenderAssetStore,
    snaps: &FixedSnapshotGuard,
    t: f32,
    pose_storage: &mut AnimPoseStore,
    frame_idx: u32,
    instance_data: &mut Vec<StaticInstance>,
    node_world_cache: &mut Vec<PoseNodesCacheEntry>,
) -> Vec<Range<u32>> {
    let mut instance_ranges = vec![0..0; pass_batch.submesh_batches.len()];
    let mat_batches = &pass_batch.material_batches[pass_batch.static_batch.clone()];
    for mat_batch in mat_batches {
        let mesh_batches = &pass_batch.mesh_batches[mat_batch.mesh_range.clone()];
        for mesh_batch in mesh_batches {
            let model = render_resources
                .models
                .get(mesh_batch.model_id.into())
                .unwrap();
            for draw_idx in mesh_batch.submesh_range.clone() {
                let curr_draw = &pass_batch.submesh_batches[draw_idx];
                let inst_start = instance_data.len();
                for instance_idx in &curr_draw.instances {
                    let instance_idx = *instance_idx as usize;
                    let curr_node_inst =
                        &snaps.curr.mesh_draw_snapshot.static_instances[instance_idx];
                    let prev_node_inst = curr_node_inst.prev_index.map(|prev_idx| {
                        &snaps.prev.mesh_draw_snapshot.static_instances[prev_idx as usize]
                    });
                    if matches!(node_world_cache[instance_idx], PoseNodesCacheEntry::Pending) {
                        let maybe_nodes = if let Some(anim_snap) = curr_node_inst.animation {
                            let snap_time = prev_node_inst
                                .and_then(|n| n.animation.as_ref())
                                .map(|prev| safe_lerpu64(prev.0, anim_snap.0, t))
                                .unwrap_or(anim_snap.0);
                            sample_pose_nodes(
                                pose_storage,
                                &curr_node_inst.node_id,
                                snap_time,
                                frame_idx,
                            )
                        } else {
                            Some(model.rig.bind_matrices.clone())
                        };
                        node_world_cache[instance_idx] = match maybe_nodes {
                            Some(nodes) => PoseNodesCacheEntry::Ready(nodes),
                            None => PoseNodesCacheEntry::Missing,
                        };
                    }
                    let node_worlds = match &node_world_cache[instance_idx] {
                        PoseNodesCacheEntry::Ready(nodes) => nodes,
                        PoseNodesCacheEntry::Missing => continue,
                        PoseNodesCacheEntry::Pending => unreachable!(),
                    };

                    let model_transform = resolve_model_transform(
                        &curr_node_inst.model_transform,
                        prev_node_inst.map(|p| &p.model_transform),
                        curr_node_inst.dirty,
                        t,
                    );
                    let instance_nodes = &model.submeshes[curr_draw.submesh_idx].instance_nodes;
                    for instance_node_idx in instance_nodes {
                        let node_mat = node_worlds
                            .get(*instance_node_idx as usize)
                            .copied()
                            .unwrap_or(Mat4::IDENTITY);
                        instance_data.push(StaticInstance::new(model_transform * node_mat));
                    }
                }

                instance_ranges[draw_idx] = inst_start as u32..instance_data.len() as u32;
            }
        }
    }

    instance_ranges
}

pub fn resolve_skinned_draw<'a>(
    bones: &mut BonesBinding,
    bones_layout: &wgpu::BindGroupLayout,
    instances: &mut SkinnedInstances,
    render_resources: &RenderAssetStore,
    snaps: &'a FixedSnapshotGuard,
    t: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pose_storage: &mut AnimPoseStore,
    frame_idx: u32,
) -> DrawContext<'a> {
    let mut joint_palette: Vec<BoneMat34> = vec![];
    let mut instance_data = vec![];
    let skinned_instance_count = snaps.curr.mesh_draw_snapshot.skinned_instances.len();
    let mut node_world_cache = Vec::with_capacity(skinned_instance_count);
    node_world_cache.resize_with(skinned_instance_count, || PoseNodesCacheEntry::Pending);
    let mut node_to_palette_offset = vec![None; skinned_instance_count];
    let opaque_instance_ranges = resolve_skinned_pass(
        &snaps.curr.mesh_draw_snapshot.opaque_batch,
        render_resources,
        snaps,
        t,
        pose_storage,
        frame_idx,
        &mut instance_data,
        &mut node_world_cache,
        &mut node_to_palette_offset,
        &mut joint_palette,
    );
    let transparent_instance_ranges = resolve_skinned_pass(
        &snaps.curr.mesh_draw_snapshot.transparent_batch,
        render_resources,
        snaps,
        t,
        pose_storage,
        frame_idx,
        &mut instance_data,
        &mut node_world_cache,
        &mut node_to_palette_offset,
        &mut joint_palette,
    );

    bones.update(joint_palette, bones_layout, device, queue);
    instances.update(instance_data, queue, device);

    DrawContext {
        snap: &snaps.curr.mesh_draw_snapshot,
        opaque: PassDrawContext {
            batch: &snaps.curr.mesh_draw_snapshot.opaque_batch,
            instance_ranges: opaque_instance_ranges,
        },
        transparent: PassDrawContext {
            batch: &snaps.curr.mesh_draw_snapshot.transparent_batch,
            instance_ranges: transparent_instance_ranges,
        },
    }
}

pub fn resolve_static_draw<'a>(
    instances: &mut StaticInstances,
    render_resources: &RenderAssetStore,
    snaps: &'a FixedSnapshotGuard,
    t: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pose_storage: &mut AnimPoseStore,
    frame_idx: u32,
) -> DrawContext<'a> {
    let mut instance_data = vec![];
    let static_instance_count = snaps.curr.mesh_draw_snapshot.static_instances.len();
    let mut node_world_cache = Vec::with_capacity(static_instance_count);
    node_world_cache.resize_with(static_instance_count, || PoseNodesCacheEntry::Pending);
    let opaque_instance_ranges = resolve_static_pass(
        &snaps.curr.mesh_draw_snapshot.opaque_batch,
        render_resources,
        snaps,
        t,
        pose_storage,
        frame_idx,
        &mut instance_data,
        &mut node_world_cache,
    );
    let transparent_instance_ranges = resolve_static_pass(
        &snaps.curr.mesh_draw_snapshot.transparent_batch,
        render_resources,
        snaps,
        t,
        pose_storage,
        frame_idx,
        &mut instance_data,
        &mut node_world_cache,
    );

    instances.update(instance_data, queue, device);

    DrawContext {
        snap: &snaps.curr.mesh_draw_snapshot,
        opaque: PassDrawContext {
            batch: &snaps.curr.mesh_draw_snapshot.opaque_batch,
            instance_ranges: opaque_instance_ranges,
        },
        transparent: PassDrawContext {
            batch: &snaps.curr.mesh_draw_snapshot.transparent_batch,
            instance_ranges: transparent_instance_ranges,
        },
    }
}

use std::collections::HashMap;

use generational_arena::{Arena, Index};
use glam::{Quat, Vec3};

use crate::{job_system::worker_pool::{AnimPoseTaskResult, RenderResponse}, sim::scene_tree::SceneNodeId};

#[derive(Clone)]
pub struct TRS {
    pub t: Vec3,
    pub r: Quat,
    pub s: Vec3,
}

pub struct PoseData {
    pub joints: Vec<TRS>,
    pub time: u64,
}

struct PoseEntry {
    poses: Vec<PoseData>,
    last_seen: u32,
}
impl Default for PoseEntry {
    fn default() -> Self {
        Self {
            poses: vec![],
            last_seen: u32::MAX,
        }
    }
}

pub const POSE_STORAGE_BUFFER_SIZE: usize = 4;
const POSE_GC_GRACE_FRAMES: u32 = 60;

pub enum GetPoseResponse<'a> {
    One(&'a PoseData), // only one pose ready, use that
    Two(&'a PoseData, &'a PoseData), // 2+ poses ready, interpolate
    Nothing, // no poses ready yet, skip or use bind pose
}

pub struct PoseStorage {
    pub scene_to_pose_id: HashMap<SceneNodeId, Index>,
    pub pose_data: Arena<PoseEntry>,
}
impl PoseStorage {
    pub fn new() -> Self {
        Self {
            scene_to_pose_id: HashMap::new(),
            pose_data: Arena::new(),
        }
    }

    pub fn receive_pose(&mut self, res: AnimPoseTaskResult) {
        let idx = self.scene_to_pose_id.entry(res.node_id).or_insert(self.pose_data.insert(PoseEntry::default()));
        let entry = self.pose_data.get_mut(*idx).unwrap();

        // sorted insert
        if entry.poses.len() == 0 {
            entry.poses.push(res.data);
        } else {
            for i in 0..=entry.poses.len() {
                if i == entry.poses.len() {
                    entry.poses.insert(i, res.data);
                    break;
                }
                if entry.poses[i].time > res.data.time {
                    entry.poses.insert(i, res.data);
                    break;
                }
            }
        }

        if entry.poses.len() > POSE_STORAGE_BUFFER_SIZE {
            entry.poses.remove(0);
        }
    }

    pub fn run_gc(&mut self, frame_idx: u32) {
        self.pose_data.retain(|_idx, entry| {
            frame_idx.saturating_sub(entry.last_seen) < POSE_GC_GRACE_FRAMES
        });
        self.scene_to_pose_id.retain(|_node_id, pose_id| {
            self.pose_data.contains(*pose_id)
        });
    }

    pub fn get<'a>(&'a mut self, id: &SceneNodeId, pose_time: u64, frame_idx: u32) -> GetPoseResponse<'a> {
        if let Some(idx) = self.scene_to_pose_id.get(id) {
            let entry = self.pose_data.get_mut(*idx).unwrap();
            entry.last_seen = frame_idx;
            if entry.poses.len() == 0 {
                GetPoseResponse::Nothing
            } else if entry.poses.len() == 1 {
                GetPoseResponse::One(&entry.poses[0])
            } else {
                // linear search for the most recent animation time that is smaller than the query
                let mut best_idx = 0;
                for i in 0..entry.poses.len() {
                    if entry.poses[i].time < pose_time {
                        best_idx = i;
                    }
                }
                if best_idx == entry.poses.len() - 1 {
                    GetPoseResponse::One(&entry.poses[best_idx])
                } else {
                    GetPoseResponse::Two(&entry.poses[best_idx], &entry.poses[best_idx + 1])
                }
            }
        } else {
            GetPoseResponse::Nothing
        }
    }
}

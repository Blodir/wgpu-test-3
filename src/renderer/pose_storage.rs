use std::collections::HashMap;

use generational_arena::{Arena, Index};
use glam::{Quat, Vec3};

use crate::{job_system::worker_pool::{AnimPoseTaskResult, RenderResponse}, sim::scene_tree::SceneNodeId};

pub struct TRS {
    pub t: Vec3,
    pub r: Quat,
    pub s: Vec3,
}

pub enum PoseTime {
    SingleTime(f32),
    BlendTime(f32, f32),
}

pub struct PoseData {
    pub joints: Vec<TRS>,
    pub time: PoseTime,
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

const POSE_STORAGE_BUFFER_SIZE: usize = 3;
const POSE_GC_GRACE_FRAMES: u32 = 60;

pub enum GetPoseResponse<'a> {
    One(&'a PoseData), // only one pose ready, use that
    Two(&'a PoseData, &'a PoseData), // 2+ poses ready, interpolate
    Nothing, // no poses ready yet, skip or use bind pose
}

struct PoseStorage {
    pub scene_to_pose_id: HashMap<SceneNodeId, Index>,
    pub pose_data: Arena<PoseEntry>,
}
impl PoseStorage {
    pub fn receive_pose(&mut self, res: AnimPoseTaskResult) {
        let idx = self.scene_to_pose_id.entry(res.node_id).or_insert(self.pose_data.insert(PoseEntry::default()));
        let data = self.pose_data.get_mut(*idx).unwrap();

        // sorted insert
        if data.poses.len() == 0 {
            data.poses.insert(0, res.data);
        } else {
            for i in 0..=data.poses.len() {
                if i == data.poses.len() {
                    data.poses.insert(i, res.data);
                    break;
                }
                if res.data.time < data.poses[i].time {
                    data.poses.insert(i, res.data);
                    break;
                }
            }
        }

        if data.poses.len() > POSE_STORAGE_BUFFER_SIZE {
            data.poses.remove(0);
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

    pub fn get<'a>(&'a mut self, id: &SceneNodeId, pose_time: PoseTime, frame_idx: u32) -> GetPoseResponse<'a> {
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
                match pose_time {
                    PoseTime::SingleTime(curr_time) => {
                        for i in 0..(entry.poses.len() - 1) {
                            let comp_time = match entry.poses[i].time {
                                PoseTime::SingleTime(t) => t,
                                PoseTime::BlendTime(t0, t1) => t1,
                            };
                            if comp_time < curr_time {
                                best_idx = i;
                            }
                        }
                    },
                    PoseTime::BlendTime(curr_from, curr_to) => {
                        for i in 0..(entry.poses.len() - 1) {
                            let comp_time = match entry.poses[i].time {
                                PoseTime::SingleTime(t) => t,
                                PoseTime::BlendTime(t0, t1) => t0,
                            };
                            if comp_time < curr_from {
                                best_idx = i;
                            }
                        }
                    },
                }
                GetPoseResponse::Two(&entry.poses[best_idx], &entry.poses[best_idx + 1])
            }
        } else {
            GetPoseResponse::Nothing
        }
    }
}

use std::collections::HashMap;

use generational_arena::{Arena, Index};
use glam::{Quat, Vec3};

use crate::{job_system::worker_pool::{PoseResponse, RenderResponse}, sim::scene_tree::SceneNodeId};

pub struct JointData {
    t: Vec3,
    r: Quat,
    s: Vec3,
}

pub struct PoseData {
    joints: Vec<JointData>,
    time: f32,
}

struct PoseEntry {
    poses: Vec<PoseData>,
    last_seen: u32,
}
impl Default for PoseEntry {
    fn default() -> Self {
        Self {
            poses: vec![],
            last_seen: 0,
        }
    }
}

const POSE_STORAGE_BUFFER_SIZE: usize = 3;

struct PoseStorage {
    pub scene_to_pose_id: HashMap<SceneNodeId, Index>,
    pub pose_data: Arena<PoseEntry>,
}
impl PoseStorage {
    pub fn receive_pose(&mut self, res: PoseResponse) {
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
}

use std::collections::HashMap;

use generational_arena::{Arena, Index};
use glam::{Quat, Vec3};

use crate::{job_system::worker_pool::{AnimPoseTaskResult}, game::scene_tree::SceneNodeId};

#[derive(Clone, Copy, Debug)]
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
    buffer: PoseBuffer,
    last_seen: u32,
}
impl PoseEntry {
    fn new(first_pose: PoseData) -> Self {
        Self {
            buffer: PoseBuffer::new(first_pose),
            last_seen: u32::MAX,
        }
    }
}

pub const POSE_STORAGE_BUFFER_SIZE: usize = 6;
const POSE_GC_GRACE_FRAMES: u32 = 120;

pub enum GetPoseResponse<'a> {
    One(&'a [TRS]), // only one pose ready, use that
    Two(u64, &'a [TRS], u64, &'a [TRS]), // 2+ poses ready, interpolate
    Nothing, // no poses ready yet, skip or use bind pose
}

struct PoseBuffer {
    times: Vec<u64>,
    joints: Vec<TRS>,
    joints_count: usize,
}
impl PoseBuffer {
    fn new(first_pose: PoseData) -> Self {
        let joints_count = first_pose.joints.len();
        let times = Vec::with_capacity(POSE_STORAGE_BUFFER_SIZE);
        let joints = Vec::with_capacity(joints_count * POSE_STORAGE_BUFFER_SIZE);

        let mut this = Self {
            times,
            joints,
            joints_count,
        };
        this.insert_one(first_pose);

        this
    }

    fn evict_first_n(
        &mut self,
        n_poses: usize,
    ) {
        // evict joints
        let joints_n = n_poses * self.joints_count;
        let joints_len = self.joints.len();

        debug_assert!(joints_n <= joints_len);

        self.joints.copy_within(joints_n..joints_len, 0);
        self.joints.truncate(joints_len - joints_n);

        // evict times
        let times_len = self.times.len();

        debug_assert!(n_poses <= times_len);

        self.times.copy_within(n_poses..times_len, 0);
        self.times.truncate(times_len - n_poses);
    }

    fn insert_one(&mut self, data: PoseData) {
        if self.times.len() == POSE_STORAGE_BUFFER_SIZE {
            self.evict_first_n(1);
        }

        let times_len = self.times.len();

        // fast path
        if times_len == 0 || self.times[times_len - 1] <= data.time {
            self.times.push(data.time);
            self.joints.extend_from_slice(&data.joints);
            return;
        }

        // slow path: linear search and shift
        let mut insert_i = times_len;
        for i in 0..times_len {
            if data.time < self.times[i] {
                insert_i = i;
                break;
            }
        }

        self.times.push(0);
        self.times.copy_within(insert_i..times_len, insert_i + 1);
        self.times[insert_i] = data.time;

        let jc = self.joints_count;
        let old_joint_len = times_len * jc;

        unsafe {
            self.joints.set_len(old_joint_len + jc);
        }

        let src = insert_i * jc;
        let dst = (insert_i + 1) * jc;

        self.joints.copy_within(src..old_joint_len, dst);
        self.joints[src..src + jc].copy_from_slice(&data.joints);
    }

    fn get<'a>(&'a self, query_time: u64) -> GetPoseResponse<'a> {
        let len = self.times.len();

        if len == 0 {
            return GetPoseResponse::Nothing;
        }

        if len == 1 {
            return GetPoseResponse::One(&self.joints[0..self.joints_count]);
        }

        let mut best_idx = None;

        for i in 0..len {
            if self.times[i] <= query_time {
                best_idx = Some(i);
            } else {
                break;
            }
        }

        let idx = match best_idx {
            Some(i) => i,
            None => {
                return GetPoseResponse::One(
                    &self.joints[0..self.joints_count],
                );
            }
        };

        if idx == len - 1 {
            GetPoseResponse::One(&self.joints[idx * self.joints_count..(idx + 1) * self.joints_count])
        } else {
            GetPoseResponse::Two(
                self.times[idx],
                &self.joints[idx * self.joints_count..(idx + 1) * self.joints_count],
                self.times[idx + 1],
                &self.joints[(idx + 1) * self.joints_count..(idx + 2) * self.joints_count],
            )
        }
    }
}

pub struct PoseStorage {
    scene_to_pose_id: HashMap<SceneNodeId, Index>,
    pose_data: Arena<PoseEntry>,
}
impl PoseStorage {
    pub fn new() -> Self {
        Self {
            scene_to_pose_id: HashMap::new(),
            pose_data: Arena::new(),
        }
    }

    pub fn receive_poses(&mut self, res: AnimPoseTaskResult) {
        match self.scene_to_pose_id.get(&res.node_id) {
            Some(pose_idx) => {
                let entry = self.pose_data.get_mut(*pose_idx).unwrap();
                // TODO improve performance: insert multiple poses in one call (so we only shift once)
                for d in res.data {
                    entry.buffer.insert_one(d);
                }
            },
            None => {
                let mut data = res.data.into_iter();
                let mut entry = PoseEntry::new(data.next().unwrap());
                for d in data {
                    entry.buffer.insert_one(d);
                }
                let idx = self.pose_data.insert(entry);
                self.scene_to_pose_id.insert(res.node_id, idx);
            },
        }
    }

    // TODO this is never called
    pub fn run_gc(&mut self, frame_idx: u32) {
        self.pose_data.retain(|_idx, entry| {
            frame_idx.saturating_sub(entry.last_seen) < POSE_GC_GRACE_FRAMES
        });
        self.scene_to_pose_id.retain(|_node_id, pose_id| {
            self.pose_data.contains(*pose_id)
        });
    }

    pub fn get<'a>(&'a mut self, id: &SceneNodeId, query_time: u64, frame_idx: u32) -> GetPoseResponse<'a> {
        if let Some(idx) = self.scene_to_pose_id.get(id) {
            let entry = self.pose_data.get_mut(*idx).unwrap();
            entry.last_seen = frame_idx;
            entry.buffer.get(query_time)
        } else {
            GetPoseResponse::Nothing
        }
    }
}

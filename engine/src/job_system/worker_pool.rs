use std::sync::Arc;

use crossbeam::channel::{Receiver, Sender};

use crate::game::assets::runtime_formats::animation::AnimationClip;
use crate::{main::pose_storage::PoseData, resource_system::{file_formats::skeletonfile::Skeleton}, game::{animator::{BoundaryMode, TimeWrapMode}, scene_tree::SceneNodeId}};

use super::anim_pose::execute_pose_tasks;

pub struct SinglePoseTask {
    pub instance_time: u64,
    pub skeleton: Arc<Skeleton>,
    pub clip: Arc<AnimationClip>,
    pub time_wrap: TimeWrapMode,
    pub boundary_mode: BoundaryMode,
    /// time in seconds since the transition into this state started
    pub local_time: f32,
}

pub struct BlendPoseTask {
    pub instance_time: u64,
    pub skeleton: Arc<Skeleton>,
    pub from_clip: Arc<AnimationClip>,
    pub to_clip: Arc<AnimationClip>,
    pub blend_time: f32,
    /// time in seconds since the transition to the previous state started
    pub from_time: f32,
    /// time in seconds since this transition started
    pub to_time: f32,
    pub from_time_wrap: TimeWrapMode,
    pub to_time_wrap: TimeWrapMode,
}

pub enum AnimPoseTask {
    Single(SinglePoseTask),
    Blend(BlendPoseTask),
}

pub enum Task {
    Pose(SceneNodeId, Vec<AnimPoseTask>)
}

pub struct AnimPoseTaskResult {
    pub node_id: SceneNodeId,
    pub data: Vec<PoseData>,
}

pub enum RenderResponse {
    Pose(AnimPoseTaskResult),
}
pub enum GameResponse {}

fn worker_loop(
    rx: crossbeam::channel::Receiver<Task>,
    render_tx: &mut crossbeam::channel::Sender<RenderResponse>,
    game_tx: crossbeam::channel::Sender<GameResponse>,
) {
    while let Ok(task) = rx.recv() {
        match task {
            Task::Pose(node_id, tasks) => {
                execute_pose_tasks(node_id, tasks, render_tx);
            }
        }
    }
}

pub struct WorkerPool {
    workers: Vec<std::thread::JoinHandle<()>>,
}
impl WorkerPool {
    pub fn init() -> (Self, Sender<Task>, Receiver<RenderResponse>, Receiver<GameResponse>) {
        let (req_tx, req_rx) = crossbeam::channel::unbounded::<Task>();
        let (render_res_tx, render_res_rx) = crossbeam::channel::unbounded::<RenderResponse>();
        let (game_res_tx, game_res_rx) = crossbeam::channel::unbounded::<GameResponse>();

        let workers = (0..8)
            .map(|_| {
                let rx = req_rx.clone();
                let mut render_tx = render_res_tx.clone();
                let game_tx = game_res_tx.clone();
                std::thread::spawn(move || {
                    worker_loop(rx, &mut render_tx, game_tx);
                })
            })
            .collect();

        let this = Self {
            workers
        };

        (this, req_tx, render_res_rx, game_res_rx)
    }
}

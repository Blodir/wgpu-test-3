use std::sync::Arc;

use crossbeam::channel::{Receiver, Sender};

use crate::{renderer::pose_storage::PoseData, resource_system::{animation::AnimationClip, file_formats::skeletonfile::Skeleton}, sim::scene_tree::SceneNodeId};

struct PoseTask {
    skeleton: Arc<Skeleton>,
    clip: Arc<AnimationClip>,
    animation: Arc<Animation>,
    time: f32,
    pose_id: PoseId,
}

pub enum Task {
    Pose(Vec<PoseTask>)
}

pub struct PoseResponse {
    pub node_id: SceneNodeId,
    pub data: PoseData,
}

pub enum RenderResponse {
    Pose(PoseResponse),
}
pub enum GameResponse {}

fn worker_loop(
    rx: crossbeam::channel::Receiver<Task>,
    render_tx: crossbeam::channel::Sender<RenderResponse>,
    game_tx: crossbeam::channel::Sender<GameResponse>,
) {
    while let Ok(task) = rx.recv() {
        match task {
            Task::Pose(tasks) => {
                let _ = render_tx.send(todo!());
            }
        }
    }
}

struct WorkerPool {
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
                let render_tx = render_res_tx.clone();
                let game_tx = game_res_tx.clone();
                std::thread::spawn(move || {
                    worker_loop(rx, render_tx, game_tx);
                })
            })
            .collect();

        let this = Self {
            workers
        };

        (this, req_tx, render_res_rx, game_res_rx)
    }
}

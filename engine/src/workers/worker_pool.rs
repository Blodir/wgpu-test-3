use crossbeam::channel as cbch;

use crate::game::scene_tree::SceneNodeId;

use super::anim_pose::{execute_pose_tasks, AnimPoseTask, AnimPoseTaskResult};

pub enum Job {
    Pose(SceneNodeId, Vec<AnimPoseTask>),
}

pub enum RenderResponse {
    Pose(AnimPoseTaskResult),
}
pub enum GameResponse {}

fn worker_loop(
    rx: cbch::Receiver<Job>,
    render_tx: &mut cbch::Sender<RenderResponse>,
    _game_tx: cbch::Sender<GameResponse>,
) {
    while let Ok(task) = rx.recv() {
        match task {
            Job::Pose(node_id, tasks) => {
                execute_pose_tasks(node_id, tasks, render_tx);
            }
        }
    }
}

pub struct WorkerPool {
    pub workers: Vec<std::thread::JoinHandle<()>>,
}
impl WorkerPool {
    pub fn init() -> (
        Self,
        cbch::Sender<Job>,
        cbch::Receiver<RenderResponse>,
        cbch::Receiver<GameResponse>,
    ) {
        let (req_tx, req_rx) = cbch::unbounded::<Job>();
        let (render_res_tx, render_res_rx) = cbch::unbounded::<RenderResponse>();
        let (game_res_tx, game_res_rx) = cbch::unbounded::<GameResponse>();

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

        let this = Self { workers };

        (this, req_tx, render_res_rx, game_res_rx)
    }
}

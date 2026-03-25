use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crossbeam::channel as cbch;
use crossbeam_queue::SegQueue;

use super::assets::registry::{RegistryExt, ResourceRegistry, ResourceRequest, ResourceResult};
use super::assets::store::{CreateGameResourceRequest, CreateGameResourceResponse, GameAssetStore};
use super::scene_tree::RenderDataType;
use crate::{
    fixed_snapshot_handoff::FixedSnapshotHandoff,
    game::build_snapshot::FixedSnapshot,
    game_trait::{InputEvent, SimDebugInfo, SimTrait},
    job_system::worker_pool::Task,
    var_snapshot_handoff::{CameraSnapshotPair, VarSnapshotHandoff},
};

const TICK: Duration = Duration::from_millis(100);
const VAR_IDLE_SPIN: Duration = Duration::from_micros(200);
const MAX_ACCUMULATED_TICKS: u32 = 5;

pub fn spawn_sim<G, F>(
    inputs: Arc<SegQueue<InputEvent<G::UiCommand>>>,
    fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
    var_snapshot_handoff: Arc<VarSnapshotHandoff<G::VarSnapshot>>,
    reg_req_tx: cbch::Sender<ResourceRequest>,
    reg_res_rx: cbch::Receiver<ResourceResult>,
    game_req_rx: cbch::Receiver<CreateGameResourceRequest>,
    game_res_tx: cbch::Sender<CreateGameResourceResponse>,
    job_task_tx: cbch::Sender<Task>,
    make_game: F,
) -> std::thread::JoinHandle<()>
where
    G: SimTrait + 'static,
    F: FnOnce() -> G + Send + 'static,
    G::UiCommand: Send + 'static,
    G::VarSnapshot: Send + Sync + 'static,
{
    std::thread::spawn(move || {
        let mut game = make_game();
        let resource_registry =
            Rc::new(RefCell::new(ResourceRegistry::new(reg_req_tx, reg_res_rx)));
        let mut game_resources = GameAssetStore::new(game_req_rx, game_res_tx, &resource_registry);
        let (mut scene, animation_graphs) = game.init(&resource_registry);
        let fixed_dt = TICK.as_secs_f32();
        let max_accumulated = TICK.saturating_mul(MAX_ACCUMULATED_TICKS);
        let mut accumulated = Duration::ZERO;
        let mut prev_loop = Instant::now();
        let mut fixed_frame_index = 0u32;
        let mut var_tick = 0u64;
        let sim_start_time = Instant::now();
        let mut sim_fps_smoothed = 0.0f32;
        let mut prev_camera_snapshot = scene.camera.build_snapshot();
        let mut prev_camera_timestamp = Instant::now();
        loop {
            let now = Instant::now();
            let frame_dt = now.saturating_duration_since(prev_loop);
            prev_loop = now;
            accumulated = (accumulated + frame_dt).min(max_accumulated);
            let frame_dt_sec = frame_dt.as_secs_f32();
            if frame_dt_sec > 0.0 {
                let fps = 1.0 / frame_dt_sec;
                sim_fps_smoothed = if sim_fps_smoothed <= 0.0 {
                    fps
                } else {
                    (sim_fps_smoothed * 0.9) + (fps * 0.1)
                };
            }
            let sim_debug = SimDebugInfo {
                fps: sim_fps_smoothed,
                frame_time_ms: if sim_fps_smoothed > 0.0 {
                    1000.0 / sim_fps_smoothed
                } else {
                    0.0
                },
            };

            scene.global_time_sec = (now - sim_start_time).as_secs_f32();

            game_resources.process_requests(&resource_registry);
            resource_registry.process_responses();

            while let Some(event) = inputs.pop() {
                match event {
                    InputEvent::Exit => return (),
                    _ => game.consume_input(&mut scene, event),
                }
            }

            let curr_camera_snapshot = scene.camera.build_snapshot();
            let curr_camera_timestamp = Instant::now();
            let var_snapshot = game.build_var_snapshot(&scene, var_tick);
            var_snapshot_handoff.publish(
                var_tick,
                CameraSnapshotPair {
                    prev: prev_camera_snapshot,
                    prev_timestamp: prev_camera_timestamp,
                    curr: curr_camera_snapshot,
                    curr_timestamp: curr_camera_timestamp,
                },
                sim_debug,
                var_snapshot,
            );
            prev_camera_snapshot = curr_camera_snapshot;
            prev_camera_timestamp = curr_camera_timestamp;
            var_tick = var_tick.wrapping_add(1);

            while accumulated >= TICK {
                scene.update(
                    &resource_registry,
                    &animation_graphs,
                    scene.root,
                    fixed_dt,
                    &mut game,
                );

                let fixed_snapshot = FixedSnapshot::build(
                    &mut scene,
                    &resource_registry,
                    &animation_graphs,
                    &game_resources,
                    fixed_frame_index,
                );

                // schedule animation jobs for all visible animated models
                for node_id in fixed_snapshot.mesh_draw_snapshot.skinned_instances.keys() {
                    match &mut scene.nodes.get_mut((*node_id).into()).unwrap().render_data {
                        RenderDataType::Model(_static_model) => (),
                        RenderDataType::AnimatedModel(animated_model) => {
                            let job = animated_model.animator.build_job(
                                fixed_dt,
                                &animation_graphs,
                                &animated_model.model,
                                &game_resources,
                                &resource_registry,
                            );
                            if job_task_tx.send(Task::Pose(*node_id, job)).is_err() {
                                todo!();
                            }
                        }
                        RenderDataType::None => (),
                    }
                }
                for node_id in fixed_snapshot.mesh_draw_snapshot.static_instances.keys() {
                    match &mut scene.nodes.get_mut((*node_id).into()).unwrap().render_data {
                        RenderDataType::Model(_static_model) => (),
                        RenderDataType::AnimatedModel(animated_model) => {
                            let job = animated_model.animator.build_job(
                                fixed_dt,
                                &animation_graphs,
                                &animated_model.model,
                                &game_resources,
                                &resource_registry,
                            );
                            if job_task_tx.send(Task::Pose(*node_id, job)).is_err() {
                                todo!();
                            }
                        }
                        RenderDataType::None => (),
                    }
                }

                fixed_snapshot_handoff.publish(fixed_snapshot);
                fixed_frame_index = fixed_frame_index.wrapping_add(1);
                accumulated -= TICK;
            }

            let idle_start = Instant::now();
            while idle_start.elapsed() < VAR_IDLE_SPIN {
                if !inputs.is_empty() {
                    break;
                }
                std::hint::spin_loop();
            }
            if inputs.is_empty() {
                thread::yield_now();
            }
        }
    })
}

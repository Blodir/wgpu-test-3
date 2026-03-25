use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crossbeam_queue::SegQueue;

use super::assets::registry::{RegistryExt, ResourceRegistry, ResourceRequest, ResourceResult};
use super::assets::store::{CreateGameResourceRequest, CreateGameResourceResponse, GameAssetStore};
use super::scene_tree::RenderDataType;
use crate::{
    fixed_snapshot_handoff::FixedSnapshotHandoff,
    game::build_snapshot::FixedSnapshot,
    game_trait::{InputEvent, SimTrait},
    job_system::worker_pool::Task,
    var_snapshot_handoff::{CameraSnapshotPair, VarSnapshotHandoff},
};

const TICK: Duration = Duration::from_millis(100);
const SPIN: Duration = Duration::from_micros(200);

pub fn spawn_sim<G>(
    inputs: Arc<SegQueue<InputEvent<G::UiCommand>>>,
    fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
    var_snapshot_handoff: Arc<VarSnapshotHandoff<G::VarSnapshot>>,
    reg_req_tx: crossbeam::channel::Sender<ResourceRequest>,
    reg_res_rx: crossbeam::channel::Receiver<ResourceResult>,
    game_req_rx: crossbeam::channel::Receiver<CreateGameResourceRequest>,
    game_res_tx: crossbeam::channel::Sender<CreateGameResourceResponse>,
    job_task_tx: crossbeam::channel::Sender<Task>,
    game: G,
) -> std::thread::JoinHandle<()>
where
    G: SimTrait + Send + 'static,
    G::UiCommand: Send + 'static,
    G::VarSnapshot: Send + Sync + 'static,
{
    std::thread::spawn(move || {
        let mut game = game;
        let resource_registry =
            Rc::new(RefCell::new(ResourceRegistry::new(reg_req_tx, reg_res_rx)));
        let mut game_resources = GameAssetStore::new(game_req_rx, game_res_tx, &resource_registry);
        let (mut scene, animation_graphs) = game.init(&resource_registry);
        let mut next = Instant::now() + TICK;
        let mut prev_tick = Instant::now();
        let mut frame_index = 0u32;
        let sim_start_time = Instant::now();
        let mut prev_camera_snapshot = scene.camera.build_snapshot();
        let mut prev_camera_timestamp = Instant::now();
        loop {
            let now = Instant::now();
            let dt = (now - prev_tick).as_secs_f32();
            prev_tick = now;

            scene.global_time_sec = (now - sim_start_time).as_secs_f32();

            game_resources.process_requests(&resource_registry);
            resource_registry.process_responses();

            while let Some(event) = inputs.pop() {
                match event {
                    InputEvent::Exit => return (),
                    _ => game.consume_input(&mut scene, event),
                }
            }

            scene.update(
                &resource_registry,
                &animation_graphs,
                scene.root,
                dt,
                &mut game,
            );
            let curr_camera_snapshot = scene.camera.build_snapshot();
            let curr_camera_timestamp = Instant::now();
            let var_snapshot = game.build_var_snapshot(&scene, frame_index as u64);
            var_snapshot_handoff.publish(
                frame_index as u64,
                CameraSnapshotPair {
                    prev: prev_camera_snapshot,
                    prev_timestamp: prev_camera_timestamp,
                    curr: curr_camera_snapshot,
                    curr_timestamp: curr_camera_timestamp,
                },
                var_snapshot,
            );
            prev_camera_snapshot = curr_camera_snapshot;
            prev_camera_timestamp = curr_camera_timestamp;

            let fixed_snapshot = FixedSnapshot::build(
                &mut scene,
                &resource_registry,
                &animation_graphs,
                &game_resources,
                frame_index,
            );

            // schedule animation jobs for all visible animated models
            for node_id in fixed_snapshot.mesh_draw_snapshot.skinned_instances.keys() {
                match &mut scene.nodes.get_mut((*node_id).into()).unwrap().render_data {
                    RenderDataType::Model(_static_model) => (),
                    RenderDataType::AnimatedModel(animated_model) => {
                        let job = animated_model.animator.build_job(
                            dt,
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
                            dt,
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

            next += TICK;

            // sleep most of the remaining time, then spin the last bit
            if let Some(remain) = next.checked_duration_since(Instant::now()) {
                if remain > SPIN {
                    thread::sleep(remain - SPIN);
                }
                while Instant::now() < next {
                    std::hint::spin_loop();
                }
            } else {
                // if we fell behind, resync the schedule
                next = Instant::now() + TICK;
            }

            frame_index = frame_index.wrapping_add(1);
        }
    })
}

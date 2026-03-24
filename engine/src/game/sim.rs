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
    game::build_snapshot::RenderSnapshot,
    game_trait::{InputEvent, SimTrait},
    job_system::worker_pool::Task,
    render_snapshot_handoff::RenderSnapshotHandoff,
    ui_snapshot_handoff::UiSnapshotHandoff,
};

const TICK: Duration = Duration::from_millis(100);
const SPIN: Duration = Duration::from_micros(200);

pub fn spawn_sim<G>(
    inputs: Arc<SegQueue<InputEvent<G::UiCommand>>>,
    snap_handoff: Arc<RenderSnapshotHandoff>,
    ui_snapshot_handoff: Arc<UiSnapshotHandoff<G::UiSnapshot>>,
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
    G::UiSnapshot: Send + Sync + 'static,
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
            let ui_snapshot = game.build_ui_snapshot(&scene, frame_index as u64);
            ui_snapshot_handoff.publish(frame_index as u64, ui_snapshot);

            let snap = RenderSnapshot::build(
                &mut scene,
                &resource_registry,
                &animation_graphs,
                &game_resources,
                frame_index,
            );

            // schedule animation jobs for all visible animated models
            for node_id in snap.mesh_draw_snapshot.skinned_instances.keys() {
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
            for node_id in snap.mesh_draw_snapshot.static_instances.keys() {
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

            snap_handoff.publish(snap);

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

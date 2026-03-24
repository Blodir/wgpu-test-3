use crossbeam_queue::SegQueue;
use game::sim::spawn_sim;
use job_system::worker_pool::WorkerPool;
use main::assets::manager::RenderAssetManager;
use std::sync::Arc;
use winit::event_loop::{ControlFlow, EventLoop};

use crate::{
    game::build_snapshot::RenderSnapshot,
    game_trait::{InputEvent, SimTrait, UiTrait},
    main::window,
    render_snapshot_handoff::RenderSnapshotHandoff,
    ui_snapshot_handoff::UiSnapshotHandoff,
};

pub mod game;
pub mod game_trait;
pub mod job_system;
pub mod main;
pub mod render_snapshot_handoff;
pub mod ui_snapshot_handoff;

pub fn run<G>(game: G)
where
    G: SimTrait
        + UiTrait<UiSnapshot = <G as SimTrait>::UiSnapshot, UiCommand = <G as SimTrait>::UiCommand>
        + Send
        + 'static,
    <G as SimTrait>::UiSnapshot: Send + Sync + 'static,
    <G as SimTrait>::UiCommand: Send + 'static,
{
    let (game_req_tx, game_req_rx) = crossbeam::channel::unbounded();
    let (game_res_tx, game_res_rx) = crossbeam::channel::unbounded();
    let (registry_req_tx, registry_req_rx) = crossbeam::channel::unbounded();
    let (registry_res_tx, registry_res_rx) = crossbeam::channel::unbounded();
    let resource_manager =
        RenderAssetManager::new(registry_req_rx, registry_res_tx, game_res_rx, game_req_tx);
    let initial_snap = RenderSnapshot::init();
    let snap_handoff = Arc::new(RenderSnapshotHandoff::new(initial_snap));
    let ui_snapshot_handoff = Arc::new(UiSnapshotHandoff::<<G as SimTrait>::UiSnapshot>::new());
    let sim_inputs = Arc::new(SegQueue::<InputEvent<<G as SimTrait>::UiCommand>>::new());

    let (worker_pool, task_tx, render_rx, game_rx) = WorkerPool::init();
    let sim_handle = spawn_sim(
        sim_inputs.clone(),
        snap_handoff.clone(),
        ui_snapshot_handoff.clone(),
        registry_req_tx,
        registry_res_rx,
        game_req_rx,
        game_res_tx,
        task_tx,
        game,
    );

    let mut main_window = window::MainWindow::new(
        sim_inputs.clone(),
        snap_handoff.clone(),
        resource_manager,
        render_rx,
        ui_snapshot_handoff,
        <G as UiTrait>::build_ui,
    );
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let _ = event_loop.run_app(&mut main_window);

    sim_inputs.push(InputEvent::Exit);
    sim_handle.join().unwrap();
}

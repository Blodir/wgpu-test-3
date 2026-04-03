use crossbeam::channel as cbch;
use crossbeam_queue::SegQueue;
use game::sim::spawn_sim;
use workers::worker_pool::WorkerPool;
use main::assets::manager::MainAssetManager;
use std::sync::Arc;
use winit::event_loop::{ControlFlow, EventLoop};

use crate::{
    fixed_snapshot_handoff::FixedSnapshotHandoff,
    game::build_snapshot::FixedSnapshot,
    game_trait::{InputEvent, SimTrait, UiTrait},
    main::window,
    var_snapshot_handoff::VarSnapshotHandoff,
};

pub mod fixed_snapshot_handoff;
pub mod game;
pub mod game_trait;
pub mod workers;
pub mod main;
pub mod var_snapshot_handoff;

pub fn run<G, F>(make_game: F)
where
    G: SimTrait
        // UiTraits associated VarSnapshot and UiCommand types have to match with SimTrait
        + UiTrait<VarSnapshot = <G as SimTrait>::VarSnapshot, UiCommand = <G as SimTrait>::UiCommand>
        + 'static,
    F: FnOnce() -> G + Send + 'static,
    <G as SimTrait>::VarSnapshot: Send + Sync + 'static,
    <G as SimTrait>::UiCommand: Send + 'static,
{
    let (game_req_tx, game_req_rx) = cbch::unbounded();
    let (game_res_tx, game_res_rx) = cbch::unbounded();
    let (registry_req_tx, registry_req_rx) = cbch::unbounded();
    let (registry_res_tx, registry_res_rx) = cbch::unbounded();
    let asset_manager =
        MainAssetManager::new(registry_req_rx, registry_res_tx, game_res_rx, game_req_tx);
    let initial_snap = FixedSnapshot::init();
    let fixed_snapshot_handoff = Arc::new(FixedSnapshotHandoff::new(initial_snap));
    let var_snapshot_handoff = Arc::new(VarSnapshotHandoff::<<G as SimTrait>::VarSnapshot>::new());
    let sim_inputs = Arc::new(SegQueue::<InputEvent<<G as SimTrait>::UiCommand>>::new());

    let (worker_pool, task_tx, render_rx, game_rx) = WorkerPool::init();
    let sim_handle = spawn_sim(
        sim_inputs.clone(),
        fixed_snapshot_handoff.clone(),
        var_snapshot_handoff.clone(),
        registry_req_tx,
        registry_res_rx,
        game_req_rx,
        game_res_tx,
        task_tx,
        make_game,
    );

    let mut main_window = window::MainWindow::new(
        sim_inputs.clone(),
        fixed_snapshot_handoff.clone(),
        asset_manager,
        render_rx,
        var_snapshot_handoff,
        <G as UiTrait>::build_ui,
    );
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let _ = event_loop.run_app(&mut main_window);

    sim_inputs.push(InputEvent::Exit);
    sim_handle.join().unwrap();
    for worker in worker_pool.workers {
        worker.join().unwrap();
    }
}

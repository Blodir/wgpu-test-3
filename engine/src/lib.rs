use crossbeam::channel as cbch;
use crossbeam_queue::SegQueue;
use game::sim::spawn_sim;
use std::sync::Arc;
use winit::event_loop::{ControlFlow, EventLoop};
use workers::worker_pool::WorkerPool;

use crate::{
    api::{GameTrait, UiTrait},
    fixed_snapshot::{FixedSnapshot, FixedSnapshotHandoff},
    game::sim::InputEvent,
    host::window,
    var_snapshot::VarSnapshotHandoff,
};

pub mod api;
pub mod fixed_snapshot;
pub mod game;
pub mod global_paths;
pub mod host;
pub mod var_snapshot;
pub mod workers;

pub fn run<G, F>(make_game: F)
where
    G: GameTrait
        // UiTraits associated VarSnapshot and UiCommand types have to match with SimTrait
        + UiTrait<
            VarSnapshot = <G as GameTrait>::VarSnapshot,
            UiCommand = <G as GameTrait>::UiCommand,
        > + 'static,
    F: FnOnce() -> G + Send + 'static,
    <G as GameTrait>::VarSnapshot: Send + Sync + Default + 'static,
    <G as GameTrait>::UiCommand: Send + 'static,
{
    let (game_req_tx, game_req_rx) = cbch::unbounded();
    let (game_res_tx, game_res_rx) = cbch::unbounded();
    let (registry_req_tx, registry_req_rx) = cbch::unbounded();
    let (registry_res_tx, registry_res_rx) = cbch::unbounded();
    let initial_snap = FixedSnapshot::default();
    let fixed_snapshot_handoff = Arc::new(FixedSnapshotHandoff::new(initial_snap));
    let var_snapshot_handoff = Arc::new(VarSnapshotHandoff::<<G as GameTrait>::VarSnapshot>::new());
    let sim_inputs = Arc::new(SegQueue::<InputEvent<<G as GameTrait>::UiCommand>>::new());

    let (worker_pool, worker_tx, worker_render_rx, worker_game_rx) = WorkerPool::init();
    let sim_handle = spawn_sim(
        sim_inputs.clone(),
        fixed_snapshot_handoff.clone(),
        var_snapshot_handoff.clone(),
        registry_req_tx,
        registry_res_rx,
        game_req_rx,
        game_res_tx,
        worker_tx,
        make_game,
    );

    let mut main_window = window::MainWindow::new(
        sim_inputs.clone(),
        fixed_snapshot_handoff.clone(),
        registry_req_rx,
        registry_res_tx,
        game_res_rx,
        game_req_tx,
        worker_render_rx,
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

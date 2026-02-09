use crossbeam_queue::SegQueue;
use job_system::worker_pool::{WorkerPool};
use main::assets::manager::RenderAssetManager;
use game::sim::{spawn_sim, InputEvent};
use std::sync::{Arc, Mutex};
use winit::{
    application::ApplicationHandler,
    event::Event,
    event_loop::{ControlFlow, EventLoop},
};

use crate::{game::build_snapshot::RenderSnapshot, main::window, snapshot_handoff::SnapshotHandoff};

pub mod snapshot_handoff;
pub mod main;
pub mod game;
pub mod job_system;

pub fn run() {
    let (game_req_tx, game_req_rx) = crossbeam::channel::unbounded();
    let (game_res_tx, game_res_rx) = crossbeam::channel::unbounded();
    let (registry_req_tx, registry_req_rx) = crossbeam::channel::unbounded();
    let (registry_res_tx, registry_res_rx) = crossbeam::channel::unbounded();
    let resource_manager = RenderAssetManager::new(registry_req_rx, registry_res_tx, game_res_rx, game_req_tx);
    let initial_snap = RenderSnapshot::init();
    let snap_handoff = Arc::new(SnapshotHandoff::new(initial_snap));
    let sim_inputs = Arc::new(SegQueue::<InputEvent>::new());

    let (worker_pool, task_tx, render_rx, game_rx) = WorkerPool::init();
    let sim_handle = spawn_sim(sim_inputs.clone(), snap_handoff.clone(), registry_req_tx, registry_res_rx, game_req_rx, game_res_tx, task_tx);

    let main_window = Arc::new(Mutex::new(window::MainWindow::new(
        sim_inputs.clone(),
        snap_handoff.clone(),
        resource_manager,
        render_rx,
    )));
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let main_window_clone = Arc::clone(&main_window);
    event_loop
        .run(move |event, event_loop| {
            let mut main_window = main_window_clone.lock().unwrap();
            match event {
                Event::NewEvents(cause) => main_window.new_events(event_loop, cause),
                Event::WindowEvent { window_id, event } => {
                    main_window.window_event(event_loop, window_id, event)
                }
                Event::DeviceEvent { device_id, event } => {
                    main_window.device_event(event_loop, device_id, event)
                }
                Event::UserEvent(event) => main_window.user_event(event_loop, event),
                Event::Suspended => main_window.suspended(event_loop),
                Event::Resumed => main_window.resumed(event_loop),
                Event::AboutToWait => main_window.about_to_wait(event_loop),
                Event::LoopExiting => main_window.exiting(event_loop),
                Event::MemoryWarning => main_window.memory_warning(event_loop),
            }
        })
        .unwrap();

    sim_inputs.push(InputEvent::Exit);
    sim_handle.join().unwrap();
}

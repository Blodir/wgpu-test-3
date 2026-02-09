use crossbeam_queue::SegQueue;
use job_system::worker_pool::{self, WorkerPool};
use render_snapshot::{RenderSnapshot, SnapshotHandoff};
use main::assets::manager::RenderAssetManager;
use game::sim::{spawn_sim, InputEvent};
use std::sync::{Arc, Mutex};
use winit::{
    application::ApplicationHandler,
    event::Event,
    event_loop::{ControlFlow, EventLoop},
};

pub mod render_snapshot;
pub mod main;
pub mod game;
pub mod app;
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

    let app = Arc::new(Mutex::new(app::App::new(
        sim_inputs.clone(),
        snap_handoff.clone(),
        resource_manager,
        render_rx,
    )));
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    /*
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
    watcher
        .watch(
            Path::new("src/renderer/pipelines/shaders/"),
            notify::RecursiveMode::Recursive,
        )
        .unwrap();

    let app_clone1 = app.clone();
    thread::spawn(move || loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => match event {
                Ok(e) => match e.kind {
                    notify::EventKind::Modify(notify::event::ModifyKind::Any) => {
                        let mut should_reload = true;
                        for path in &e.paths {
                            if path.to_string_lossy().ends_with('~') {
                                should_reload = false;
                                break;
                            }
                        }
                        if should_reload {
                            let mut app = app_clone1.lock().unwrap();
                            app.reload_shaders();
                        }
                    }
                    _ => {}
                },
                Err(e) => println!("watch error: {:?}", e),
            },
            Err(e) => {}
        }
    });
    */

    let app_clone2 = Arc::clone(&app);
    event_loop
        .run(move |event, event_loop| {
            let mut app = app_clone2.lock().unwrap();
            match event {
                Event::NewEvents(cause) => app.new_events(event_loop, cause),
                Event::WindowEvent { window_id, event } => {
                    app.window_event(event_loop, window_id, event)
                }
                Event::DeviceEvent { device_id, event } => {
                    app.device_event(event_loop, device_id, event)
                }
                Event::UserEvent(event) => app.user_event(event_loop, event),
                Event::Suspended => app.suspended(event_loop),
                Event::Resumed => app.resumed(event_loop),
                Event::AboutToWait => app.about_to_wait(event_loop),
                Event::LoopExiting => app.exiting(event_loop),
                Event::MemoryWarning => app.memory_warning(event_loop),
            }
        })
        .unwrap();

    sim_inputs.push(InputEvent::Exit);
    sim_handle.join().unwrap();
}

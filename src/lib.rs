use crossbeam_queue::{ArrayQueue, SegQueue};
use notify::{Config, RecommendedWatcher, Watcher};
use pollster::FutureExt as _;
use renderer::render_snapshot::SnapshotHandoff;
use sim::{spawn_sim, InputEvent};
use std::{
    path::Path,
    sync::{mpsc::channel, Arc, Mutex},
    thread,
    time::Duration,
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, ElementState, Event, KeyEvent, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{
    renderer::{
        render_resources::{EnvironmentMapHandle, ModelHandle, RenderResources},
        wgpu_context::WgpuContext,
        Renderer,
    },
    scene_tree::{Node, RenderDataType, Scene},
};

pub mod renderer;
pub mod scene_tree;
pub mod sim;

pub fn align_to_256(n: usize) -> usize {
    (n + 255) & !255
}

pub fn strip_extension(path: &str) -> String {
    let path = Path::new(path);
    let stem = path.file_stem().and_then(|s| s.to_str());

    match (path.parent(), stem) {
        (Some(parent), Some(stem)) => parent.join(stem).to_string_lossy().into_owned(),
        (None, Some(stem)) => stem.to_string(),
        _ => path.to_string_lossy().into_owned(),
    }
}

struct App<'surface> {
    renderer: Option<Arc<Mutex<Renderer>>>,
    window: Option<Arc<Window>>,
    wgpu_context: Option<WgpuContext<'surface>>,
    render_resources: Option<RenderResources>,
    snap_handoff: Arc<SnapshotHandoff>,
    sim_inputs: Arc<SegQueue<InputEvent>>,
}

impl App<'_> {
    pub fn new(sim_inputs: Arc<SegQueue<InputEvent>>, snap_handoff: Arc<SnapshotHandoff>) -> Self {
        Self {
            renderer: None,
            window: None,
            wgpu_context: None,
            render_resources: None,
            snap_handoff,
            sim_inputs,
        }
    }

    pub fn reload_shaders(&mut self) {
        /*
        if let Some(ref mut renderer_arc_mutex) = self.renderer {
            let mut renderer = renderer_arc_mutex.lock().unwrap();
            match renderer.reload_pbr_pipeline() {
                Ok(_) => {}
                Err(e) => eprintln!("render error: {:?}", e),
            }
        }
        */
    }
}

fn resize(
    physical_size: PhysicalSize<u32>,
    wgpu_context: &mut WgpuContext,
    renderer: &mut Renderer,
) {
    if physical_size.width > 0 && physical_size.height > 0 {
        wgpu_context.surface_config.width = physical_size.width;
        wgpu_context.surface_config.height = physical_size.height;
        wgpu_context
            .surface
            .configure(&wgpu_context.device, &wgpu_context.surface_config);
        renderer.resize(wgpu_context);
    }
}

impl<'surface> ApplicationHandler for App<'surface> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        self.window = Some(window.clone());
        let wgpu_context = WgpuContext::new(window).block_on();
        let mut render_resources = RenderResources::new(&wgpu_context);
        // TODO proper render resources loading!!!
        render_resources.load_environment_map(
            EnvironmentMapHandle("assets/kloofendal_overcast_puresky_8k".to_string()),
            &wgpu_context,
        );
        render_resources
            .load_model(
                ModelHandle("assets/local/Sponza/Sponza.json".to_string()),
                &wgpu_context,
            )
            .unwrap();
        /*
        render_resources
            .load_scene(&self.scene, &wgpu_context)
            .unwrap();
        */
        let temp_renderer =
            Renderer::new(&wgpu_context, &render_resources, self.snap_handoff.clone());
        let renderer_arc_mutex = Arc::new(Mutex::new(temp_renderer));
        self.renderer = Some(renderer_arc_mutex);
        self.render_resources = Some(render_resources);
        self.wgpu_context = Some(wgpu_context);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let renderer = renderer_arc_mutex.lock().unwrap();
                    match renderer.render(
                        self.render_resources.as_mut().unwrap(),
                        self.wgpu_context.as_ref().unwrap(),
                    ) {
                        Ok(_) => {}
                        Err(e) => eprintln!("render error: {:?}", e),
                    }
                }
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let wgpu_context = self.wgpu_context.as_mut().unwrap();
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    resize(physical_size, wgpu_context, &mut renderer);
                    self.render_resources.as_mut().unwrap().camera.update(
                        // TODO resize events should probably respect render loop interpolation
                        &self.snap_handoff.load().curr.camera,
                        &wgpu_context.queue,
                        &wgpu_context.surface_config,
                    );
                }
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer,
            } => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let wgpu_context = self.wgpu_context.as_mut().unwrap();
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    let new_size = wgpu_context.window.inner_size();
                    resize(new_size, wgpu_context, &mut renderer);
                    self.render_resources.as_mut().unwrap().camera.update(
                        &self.snap_handoff.load().curr.camera,
                        &wgpu_context.queue,
                        &wgpu_context.surface_config,
                    );
                }
            }
            WindowEvent::MouseWheel { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::KeyboardInput { .. } => {
                self.sim_inputs.push(InputEvent::WindowEvent(event))
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.window.as_mut().unwrap().request_redraw();
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { .. } => self.sim_inputs.push(InputEvent::DeviceEvent(event)),
            _ => (),
        }
    }
}

pub fn run() {
    let snap_handoff = Arc::new(SnapshotHandoff::new());
    let sim_inputs = Arc::new(SegQueue::<InputEvent>::new());
    let sim_handle = spawn_sim(sim_inputs.clone(), snap_handoff.clone());

    let app = Arc::new(Mutex::new(App::new(
        sim_inputs.clone(),
        snap_handoff.clone(),
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

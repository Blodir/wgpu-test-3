use cgmath::{InnerSpace, Rotation3};
use notify::{Config, RecommendedWatcher, Watcher};
use pollster::FutureExt as _;
use std::{
    path::Path,
    sync::{mpsc::channel, Arc, Mutex},
    thread,
    time::Duration,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceEvent, ElementState, Event, KeyEvent, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

pub mod dds;
pub mod renderer;
pub mod scene_graph;

use renderer::{gltf::GLTF, renderer::Renderer};

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
    renderer: Option<Arc<Mutex<Renderer<'surface>>>>,
    window: Option<Arc<Window>>,
    scene: Arc<GLTF>,
    mouse_btn_is_pressed: bool,
    shift_is_pressed: bool,
}

impl App<'_> {
    pub fn new(gltf: GLTF) -> Self {
        Self {
            renderer: None,
            window: None,
            scene: Arc::new(gltf),
            mouse_btn_is_pressed: false,
            shift_is_pressed: false,
        }
    }

    pub fn reload_shaders(&mut self) {
        if let Some(ref mut renderer_arc_mutex) = self.renderer {
            let mut renderer = renderer_arc_mutex.lock().unwrap();
            match renderer.reload_pbr_pipeline() {
                Ok(_) => {}
                Err(e) => eprintln!("render error: {:?}", e),
            }
        }
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

        let meshes = self.scene.to_pbr_meshes();
        let temp_renderer = Renderer::new(window.clone(), meshes).block_on();
        let renderer_arc_mutex = Arc::new(Mutex::new(temp_renderer));
        self.renderer = Some(renderer_arc_mutex.clone());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    match renderer.render() {
                        Ok(_) => {}
                        Err(e) => eprintln!("render error: {:?}", e),
                    }
                }
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    let camera = renderer.get_camera_mut();
                    match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            camera.eye.z = (camera.eye.z
                                + ((if self.shift_is_pressed { 10f32 } else { 1f32 }) * -y as f32))
                                .max(0f32);
                            renderer.update_camera();
                            self.window.as_mut().unwrap().request_redraw();
                        }
                        MouseScrollDelta::PixelDelta(pos) => (),
                    }
                }
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                match button {
                    winit::event::MouseButton::Left => match state {
                        ElementState::Pressed => {
                            self.mouse_btn_is_pressed = true;
                        }
                        ElementState::Released => {
                            self.mouse_btn_is_pressed = false;
                        }
                    },
                    _ => (),
                };
            }
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => match event {
                KeyEvent {
                    physical_key: PhysicalKey::Code(KeyCode::ShiftLeft),
                    state: ElementState::Pressed,
                    ..
                } => {
                    self.shift_is_pressed = true;
                }
                KeyEvent {
                    physical_key: PhysicalKey::Code(KeyCode::ShiftLeft),
                    state: ElementState::Released,
                    ..
                } => {
                    self.shift_is_pressed = false;
                }
                _ => (),
            },
            WindowEvent::Resized(physical_size) => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    renderer.resize(Some(physical_size));
                    self.window.as_mut().unwrap().request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer,
            } => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    renderer.resize(None);
                    self.window.as_mut().unwrap().request_redraw();
                }
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { delta: (x, y) } => {
                if !self.mouse_btn_is_pressed {
                    return ();
                }
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    let camera = renderer.get_camera_mut();
                    let sensitivity = 5f32;
                    camera.rot_x = camera.rot_x - cgmath::Deg(x as f32 / sensitivity);
                    camera.rot_y = camera.rot_y - cgmath::Deg(y as f32 / sensitivity);
                    renderer.update_camera();
                    self.window.as_mut().unwrap().request_redraw();
                }
            }
            _ => (),
        }
    }
}

pub fn run(gltf: GLTF) {
    let app = Arc::new(Mutex::new(App::new(gltf)));
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
    watcher
        .watch(
            Path::new("src/renderer/shaders/"),
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
}

use std::{sync::{Arc, Mutex, mpsc::channel}, path::Path, time::Duration, thread};
use winit::{window::{Window, WindowId}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, event::{WindowEvent, KeyEvent, ElementState}, application::ApplicationHandler, keyboard::{KeyCode, PhysicalKey}};
use notify::{Watcher, RecommendedWatcher, Config};
use crate::{glb::GLBObject, renderer};
use pollster::FutureExt as _;

struct ShaderWatcher {
    watcher: RecommendedWatcher,
    pub renderer_wrapper: Arc<Mutex<Option<Arc<Mutex<renderer::Renderer>>>>>,
}
impl ShaderWatcher {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
        watcher.watch(Path::new("src/shaders/"), notify::RecursiveMode::Recursive).unwrap();

        let renderer_wrapper_arc_mutex: Arc<Mutex<Option<Arc<Mutex<renderer::Renderer>>>>> = Arc::new(Mutex::new(None));
        let renderer_wrapper_clone = renderer_wrapper_arc_mutex.clone();
        thread::spawn(move || {
            loop {
                match rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(event) => {
                        match event {
                            Ok(e) => {
                                match e.kind {
                                    notify::EventKind::Modify(notify::event::ModifyKind::Any) => {
                                        let mut should_reload = true;
                                        for path in &e.paths {
                                            if path.to_string_lossy().ends_with('~') {
                                                should_reload = false;
                                                break;
                                            }
                                        }
                                        if should_reload {
                                            let mut renderer_wrapper = renderer_wrapper_clone.lock().unwrap();
                                            if let Some(ref mut renderer_arc_mutex) = *renderer_wrapper {
                                                let mut renderer = renderer_arc_mutex.lock().unwrap();
                                                renderer.reload_shaders();
                                            }
                                        }
                                    },
                                    _ => {}
                                }
                            },
                            Err(e) => println!("watch error: {:?}", e),
                        }
                    }
                    Err(e) => {},
                }
            }
        });
        ShaderWatcher { watcher, renderer_wrapper: renderer_wrapper_arc_mutex }
    }
}

struct App {
    renderer: Option<Arc<Mutex<renderer::Renderer>>>,
    window: Option<Arc<Window>>,
    shader_watcher: ShaderWatcher,
    glb_data: GLBObject,
}

impl App {
    pub fn new(glb_data: GLBObject) -> Self {
        Self { renderer: None, window: None, shader_watcher: ShaderWatcher::new(), glb_data }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(Window::default_attributes()).unwrap());
        self.window = Some(window.clone());
        let temp_renderer = renderer::Renderer::new(window.clone(), &self.glb_data).block_on();
        let renderer_arc_mutex = Arc::new(Mutex::new(temp_renderer));
        self.renderer = Some(renderer_arc_mutex.clone());
        let mut renderer_wrapper = self.shader_watcher.renderer_wrapper.lock().unwrap();
        *renderer_wrapper = Some(renderer_arc_mutex.clone());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    // renderer.update();
                    match renderer.render() {
                        Ok(_) => {},
                        Err(e) => eprintln!("render error: {:?}", e),
                    }
                }
            },
            WindowEvent::KeyboardInput { device_id, event: KeyEvent {
                state: ElementState::Released,
                physical_key: PhysicalKey::Code(KeyCode::Space),
                ..
            }, is_synthetic } => {
                return;
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    renderer.reload_shaders();
                }
            },
            WindowEvent::MouseWheel { device_id, delta, phase } => {
                // move camera in/out
            },
            WindowEvent::MouseInput { device_id, state, button } => {
                match button {
                    winit::event::MouseButton::Left => {
                        // start drag
                    },
                    _ => ()
                };
            },
            WindowEvent::AxisMotion { device_id, axis, value } => {
                // rotate camera if dragging
            },
            WindowEvent::Resized(physical_size) => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    renderer.resize(Some(physical_size));
                }
            },
            WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer } => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    renderer.resize(None);
                }
            },
            _ => (),
        }
    }
}

pub fn run(glb_data: GLBObject) {
    let mut app = App::new(glb_data);
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app).unwrap();
}


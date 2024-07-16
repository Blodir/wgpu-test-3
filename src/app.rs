use std::{sync::{Arc, Mutex, mpsc::channel}, path::Path, time::Duration, thread};
use cgmath::Rotation3;
use winit::{application::ApplicationHandler, dpi::PhysicalPosition, event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, keyboard::{KeyCode, PhysicalKey}, window::{Window, WindowId}};
use notify::{Watcher, RecommendedWatcher, Config};
use crate::{glb::{GLBObject, GLTFSceneRef}, renderer};
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

struct App<'a> {
    renderer: Option<Arc<Mutex<renderer::Renderer>>>,
    window: Option<Arc<Window>>,
    shader_watcher: ShaderWatcher,
    scene: GLTFSceneRef<'a>,
    mouse_btn_is_pressed: bool,
}

impl<'a> App<'a> {
    pub fn new(scene: GLTFSceneRef<'a>) -> Self {
        Self { renderer: None, window: None, shader_watcher: ShaderWatcher::new(), scene, mouse_btn_is_pressed: false }
    }
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(Window::default_attributes()).unwrap());
        self.window = Some(window.clone());
        let temp_renderer = renderer::Renderer::new(window.clone(), &self.scene).block_on();
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
            WindowEvent::MouseWheel { device_id, delta, phase } => {
                // move camera in/out
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    let camera = renderer.get_camera_mut();
                    match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            camera.eye.z = (camera.eye.z + (-y as f32)).max(0f32);
                            renderer.update_camera_bindings();
                            self.window.as_mut().unwrap().request_redraw();
                        },
                        MouseScrollDelta::PixelDelta(pos) => ()
                    }
                }
            },
            WindowEvent::MouseInput { device_id, state, button } => {
                match button {
                    winit::event::MouseButton::Left => {
                        match state {
                            ElementState::Pressed => {
                                self.mouse_btn_is_pressed = true;
                            },
                            ElementState::Released => {
                                self.mouse_btn_is_pressed = false;
                            },
                        }
                    },
                    _ => ()
                };
            },
            WindowEvent::Resized(physical_size) => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    renderer.resize(Some(physical_size));
                    self.window.as_mut().unwrap().request_redraw();
                }
            },
            WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer } => {
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    renderer.resize(None);
                    self.window.as_mut().unwrap().request_redraw();
                }
            },
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
                if !self.mouse_btn_is_pressed { return (); }
                if let Some(ref mut renderer_arc_mutex) = self.renderer {
                    let mut renderer = renderer_arc_mutex.lock().unwrap();
                    let camera = renderer.get_camera_mut();
                    let rot_x = cgmath::Quaternion::from_angle_y(cgmath::Deg((-x as f32) / 5f32));
                    let rot_y = cgmath::Quaternion::from_angle_x(cgmath::Deg((-y as f32) / 5f32));
                    camera.rotation = camera.rotation * rot_x * rot_y;
                    renderer.update_camera_bindings();
                    self.window.as_mut().unwrap().request_redraw();
                }
            },
            _ => (),
        }
    }
}

pub fn run(scene: GLTFSceneRef) {
    let mut app = App::new(scene);
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app).unwrap();
}


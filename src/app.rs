use std::{sync::{Arc, Mutex, mpsc::channel}, path::Path, time::Duration, thread};
use cgmath::{InnerSpace, Rotation3};
use winit::{application::ApplicationHandler, dpi::PhysicalPosition, event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, keyboard::{KeyCode, PhysicalKey}, window::{Window, WindowId}};
use notify::{Watcher, RecommendedWatcher, Config};
use pollster::FutureExt as _;

use crate::renderer::{glb::{GLBObject, GLTFSceneRef}, renderer::Renderer, PipelineCache, ShaderCache};

/*
struct ShaderWatcher<'cache> {
    watcher: RecommendedWatcher,
    pub renderer_wrapper: Arc<Mutex<Option<Arc<Mutex<Renderer<'cache>>>>>>,
}
impl<'cache> ShaderWatcher<'cache> {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
        watcher.watch(Path::new("src/shaders/"), notify::RecursiveMode::Recursive).unwrap();

        let renderer_wrapper_arc_mutex: Arc<Mutex<Option<Arc<Mutex<Renderer<'cache>>>>>> = Arc::new(Mutex::new(None));
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
*/

struct App<'scene, 'surface> {
    renderer: Option<Arc<Mutex<Renderer<'surface>>>>,
    window: Option<Arc<Window>>,
    //shader_watcher: ShaderWatcher<'cache>,
    scene: &'scene GLTFSceneRef<'scene>,
    mouse_btn_is_pressed: bool,
    shift_is_pressed: bool,
    pipeline_cache: PipelineCache,
    shader_cache: ShaderCache,
}

impl<'scene> App<'scene, '_> {
    pub fn new(
        scene: &'scene GLTFSceneRef<'scene>,
    ) -> Self {
        Self {
            renderer: None, window: None, //shader_watcher: ShaderWatcher::new(),
            scene, mouse_btn_is_pressed: false, shift_is_pressed: false,
            pipeline_cache: PipelineCache::default(), shader_cache: ShaderCache::default()
        }
    }
}

impl<'scene, 'surface> ApplicationHandler for App<'scene, 'surface> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(Window::default_attributes()).unwrap());
        self.window = Some(window.clone());

        let temp_renderer = Renderer::new(window.clone(), self.scene, &mut self.pipeline_cache, &mut self.shader_cache).block_on();
        let renderer_arc_mutex = Arc::new(Mutex::new(temp_renderer));
        self.renderer = Some(renderer_arc_mutex.clone());
        /*
        let mut renderer_wrapper = self.shader_watcher.renderer_wrapper.lock().unwrap();
        *renderer_wrapper = Some(renderer_arc_mutex.clone());
        */
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
                            camera.eye.z = (camera.eye.z + ((if self.shift_is_pressed { 10f32 } else { 1f32 }) * -y as f32)).max(0f32);
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
            WindowEvent::KeyboardInput { device_id, event, is_synthetic } => {
                match event {
                    KeyEvent { physical_key: PhysicalKey::Code(KeyCode::ShiftLeft), state: ElementState::Pressed, .. } => {
                        self.shift_is_pressed = true;
                    },
                    KeyEvent { physical_key: PhysicalKey::Code(KeyCode::ShiftLeft), state: ElementState::Released, .. } => {
                        self.shift_is_pressed = false;
                    },
                    _ => ()
                }
            }
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
                    let sensitivity = 5f32;
                    camera.rot_x = camera.rot_x - cgmath::Deg(x as f32 / sensitivity);
                    camera.rot_y = camera.rot_y - cgmath::Deg(y as f32 / sensitivity);
                    renderer.update_camera_bindings();
                    self.window.as_mut().unwrap().request_redraw();
                }
            },
            _ => (),
        }
    }
}

pub fn run(glb_data: &GLBObject) {
    let scene_ref = GLTFSceneRef::new(&glb_data);
    let mut app = App::new(&scene_ref);
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app).unwrap();
}


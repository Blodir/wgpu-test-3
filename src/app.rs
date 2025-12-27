use std::sync::{Arc, Mutex};

use crossbeam_queue::SegQueue;
use pollster::FutureExt as _;
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{DeviceEvent, WindowEvent}, event_loop::ActiveEventLoop, window::{Window, WindowId}};

use crate::{render_snapshot::SnapshotHandoff, renderer::{wgpu_context::WgpuContext, Renderer}, resource_manager::resource_manager::ResourceManager, sim::sim::InputEvent};

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

struct RenderContext<'surface> {
    renderer: Arc<Mutex<Renderer>>,
    window: Arc<Window>,
    wgpu_context: WgpuContext<'surface>,
}

pub struct App<'surface> {
    render_context: Option<RenderContext<'surface>>,
    snap_handoff: Arc<SnapshotHandoff>,
    sim_inputs: Arc<SegQueue<InputEvent>>,
    resource_manager: Arc<ResourceManager>,
}

impl App<'_> {
    pub fn new(sim_inputs: Arc<SegQueue<InputEvent>>, snap_handoff: Arc<SnapshotHandoff>, resource_manager: Arc<ResourceManager>) -> Self {
        Self {
            render_context: None,
            snap_handoff,
            sim_inputs,
            resource_manager,
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
        let wgpu_context = WgpuContext::new(window.clone()).block_on();
        let placeholders = self.resource_manager.gpu.initialize_placeholders(&wgpu_context);
        let renderer = Arc::new(
            Mutex::new(
                Renderer::new(&wgpu_context, self.snap_handoff.clone(), placeholders, &self.resource_manager)
            )
        );
        self.render_context = Some(
            RenderContext {
                window,
                renderer,
                wgpu_context,
            }
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.resource_manager.process_io_responses();
                if let Some(ref mut render_context) = self.render_context {
                    let mut renderer = render_context.renderer.lock().unwrap();
                    self.resource_manager.process_upload_queue(&mut renderer, &render_context.wgpu_context);
                    match renderer.render(
                        &render_context.wgpu_context,
                        &self.resource_manager,
                    ) {
                        Ok(_) => {}
                        Err(e) => eprintln!("render error: {:?}", e),
                    }
                }
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(ref mut render_context) = self.render_context {
                    let mut renderer = render_context.renderer.lock().unwrap();
                    let wgpu_context = &mut render_context.wgpu_context;
                    resize(physical_size, wgpu_context, &mut renderer);
                    let aspect = wgpu_context.surface_config.width as f32 / wgpu_context.surface_config.height as f32;
                    self.sim_inputs.push(InputEvent::AspectChange(aspect));
                }
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer,
            } => {
                if let Some(ref mut render_context) = self.render_context {
                    let mut renderer = render_context.renderer.lock().unwrap();
                    let wgpu_context = &mut render_context.wgpu_context;
                    let new_size = wgpu_context.window.inner_size();
                    resize(new_size, wgpu_context, &mut renderer);
                    let aspect = wgpu_context.surface_config.width as f32 / wgpu_context.surface_config.height as f32;
                    self.sim_inputs.push(InputEvent::AspectChange(aspect));
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
        self.render_context.as_mut().unwrap().window.request_redraw();
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

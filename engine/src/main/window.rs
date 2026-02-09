use std::sync::{Arc, Mutex};

use crossbeam_queue::SegQueue;
use pollster::FutureExt as _;
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{DeviceEvent, WindowEvent}, event_loop::ActiveEventLoop, window::{Window, WindowId}};

use crate::{job_system::worker_pool::RenderResponse, render_snapshot::SnapshotHandoff, main::{world::anim_pose_store::AnimPoseStore, wgpu_context::WgpuContext, world::Renderer}, main::assets::{store::RenderAssetStore, manager::RenderAssetManager}, game::sim::InputEvent};

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
    render_resources: RenderAssetStore,
    pose_storage: AnimPoseStore,
    window: Arc<Window>,
    wgpu_context: WgpuContext<'surface>,
    frame_idx: u32,
}

pub struct MainWindow<'surface> {
    render_context: Option<RenderContext<'surface>>,
    snap_handoff: Arc<SnapshotHandoff>,
    sim_inputs: Arc<SegQueue<InputEvent>>,
    resource_manager: RenderAssetManager,
    task_res_rx: crossbeam::channel::Receiver<RenderResponse>,
}

impl MainWindow<'_> {
    pub fn new(sim_inputs: Arc<SegQueue<InputEvent>>, snap_handoff: Arc<SnapshotHandoff>, resource_manager: RenderAssetManager, task_res_rx: crossbeam::channel::Receiver<RenderResponse>) -> Self {
        Self {
            render_context: None,
            snap_handoff,
            sim_inputs,
            resource_manager,
            task_res_rx
        }
    }
}

impl<'surface> ApplicationHandler for MainWindow<'surface> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        let wgpu_context = WgpuContext::new(window.clone()).block_on();
        let mut render_resources = RenderAssetStore::new();
        let placeholders = render_resources.initialize_placeholders(&wgpu_context);
        let renderer = Arc::new(
            Mutex::new(
                Renderer::new(&wgpu_context, self.snap_handoff.clone(), placeholders, &render_resources)
            )
        );
        let pose_storage = AnimPoseStore::new();
        self.render_context = Some(
            RenderContext {
                window,
                renderer,
                render_resources,
                pose_storage,
                wgpu_context,
                frame_idx: 0u32,
            }
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(ref mut render_context) = self.render_context {
                    self.resource_manager.process_io_responses(&mut render_context.render_resources, &render_context.wgpu_context);
                    let mut renderer = render_context.renderer.lock().unwrap();
                    self.resource_manager.process_game_responses(&mut renderer, &mut render_context.render_resources, &render_context.wgpu_context);
                    self.resource_manager.process_reg_requests();

                    for res in self.task_res_rx.try_iter() {
                        match res {
                            RenderResponse::Pose(anim_pose_task_results) => {
                                render_context.pose_storage.receive_poses(anim_pose_task_results);
                            },
                        }
                    }

                    // self.resource_manager.process_upload_queue(&mut renderer, &mut render_context.render_resources, &render_context.wgpu_context);
                    match renderer.render(
                        &render_context.wgpu_context,
                        &render_context.render_resources,
                        &mut render_context.pose_storage,
                        render_context.frame_idx,
                    ) {
                        Ok(_) => {}
                        Err(e) => eprintln!("render error: {:?}", e),
                    }

                    render_context.frame_idx += 1;
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

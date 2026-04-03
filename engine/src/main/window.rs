use std::sync::{Arc, Mutex};

use crossbeam::channel as cbch;
use crossbeam_queue::SegQueue;
use pollster::FutureExt as _;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::{
    fixed_snapshot_handoff::FixedSnapshotHandoff,
    game_trait::{BuildUiFn, InputEvent},
    job_system::worker_pool::RenderResponse,
    main::assets::{manager::MainAssetManager, store::RenderAssetStore},
    main::{renderer::Renderer, wgpu_context::WgpuContext},
    var_snapshot_handoff::VarSnapshotHandoff,
};

fn resize<S, C>(
    physical_size: PhysicalSize<u32>,
    wgpu_context: &mut WgpuContext,
    renderer: &mut Renderer<S, C>,
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

struct RenderContext<'surface, S, C> {
    renderer: Arc<Mutex<Renderer<S, C>>>,
    render_resources: RenderAssetStore,
    window: Arc<Window>,
    wgpu_context: WgpuContext<'surface>,
    frame_idx: u32,
}

pub struct MainWindow<'surface, S, C> {
    render_context: Option<RenderContext<'surface, S, C>>,
    fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
    var_snapshot_handoff: Arc<VarSnapshotHandoff<S>>,
    sim_inputs: Arc<SegQueue<InputEvent<C>>>,
    resource_manager: MainAssetManager,
    task_res_rx: cbch::Receiver<RenderResponse>,
    build_ui_fn: BuildUiFn<S, C>,
}

impl<S, C> MainWindow<'_, S, C> {
    pub fn new(
        sim_inputs: Arc<SegQueue<InputEvent<C>>>,
        fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
        resource_manager: MainAssetManager,
        task_res_rx: cbch::Receiver<RenderResponse>,
        var_snapshot_handoff: Arc<VarSnapshotHandoff<S>>,
        build_ui_fn: BuildUiFn<S, C>,
    ) -> Self {
        Self {
            render_context: None,
            fixed_snapshot_handoff,
            var_snapshot_handoff,
            sim_inputs,
            resource_manager,
            task_res_rx,
            build_ui_fn,
        }
    }
}

impl<'surface, S, C> ApplicationHandler for MainWindow<'surface, S, C>
where
    S: Send + Sync + 'static,
    C: Send + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        let wgpu_context = WgpuContext::new(window.clone()).block_on();
        let mut render_resources = RenderAssetStore::new();
        let placeholders = render_resources.initialize_placeholders(&wgpu_context);
        let renderer = Arc::new(Mutex::new(Renderer::new(
            &wgpu_context,
            self.fixed_snapshot_handoff.clone(),
            placeholders,
            &render_resources,
            self.build_ui_fn,
        )));
        self.render_context = Some(RenderContext {
            window,
            renderer,
            render_resources,
            wgpu_context,
            frame_idx: 0u32,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let mut gui_consumed = false;
        if let Some(ref mut render_context) = self.render_context {
            let mut renderer = render_context.renderer.lock().unwrap();
            gui_consumed = renderer.handle_window_event(&event);
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(ref mut render_context) = self.render_context {
                    self.resource_manager.process_io_responses(
                        &mut render_context.render_resources,
                        &render_context.wgpu_context,
                    );
                    let mut renderer = render_context.renderer.lock().unwrap();
                    self.resource_manager.process_game_responses(
                        &mut renderer,
                        &mut render_context.render_resources,
                        &render_context.wgpu_context,
                    );
                    self.resource_manager.process_reg_requests();

                    for res in self.task_res_rx.try_iter() {
                        match res {
                            RenderResponse::Pose(anim_pose_task_results) => {
                                renderer.receive_poses(anim_pose_task_results);
                            }
                        }
                    }

                    // self.resource_manager.process_upload_queue(&mut renderer, &mut render_context.render_resources, &render_context.wgpu_context);
                    let var_snapshot_guard = self.var_snapshot_handoff.load();
                    let var_snapshot = var_snapshot_guard.as_ref().map(|snapshot| &snapshot.snap);
                    let camera_pair = var_snapshot_guard
                        .as_ref()
                        .map(|snapshot| &snapshot.camera_pair);
                    let sim_debug = var_snapshot_guard
                        .as_ref()
                        .map(|snapshot| &snapshot.sim_debug);
                    let default_sim_debug = crate::game::sim::SimDebugInfo::default();

                    renderer.begin_frame(render_context.frame_idx);
                    let ui_commands = renderer.run_ui(
                        &render_context.wgpu_context,
                        var_snapshot,
                        sim_debug.unwrap_or(&default_sim_debug),
                    );
                    for cmd in ui_commands {
                        self.sim_inputs.push(InputEvent::Ui(cmd));
                    }

                    match renderer.render(
                        &render_context.wgpu_context,
                        &render_context.render_resources,
                        render_context.frame_idx,
                        camera_pair,
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
                    let aspect = wgpu_context.surface_config.width as f32
                        / wgpu_context.surface_config.height as f32;
                    self.sim_inputs.push(InputEvent::AspectChange(aspect));
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(ref mut render_context) = self.render_context {
                    let mut renderer = render_context.renderer.lock().unwrap();
                    let wgpu_context = &mut render_context.wgpu_context;
                    let new_size = wgpu_context.window.inner_size();
                    resize(new_size, wgpu_context, &mut renderer);
                    let aspect = wgpu_context.surface_config.width as f32
                        / wgpu_context.surface_config.height as f32;
                    self.sim_inputs.push(InputEvent::AspectChange(aspect));
                }
            }
            WindowEvent::MouseWheel { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::KeyboardInput { .. } => {
                if !gui_consumed {
                    self.sim_inputs.push(InputEvent::WindowEvent(event))
                }
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.render_context
            .as_mut()
            .unwrap()
            .window
            .request_redraw();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { .. } => {
                if let Some(ref render_context) = self.render_context {
                    let renderer = render_context.renderer.lock().unwrap();
                    if renderer.wants_pointer_input() {
                        return;
                    }
                }
                self.sim_inputs.push(InputEvent::DeviceEvent(event))
            }
            _ => (),
        }
    }
}

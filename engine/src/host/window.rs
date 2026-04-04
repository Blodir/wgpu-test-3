use std::sync::Arc;

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
    api::BuildUiFn,
    fixed_snapshot::FixedSnapshotHandoff,
    game::assets::{
        registry::{ResourceRequest, ResourceResult},
        store::{CreateGameResourceRequest, CreateGameResourceResponse},
    },
    game::sim::InputEvent,
    host::assets::{manager::HostAssetManager, store::RenderAssetStore},
    host::{renderer::Renderer, wgpu_context::WgpuContext},
    var_snapshot::VarSnapshotHandoff,
    workers::worker_pool::RenderResponse,
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
    renderer: Renderer<S, C>,
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
    resource_manager: HostAssetManager,
    worker_res_rx: cbch::Receiver<RenderResponse>,
    build_ui_fn: BuildUiFn<S, C>,
}

impl<S, C> MainWindow<'_, S, C> {
    pub fn new(
        sim_inputs: Arc<SegQueue<InputEvent<C>>>,
        fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
        registry_req_rx: cbch::Receiver<ResourceRequest>,
        registry_res_tx: cbch::Sender<ResourceResult>,
        game_res_rx: cbch::Receiver<CreateGameResourceResponse>,
        game_req_tx: cbch::Sender<CreateGameResourceRequest>,
        worker_res_rx: cbch::Receiver<RenderResponse>,
        var_snapshot_handoff: Arc<VarSnapshotHandoff<S>>,
        build_ui_fn: BuildUiFn<S, C>,
    ) -> Self {
        let resource_manager =
            HostAssetManager::new(registry_req_rx, registry_res_tx, game_res_rx, game_req_tx);
        Self {
            render_context: None,
            fixed_snapshot_handoff,
            var_snapshot_handoff,
            sim_inputs,
            resource_manager,
            worker_res_rx,
            build_ui_fn,
        }
    }
}

impl<'surface, S, C> ApplicationHandler for MainWindow<'surface, S, C>
where
    S: Send + Sync + Default + 'static,
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
        let brdf_lut = render_resources.initialize_brdf_lut(&wgpu_context);
        let renderer = Renderer::new(
            &wgpu_context,
            placeholders,
            brdf_lut,
            &render_resources,
            wgpu_context.renderer_options,
            self.build_ui_fn,
        );
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
            gui_consumed = render_context.renderer.handle_window_event(&event);
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
                    let renderer = &mut render_context.renderer;
                    self.resource_manager.process_game_responses(
                        renderer,
                        &mut render_context.render_resources,
                        &render_context.wgpu_context,
                    );
                    self.resource_manager.process_reg_requests();

                    for res in self.worker_res_rx.try_iter() {
                        match res {
                            RenderResponse::Pose(anim_pose_task_results) => {
                                renderer.receive_poses(anim_pose_task_results);
                            }
                        }
                    }

                    let var_snapshot_guard = self.var_snapshot_handoff.load();
                    let fixed_snapshot_guard = self.fixed_snapshot_handoff.load();
                    let var_snapshot = &var_snapshot_guard.snap;
                    let camera_pair = &var_snapshot_guard.camera_pair;
                    let sim_debug = &var_snapshot_guard.sim_debug;

                    renderer.begin_frame(render_context.frame_idx);
                    let ui_commands =
                        renderer.run_ui(&render_context.wgpu_context, var_snapshot, sim_debug);
                    for cmd in ui_commands {
                        self.sim_inputs.push(InputEvent::Ui(cmd));
                    }

                    match renderer.render(
                        &fixed_snapshot_guard,
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
                    let wgpu_context = &mut render_context.wgpu_context;
                    resize(physical_size, wgpu_context, &mut render_context.renderer);
                    let aspect = wgpu_context.surface_config.width as f32
                        / wgpu_context.surface_config.height as f32;
                    self.sim_inputs.push(InputEvent::AspectChange(aspect));
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(ref mut render_context) = self.render_context {
                    let wgpu_context = &mut render_context.wgpu_context;
                    let new_size = wgpu_context.window.inner_size();
                    resize(new_size, wgpu_context, &mut render_context.renderer);
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
                    if render_context.renderer.wants_pointer_input() {
                        return;
                    }
                }
                self.sim_inputs.push(InputEvent::DeviceEvent(event))
            }
            _ => (),
        }
    }
}

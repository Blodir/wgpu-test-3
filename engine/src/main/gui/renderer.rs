use std::sync::Arc;

use egui_wgpu::ScreenDescriptor;
use winit::{event::WindowEvent, window::Window};

use crate::{
    game_trait::{BuildUiFn, DebugInfo},
    main::wgpu_context::WgpuContext,
};

struct PendingGuiFrame {
    paint_jobs: Vec<egui::ClippedPrimitive>,
    textures_to_free: Vec<egui::TextureId>,
}

pub struct GuiRenderer<S, C> {
    window: Arc<Window>,
    build_ui_fn: BuildUiFn<S, C>,
    egui_context: egui::Context,
    egui_state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    screen_descriptor: ScreenDescriptor,
    wants_pointer_input: bool,
    pending_frame: Option<PendingGuiFrame>,
}

impl<S, C> GuiRenderer<S, C> {
    pub fn new(wgpu_context: &WgpuContext, build_ui_fn: BuildUiFn<S, C>) -> Self {
        let window = wgpu_context.window.clone();
        let egui_context = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(wgpu_context.device.limits().max_texture_dimension_2d as usize),
        );
        let renderer = egui_wgpu::Renderer::new(
            &wgpu_context.device,
            wgpu_context.surface_config.format,
            None,
            1,
            false,
        );

        Self {
            window,
            build_ui_fn,
            egui_context,
            egui_state,
            renderer,
            screen_descriptor: screen_descriptor_from_context(wgpu_context),
            wants_pointer_input: false,
            pending_frame: None,
        }
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.screen_descriptor = screen_descriptor_from_context(wgpu_context);
    }

    pub fn wants_pointer_input(&self) -> bool {
        self.wants_pointer_input
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        self.egui_state
            .on_window_event(self.window.as_ref(), event)
            .consumed
    }

    pub fn run_ui(
        &mut self,
        wgpu_context: &WgpuContext,
        ui_snapshot: Option<&S>,
        debug_info: &DebugInfo,
    ) -> Vec<C> {
        self.screen_descriptor = screen_descriptor_from_context(wgpu_context);
        let mut ui_commands = Vec::new();

        let raw_input = self.egui_state.take_egui_input(self.window.as_ref());
        let full_output = self.egui_context.run(raw_input, |ctx| {
            let mut emit = |cmd: C| ui_commands.push(cmd);
            (self.build_ui_fn)(ctx, ui_snapshot, debug_info, &mut emit);
        });
        self.wants_pointer_input = self.egui_context.wants_pointer_input();

        self.egui_state
            .handle_platform_output(self.window.as_ref(), full_output.platform_output);

        let paint_jobs = self
            .egui_context
            .tessellate(full_output.shapes, self.screen_descriptor.pixels_per_point);

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(
                &wgpu_context.device,
                &wgpu_context.queue,
                *id,
                image_delta,
            );
        }
        self.pending_frame = Some(PendingGuiFrame {
            paint_jobs,
            textures_to_free: full_output.textures_delta.free,
        });

        ui_commands
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
    ) {
        let Some(frame) = self.pending_frame.take() else {
            return;
        };

        self.renderer.update_buffers(
            &wgpu_context.device,
            &wgpu_context.queue,
            encoder,
            &frame.paint_jobs,
            &self.screen_descriptor,
        );
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GUI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            let mut render_pass = render_pass.forget_lifetime();
            let _ =
                self.renderer
                    .render(&mut render_pass, &frame.paint_jobs, &self.screen_descriptor);
        }

        for texture_id in frame.textures_to_free {
            self.renderer.free_texture(&texture_id);
        }
    }
}

fn screen_descriptor_from_context(wgpu_context: &WgpuContext) -> ScreenDescriptor {
    ScreenDescriptor {
        size_in_pixels: [
            wgpu_context.surface_config.width,
            wgpu_context.surface_config.height,
        ],
        pixels_per_point: wgpu_context.window.scale_factor() as f32,
    }
}

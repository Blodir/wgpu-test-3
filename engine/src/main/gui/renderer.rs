use std::time::Instant;

use egui::{Modifiers, Pos2, Rect, Vec2};
use egui_wgpu::ScreenDescriptor;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use crate::main::wgpu_context::WgpuContext;

pub struct GuiRenderer {
    egui_context: egui::Context,
    renderer: egui_wgpu::Renderer,
    raw_input: egui::RawInput,
    screen_descriptor: ScreenDescriptor,
    start_time: Instant,
    pointer_pos: Pos2,
    modifiers: Modifiers,
    pixels_per_point: f32,
}

impl GuiRenderer {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let egui_context = egui::Context::default();
        let renderer = egui_wgpu::Renderer::new(
            &wgpu_context.device,
            wgpu_context.surface_config.format,
            None,
            1,
        );
        let screen_descriptor = screen_descriptor_from_context(wgpu_context);
        let raw_input = egui::RawInput::default();

        Self {
            egui_context,
            renderer,
            raw_input,
            screen_descriptor,
            start_time: Instant::now(),
            pointer_pos: Pos2::new(0.0, 0.0),
            modifiers: Modifiers::default(),
            pixels_per_point: wgpu_context.window.scale_factor() as f32,
        }
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.pixels_per_point = wgpu_context.window.scale_factor() as f32;
        self.screen_descriptor = screen_descriptor_from_context(wgpu_context);
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_pos = Pos2::new(
                    position.x as f32 / self.pixels_per_point,
                    position.y as f32 / self.pixels_per_point,
                );
                self.raw_input
                    .events
                    .push(egui::Event::PointerMoved(self.pointer_pos));
            }
            WindowEvent::CursorLeft { .. } => {
                self.raw_input.events.push(egui::Event::PointerGone);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(egui_button) = map_mouse_button(*button) {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: self.pointer_pos,
                        button: egui_button,
                        pressed: *state == ElementState::Pressed,
                        modifiers: self.modifiers,
                    });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Vec2::new(*x * 16.0, *y * 16.0),
                    MouseScrollDelta::PixelDelta(delta) => Vec2::new(
                        delta.x as f32 / self.pixels_per_point,
                        delta.y as f32 / self.pixels_per_point,
                    ),
                };
                self.raw_input
                    .events
                    .push(egui::Event::Scroll(scroll_delta));
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = map_modifiers(modifiers.state());
            }
            _ => {}
        }
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        frame_idx: u32,
    ) {
        self.pixels_per_point = wgpu_context.window.scale_factor() as f32;
        self.screen_descriptor = screen_descriptor_from_context(wgpu_context);
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.raw_input.modifiers = self.modifiers;
        self.egui_context
            .set_pixels_per_point(self.pixels_per_point);
        self.raw_input.screen_rect = Some(Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            Vec2::new(
                wgpu_context.surface_config.width as f32 / self.pixels_per_point,
                wgpu_context.surface_config.height as f32 / self.pixels_per_point,
            ),
        ));

        let raw_input = std::mem::take(&mut self.raw_input);
        let full_output = self.egui_context.run(raw_input, |ctx| {
            self.build_ui(ctx, frame_idx);
        });
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
        self.renderer.update_buffers(
            &wgpu_context.device,
            &wgpu_context.queue,
            encoder,
            &paint_jobs,
            &self.screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            let _ = self
                .renderer
                .render(&mut render_pass, &paint_jobs, &self.screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    fn build_ui(&self, ctx: &egui::Context, frame_idx: u32) {
        egui::Window::new("Renderer UI").show(ctx, |ui| {
            ui.label("egui boilerplate wired through main renderer");
            ui.label(format!("frame index: {}", frame_idx));
        });
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

fn map_mouse_button(button: MouseButton) -> Option<egui::PointerButton> {
    match button {
        MouseButton::Left => Some(egui::PointerButton::Primary),
        MouseButton::Right => Some(egui::PointerButton::Secondary),
        MouseButton::Middle => Some(egui::PointerButton::Middle),
        _ => None,
    }
}

fn map_modifiers(modifiers: winit::keyboard::ModifiersState) -> Modifiers {
    Modifiers {
        alt: modifiers.alt_key(),
        ctrl: modifiers.control_key(),
        shift: modifiers.shift_key(),
        mac_cmd: modifiers.super_key(),
        command: modifiers.control_key() || modifiers.super_key(),
    }
}

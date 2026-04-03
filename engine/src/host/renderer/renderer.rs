use std::{sync::Arc, time::Instant};

use winit::event::WindowEvent;

use super::gui::GuiRenderer;
use super::world::bindgroups::material::MaterialBinding;
use super::world::WorldRenderer;
use crate::fixed_snapshot_handoff::FixedSnapshotHandoff;
use crate::game::sim::SimDebugInfo;
use crate::game_trait::BuildUiFn;
use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
use crate::host::wgpu_context::WgpuContext;
pub use crate::host::renderer::world::UploadMaterialRequest;
use crate::var_snapshot_handoff::CameraSnapshotPair;
use crate::workers::worker_pool::AnimPoseTaskResult;

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderDebugInfo {
    pub frame_index: u32,
    pub fps: f32,
    pub frame_time_ms: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugInfo {
    pub render: RenderDebugInfo,
    pub sim: SimDebugInfo,
}

pub struct Renderer<S, C> {
    world_renderer: WorldRenderer,
    gui_renderer: GuiRenderer<S, C>,
    last_frame_instant: Option<Instant>,
    frame_fps_smoothed: f32,
    render_debug: RenderDebugInfo,
}

impl<S, C> Renderer<S, C> {
    pub fn new(
        wgpu_context: &WgpuContext,
        fixed_snapshot_handoff: Arc<FixedSnapshotHandoff>,
        placeholders: PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        render_resources: &RenderAssetStore,
        build_ui_fn: BuildUiFn<S, C>,
    ) -> Self {
        let world_renderer = WorldRenderer::new(
            wgpu_context,
            fixed_snapshot_handoff,
            placeholders,
            brdf_lut,
            render_resources,
        );
        let gui_renderer = GuiRenderer::new(wgpu_context, build_ui_fn);
        Self {
            world_renderer,
            gui_renderer,
            last_frame_instant: None,
            frame_fps_smoothed: 0.0,
            render_debug: RenderDebugInfo::default(),
        }
    }

    pub fn run_ui(
        &mut self,
        wgpu_context: &WgpuContext,
        ui_snapshot: Option<&S>,
        sim_debug: &SimDebugInfo,
    ) -> Vec<C> {
        let debug_info = DebugInfo {
            render: self.render_debug,
            sim: *sim_debug,
        };
        self.gui_renderer
            .run_ui(wgpu_context, ui_snapshot, &debug_info)
    }

    pub fn begin_frame(&mut self, frame_index: u32) {
        self.update_render_debug(frame_index);
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        frame_idx: u32,
        camera_pair: Option<&CameraSnapshotPair>,
    ) -> Result<(), wgpu::SurfaceError> {
        let output_surface_texture = wgpu_context.surface.get_current_texture()?;
        let output_view = output_surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            wgpu_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Main Render Encoder"),
                });

        self.world_renderer.render(
            wgpu_context,
            render_resources,
            frame_idx,
            &mut encoder,
            &output_view,
            camera_pair,
        );
        self.gui_renderer
            .render(wgpu_context, &mut encoder, &output_view);

        wgpu_context.queue.submit(Some(encoder.finish()));
        output_surface_texture.present();
        Ok(())
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.world_renderer.resize(wgpu_context);
        self.gui_renderer.resize(wgpu_context);
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        self.gui_renderer.handle_window_event(event)
    }

    pub fn wants_pointer_input(&self) -> bool {
        self.gui_renderer.wants_pointer_input()
    }

    pub fn receive_poses(&mut self, anim_pose_task_results: AnimPoseTaskResult) {
        self.world_renderer.receive_poses(anim_pose_task_results);
    }

    pub fn upload_material(
        &mut self,
        request: UploadMaterialRequest<'_>,
        render_resources: &RenderAssetStore,
        wgpu_context: &WgpuContext,
    ) -> Result<MaterialBinding, ()> {
        self.world_renderer
            .upload_material(request, render_resources, wgpu_context)
    }

    fn update_render_debug(&mut self, frame_index: u32) {
        let now = Instant::now();
        if let Some(prev) = self.last_frame_instant {
            let dt = (now - prev).as_secs_f32();
            if dt > 0.0 {
                let fps = 1.0 / dt;
                self.frame_fps_smoothed = if self.frame_fps_smoothed <= 0.0 {
                    fps
                } else {
                    (self.frame_fps_smoothed * 0.9) + (fps * 0.1)
                };
            }
        }
        self.last_frame_instant = Some(now);
        self.render_debug = RenderDebugInfo {
            frame_index,
            fps: self.frame_fps_smoothed,
            frame_time_ms: if self.frame_fps_smoothed > 0.0 {
                1000.0 / self.frame_fps_smoothed
            } else {
                0.0
            },
        };
    }
}

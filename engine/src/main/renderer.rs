use std::sync::Arc;

use winit::event::WindowEvent;

use super::assets::store::{PlaceholderTextureIds, RenderAssetStore};
use super::gui::GuiRenderer;
use super::wgpu_context::WgpuContext;
use super::world::bindgroups::material::MaterialBinding;
use super::world::WorldRenderer;
use crate::game_trait::BuildUiFn;
use crate::job_system::worker_pool::AnimPoseTaskResult;
pub use crate::main::world::UploadMaterialRequest;
use crate::render_snapshot_handoff::RenderSnapshotHandoff;

pub struct Renderer<S, C> {
    world_renderer: WorldRenderer,
    gui_renderer: GuiRenderer<S, C>,
}

impl<S, C> Renderer<S, C> {
    pub fn new(
        wgpu_context: &WgpuContext,
        snapshot_handoff: Arc<RenderSnapshotHandoff>,
        placeholders: PlaceholderTextureIds,
        render_resources: &RenderAssetStore,
        build_ui_fn: BuildUiFn<S, C>,
    ) -> Self {
        let world_renderer = WorldRenderer::new(
            wgpu_context,
            snapshot_handoff,
            placeholders,
            render_resources,
        );
        let gui_renderer = GuiRenderer::new(wgpu_context, build_ui_fn);
        Self {
            world_renderer,
            gui_renderer,
        }
    }

    pub fn run_ui(
        &mut self,
        wgpu_context: &WgpuContext,
        frame_idx: u32,
        ui_snapshot: Option<&S>,
    ) -> Vec<C> {
        self.gui_renderer
            .run_ui(wgpu_context, frame_idx, ui_snapshot)
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        frame_idx: u32,
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
}

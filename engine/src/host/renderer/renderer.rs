use std::time::Instant;

use winit::event::WindowEvent;

use super::gui::GuiRenderer;
use super::sampler_cache::SamplerCache;
use super::shader_cache::ShaderCache;
use super::world::bindgroups::material::MaterialBinding;
use super::world::WorldRenderer;
use crate::api::BuildUiFn;
use crate::fixed_snapshot::FixedSnapshotGuard;
use crate::game::sim::SimDebugInfo;
use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
pub use crate::host::renderer::world::UploadMaterialRequest;
use crate::host::wgpu_context::WgpuContext;
use crate::var_snapshot::CameraSnapshotPair;
use crate::workers::anim_pose::PoseJobResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpaqueRenderPath {
    Deferred,
    CompactDeferred,
    #[default]
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererOptions {
    pub opaque_render_path: OpaqueRenderPath,
}
impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            opaque_render_path: OpaqueRenderPath::Forward,
        }
    }
}
impl RendererOptions {
    pub fn from_limits(limits: &wgpu::Limits) -> Self {
        // Conservative heuristic based on MRT availability for deferred paths.
        let opaque_render_path = if limits.max_color_attachments >= 6 {
            OpaqueRenderPath::Deferred
        } else if limits.max_color_attachments >= 4 {
            OpaqueRenderPath::CompactDeferred
        } else {
            OpaqueRenderPath::Forward
        };

        Self { opaque_render_path }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderDebugInfo {
    pub frame_index: u32,
    pub fps: f32,
    pub frame_time_ms: f32,
    pub opaque_render_path: OpaqueRenderPath,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugInfo {
    pub render: RenderDebugInfo,
    pub sim: SimDebugInfo,
}

pub struct Renderer<S, C> {
    world_renderer: WorldRenderer,
    gui_renderer: GuiRenderer<S, C>,
    sampler_cache: SamplerCache,
    _shader_cache: ShaderCache,
    options: RendererOptions,
    last_frame_instant: Option<Instant>,
    frame_fps_smoothed: f32,
    render_debug: RenderDebugInfo,
}

impl<S, C> Renderer<S, C> {
    pub fn new(
        wgpu_context: &WgpuContext,
        placeholders: PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        render_resources: &RenderAssetStore,
        options: RendererOptions,
        build_ui_fn: BuildUiFn<S, C>,
    ) -> Self {
        let mut sampler_cache = SamplerCache::new();
        let mut shader_cache = ShaderCache::new();
        let world_renderer = WorldRenderer::new(
            wgpu_context,
            placeholders,
            brdf_lut,
            &mut sampler_cache,
            &mut shader_cache,
            render_resources,
            options,
        );
        let gui_renderer = GuiRenderer::new(wgpu_context, build_ui_fn);
        let render_debug = RenderDebugInfo {
            opaque_render_path: options.opaque_render_path,
            ..Default::default()
        };
        Self {
            world_renderer,
            gui_renderer,
            sampler_cache,
            _shader_cache: shader_cache,
            options,
            last_frame_instant: None,
            frame_fps_smoothed: 0.0,
            render_debug,
        }
    }

    pub fn run_ui(
        &mut self,
        wgpu_context: &WgpuContext,
        ui_snapshot: &S,
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
        fixed_snapshot_guard: &FixedSnapshotGuard,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        frame_idx: u32,
        camera_pair: &CameraSnapshotPair,
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
            fixed_snapshot_guard,
            wgpu_context,
            render_resources,
            &mut self.sampler_cache,
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

    /// returns true if event was consumed by the gui
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        self.gui_renderer.handle_window_event(event)
    }

    pub fn wants_pointer_input(&self) -> bool {
        self.gui_renderer.wants_pointer_input()
    }

    pub fn receive_poses(&mut self, anim_pose_task_results: PoseJobResult) {
        self.world_renderer.receive_poses(anim_pose_task_results);
    }

    pub fn upload_material(
        &mut self,
        request: UploadMaterialRequest<'_>,
        render_resources: &RenderAssetStore,
        wgpu_context: &WgpuContext,
    ) -> Result<MaterialBinding, ()> {
        self.world_renderer.upload_material(
            request,
            render_resources,
            &mut self.sampler_cache,
            wgpu_context,
        )
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
            opaque_render_path: self.options.opaque_render_path,
        };
    }
}

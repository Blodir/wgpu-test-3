use std::sync::Arc;

use winit::event::WindowEvent;

use super::assets::io::asset_formats::materialfile;
use super::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};
use super::gui::GuiRenderer;
use super::wgpu_context::WgpuContext;
use super::world::anim_pose_store::AnimPoseStore;
use super::world::bindgroups::material::MaterialBinding;
use super::world::WorldRenderer;
use crate::snapshot_handoff::SnapshotHandoff;

pub struct Renderer {
    world_renderer: WorldRenderer,
    gui_renderer: GuiRenderer,
}

impl Renderer {
    pub fn new(
        wgpu_context: &WgpuContext,
        snapshot_handoff: Arc<SnapshotHandoff>,
        placeholders: PlaceholderTextureIds,
        render_resources: &RenderAssetStore,
    ) -> Self {
        let world_renderer = WorldRenderer::new(
            wgpu_context,
            snapshot_handoff,
            placeholders,
            render_resources,
        );
        let gui_renderer = GuiRenderer::new(wgpu_context);
        Self {
            world_renderer,
            gui_renderer,
        }
    }

    pub fn render(
        &mut self,
        wgpu_context: &WgpuContext,
        render_resources: &RenderAssetStore,
        pose_storage: &mut AnimPoseStore,
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
            pose_storage,
            frame_idx,
            &mut encoder,
            &output_view,
        );
        self.gui_renderer
            .render(wgpu_context, &mut encoder, &output_view, frame_idx);

        wgpu_context.queue.submit(Some(encoder.finish()));
        output_surface_texture.present();
        Ok(())
    }

    pub fn resize(&mut self, wgpu_context: &WgpuContext) {
        self.world_renderer.resize(wgpu_context);
        self.gui_renderer.resize(wgpu_context);
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        self.gui_renderer.handle_window_event(event);
    }

    pub fn upload_material(
        &mut self,
        manifest: &materialfile::Material,
        normal_texture: &Option<TextureRenderId>,
        occlusion_texture: &Option<TextureRenderId>,
        emissive_texture: &Option<TextureRenderId>,
        base_color_texture: &Option<TextureRenderId>,
        metallic_roughness_texture: &Option<TextureRenderId>,
        render_resources: &RenderAssetStore,
        wgpu_context: &WgpuContext,
    ) -> Result<MaterialBinding, ()> {
        self.world_renderer.upload_material(
            manifest,
            normal_texture,
            occlusion_texture,
            emissive_texture,
            base_color_texture,
            metallic_roughness_texture,
            render_resources,
            wgpu_context,
        )
    }
}

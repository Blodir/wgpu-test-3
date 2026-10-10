use crate::host::renderer::{
    rw_texture::{RWTexture, RWTextureView},
    world::gpu_context::resources::textures::SsrtTextures,
};

pub(crate) struct SsrtTextureViews {
    pub normal_roughness_history: RWTextureView,
    pub diffuse_history: RWTextureView,
    pub ssgi_history: RWTextureView,
    pub bent_normal_ao_history: RWTextureView,
    pub ssr_history: RWTextureView,
    pub moments_history: RWTextureView,
    pub reprojected_diffuse: wgpu::TextureView,
    pub reprojected_diffuse_mips: Vec<wgpu::TextureView>,
    pub reprojected_bent_normal_ao: wgpu::TextureView,
    pub ssgi_trace: wgpu::TextureView,
    pub bent_normal_ao_trace: wgpu::TextureView,
    pub ssr_trace: wgpu::TextureView,
    pub ssgi_denoise: RWTextureView,
    pub bent_normal_ao_denoise: RWTextureView,
    pub ssr_denoise: wgpu::TextureView,
    pub final_hdr: wgpu::TextureView,
    pub final_hdr_history: RWTextureView,
    pub depth_history: RWTextureView,
    pub depth_history_mips: Vec<RWTextureView>,
}

impl SsrtTextureViews {
    pub fn new(textures: &SsrtTextures) -> Self {
        let create_view =
            |texture: &wgpu::Texture| texture.create_view(&wgpu::TextureViewDescriptor::default());
        let create_rw_view = |texture: &RWTexture| {
            RWTextureView::new(texture, &wgpu::TextureViewDescriptor::default())
        };
        Self {
            normal_roughness_history: create_rw_view(&textures.normal_roughness_history),
            diffuse_history: create_rw_view(&textures.diffuse_history),
            ssgi_history: create_rw_view(&textures.ssgi_history),
            bent_normal_ao_history: create_rw_view(&textures.bent_normal_ao_history),
            ssr_history: create_rw_view(&textures.ssr_history),
            moments_history: create_rw_view(&textures.moments_history),
            reprojected_diffuse: create_view(&textures.reprojected_diffuse),
            reprojected_diffuse_mips: (0..textures.reprojected_diffuse.mip_level_count())
                .map(|mip_level| {
                    textures
                        .reprojected_diffuse
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: Some("Reprojected Diffuse Mip"),
                            base_mip_level: mip_level,
                            mip_level_count: Some(1),
                            ..Default::default()
                        })
                })
                .collect(),
            reprojected_bent_normal_ao: create_view(&textures.reprojected_bent_normal_ao),
            ssgi_trace: create_view(&textures.ssgi_trace),
            bent_normal_ao_trace: create_view(&textures.bent_normal_ao_trace),
            ssr_trace: create_view(&textures.ssr_trace),
            ssgi_denoise: create_rw_view(&textures.ssgi_denoise),
            bent_normal_ao_denoise: create_rw_view(&textures.bent_normal_ao_denoise),
            ssr_denoise: create_view(&textures.ssr_denoise),
            final_hdr: create_view(&textures.final_hdr),
            final_hdr_history: create_rw_view(&textures.final_hdr_history),
            depth_history: create_rw_view(&textures.depth_history),
            depth_history_mips: (0..textures.depth_history.get_write_tex().mip_level_count())
                .map(|mip_level| {
                    RWTextureView::new(
                        &textures.depth_history,
                        &wgpu::TextureViewDescriptor {
                            label: Some("Depth History Mip"),
                            base_mip_level: mip_level,
                            mip_level_count: Some(1),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        }
    }
}

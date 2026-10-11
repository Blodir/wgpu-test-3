use crate::host::{renderer::rw_texture::RWTexture, wgpu_context::WgpuContext};

pub(crate) const SSRT_A_TROUS_PASS_COUNT: usize = 5;

pub(crate) struct SsrtTextures {
    pub normal_roughness_history: RWTexture,
    pub normal_variance: wgpu::Texture,
    pub diffuse_history: RWTexture,
    pub ssgi_history: RWTexture,
    pub bent_normal_ao_history: RWTexture,
    pub ssr_history: RWTexture,
    pub moments_history: RWTexture,
    pub reprojected_diffuse: wgpu::Texture,
    pub reprojected_bent_normal_ao: wgpu::Texture,
    pub ssgi_trace: wgpu::Texture,
    pub bent_normal_ao_trace: wgpu::Texture,
    pub ssr_trace: wgpu::Texture,
    pub ssgi_denoise: RWTexture,
    pub bent_normal_ao_denoise: RWTexture,
    pub ssr_denoise: wgpu::Texture,
    pub final_hdr: wgpu::Texture,
    pub final_hdr_history: RWTexture,
    pub depth_history: RWTexture,
}

impl SsrtTextures {
    pub fn new(wgpu_context: &WgpuContext, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let full_size = wgpu::Extent3d {
            width: surface_config.width.max(1),
            height: surface_config.height.max(1),
            depth_or_array_layers: 1,
        };
        let half_size = wgpu::Extent3d {
            width: (full_size.width / 2).max(1),
            height: (full_size.height / 2).max(1),
            ..full_size
        };
        let descriptor = |label, size| wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let create_texture = |label| {
            wgpu_context
                .device
                .create_texture(&descriptor(label, full_size))
        };
        let create_rw_texture = |label| RWTexture::new(descriptor(label, full_size), wgpu_context);
        let create_half_texture = |label| {
            wgpu_context
                .device
                .create_texture(&descriptor(label, half_size))
        };
        let create_half_rw_texture =
            |label| RWTexture::new(descriptor(label, half_size), wgpu_context);

        Self {
            normal_roughness_history: create_half_rw_texture("Normal Roughness History"),
            normal_variance: wgpu_context
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    format: wgpu::TextureFormat::R16Float,
                    ..descriptor("Normal Variance", half_size)
                }),
            diffuse_history: create_rw_texture("Diffuse History"),
            ssgi_history: create_half_rw_texture("SSGI History"),
            bent_normal_ao_history: create_half_rw_texture("Bent Normal AO History"),
            ssr_history: create_half_rw_texture("SSR History"),
            moments_history: create_half_rw_texture("Moments History"),
            reprojected_diffuse: wgpu_context
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    mip_level_count: half_size.width.max(half_size.height).ilog2() + 1,
                    ..descriptor("Reprojected Diffuse", half_size)
                }),
            reprojected_bent_normal_ao: create_half_texture("Reprojected Bent Normal AO"),
            ssgi_trace: create_half_texture("SSGI Trace"),
            bent_normal_ao_trace: create_half_texture("Bent Normal AO Trace"),
            ssr_trace: create_half_texture("SSR Trace"),
            ssgi_denoise: create_half_rw_texture("SSGI Denoise"),
            bent_normal_ao_denoise: create_half_rw_texture("Bent Normal AO Denoise"),
            ssr_denoise: create_half_texture("SSR Denoise"),
            final_hdr: create_texture("Final HDR"),
            final_hdr_history: create_rw_texture("Final HDR History"),
            depth_history: RWTexture::new(
                wgpu::TextureDescriptor {
                    mip_level_count: (full_size.width.max(full_size.height).ilog2() + 1).min(2),
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST,
                    ..descriptor("Depth History", full_size)
                },
                wgpu_context,
            ),
        }
    }

    pub fn rotate(&mut self) {
        self.normal_roughness_history.swap();
        self.diffuse_history.swap();
        self.ssgi_history.swap();
        self.bent_normal_ao_history.swap();
        self.ssr_history.swap();
        self.moments_history.swap();
        self.final_hdr_history.swap();
        self.depth_history.swap();
    }
}

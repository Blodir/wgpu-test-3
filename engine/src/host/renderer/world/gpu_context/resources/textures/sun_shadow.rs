use crate::host::wgpu_context::WgpuContext;

pub(crate) struct SunShadowTexture2(pub(crate) wgpu::Texture);

impl SunShadowTexture2 {
    pub const SHADOW_MAP_SIZE: u32 = crate::host::world::sun_shadow::SUN_SHADOW_MAP_SIZE;
    pub const CASCADE_COUNT: u32 =
        crate::host::world::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT as u32;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let tex = wgpu_context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Sun Shadow Texture"),
                size: wgpu::Extent3d {
                    width: Self::SHADOW_MAP_SIZE,
                    height: Self::SHADOW_MAP_SIZE,
                    depth_or_array_layers: Self::CASCADE_COUNT,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        Self(tex)
    }
}

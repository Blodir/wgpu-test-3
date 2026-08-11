use crate::host::assets::store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId};

pub(crate) struct ExternalTextureViews {
    pub(crate) brdf_lut: wgpu::TextureView,
    pub(crate) blue_noise: wgpu::TextureView,
    pub(crate) placeholder_prefiltered: wgpu::TextureView,
    pub(crate) placeholder_di: wgpu::TextureView,
}

impl ExternalTextureViews {
    fn create_d2_view(texture: &wgpu::Texture, label: &'static str) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(label),
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        })
    }

    fn create_cube_view(texture: &wgpu::Texture, label: &'static str) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(label),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        })
    }

    pub(crate) fn new(
        render_resources: &RenderAssetStore,
        placeholders: &PlaceholderTextureIds,
        brdf_lut: TextureRenderId,
        blue_noise: TextureRenderId,
    ) -> Self {
        let textures = &render_resources.textures;
        let brdf_lut = Self::create_d2_view(
            &textures.get(brdf_lut.into()).unwrap().texture,
            "External: BRDF LUT Texture View",
        );
        let blue_noise = Self::create_d2_view(
            &textures.get(blue_noise.into()).unwrap().texture,
            "External: Blue Noise Texture View",
        );
        let placeholder_prefiltered = Self::create_cube_view(
            &textures
                .get(placeholders.prefiltered.into())
                .unwrap()
                .texture,
            "External: Placeholder Prefiltered Texture View",
        );
        let placeholder_di = Self::create_cube_view(
            &textures.get(placeholders.di.into()).unwrap().texture,
            "External: Placeholder DI Texture View",
        );

        Self {
            brdf_lut,
            blue_noise,
            placeholder_prefiltered,
            placeholder_di,
        }
    }

    pub(crate) fn resolve_environment_views<'a>(
        &'a self,
        render_resources: &'a RenderAssetStore,
        environment_map: Option<(TextureRenderId, TextureRenderId)>,
    ) -> (&'a wgpu::TextureView, &'a wgpu::TextureView) {
        if let Some((prefiltered_id, di_id)) = environment_map {
            let textures = &render_resources.textures;
            let prefiltered = &textures.get(prefiltered_id.into()).unwrap().texture_view;
            let di = &textures.get(di_id.into()).unwrap().texture_view;
            (prefiltered, di)
        } else {
            (&self.placeholder_prefiltered, &self.placeholder_di)
        }
    }
}

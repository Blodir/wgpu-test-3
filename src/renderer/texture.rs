use super::pbr::SamplerOptions;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img_and_sampler: &(image::DynamicImage, Option<SamplerOptions>),
        srgb: bool,
    ) -> Self {
        let img = &img_and_sampler.0;
        let sampler_options = &img_and_sampler.1;
        let rgba = img.to_rgba8();
        let dimensions = image::GenericImageView::dimensions(img);

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let format = if srgb { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(
            &sampler_options.as_ref().map(
                |s| wgpu::SamplerDescriptor {
                    address_mode_u: s.address_mode_u,
                    address_mode_v: s.address_mode_v,
                    mag_filter: s.mag_filter,
                    min_filter: s.min_filter,
                    ..wgpu::SamplerDescriptor::default()
                }
            ).unwrap_or(wgpu::SamplerDescriptor::default())
        );

        Self { view, sampler, texture }
    }
}


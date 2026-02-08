use wgpu::FilterMode;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        srgb: bool,
    ) -> Self {
        let dimensions = image::GenericImageView::dimensions(img);

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let (remapped, format): (Vec<u8>, wgpu::TextureFormat) = match (img, srgb) {
            (image::DynamicImage::ImageRgb32F(_), false) => (
                bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
                wgpu::TextureFormat::Rgba32Float,
            ),
            (image::DynamicImage::ImageRgba32F(_), false) => (
                bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
                wgpu::TextureFormat::Rgba32Float,
            ),
            (_, true) => (
                bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
                wgpu::TextureFormat::Rgba8UnormSrgb,
            ),
            (_, false) => (
                bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
                wgpu::TextureFormat::Rgba8Unorm,
            ),
        };
        let bytes_per_row = match format {
            wgpu::TextureFormat::Rgba32Float => 4 * 4 * dimensions.0,
            _ => 4 * dimensions.0,
        };
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
            &remapped,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        Self {
            view,
            sampler,
            texture,
        }
    }
}

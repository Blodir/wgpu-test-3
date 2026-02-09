use wgpu::util::DeviceExt as _;

use crate::{game::scene_tree, main::{assets::{io::asset_formats::materialfile, store::{PlaceholderTextureIds, RenderAssetStore, TextureRenderId}}, sampler_cache::SamplerCache, wgpu_context::WgpuContext}};

pub struct LightsBinding {
    pub sun_direction_buffer: wgpu::Buffer,
    pub sun_color_buffer: wgpu::Buffer,
    pub curr_prefiltered_render_id: TextureRenderId,
    pub curr_di_render_id: TextureRenderId,
    pub curr_brdf_render_id: TextureRenderId,
    pub bind_group: wgpu::BindGroup,
}
impl LightsBinding {
    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // sun dir
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // sun color
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // prefiltered
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Diffuse irradiance
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // BRDF LUT
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Lights Group Layout"),
        }
    }

    pub fn new(
        render_resources: &RenderAssetStore,
        sampler_cache: &mut SamplerCache,
        placeholders: &PlaceholderTextureIds,
        wgpu_context: &WgpuContext,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let sun = scene_tree::Sun::default();
        let direction_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Direction Buffer"),
            contents: bytemuck::cast_slice(&sun.direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let color_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Color Buffer"),
            contents: bytemuck::cast_slice(&sun.color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let textures = &render_resources.textures;
        let (prefiltered, di, brdf) = (textures.get(placeholders.prefiltered.into()).unwrap(), textures.get(placeholders.di.into()).unwrap(), textures.get(placeholders.brdf.into()).unwrap());
        let default_sampler = sampler_cache.get(&materialfile::Sampler::default(), wgpu_context);
        let bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: direction_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&prefiltered.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&default_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&di.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&default_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&brdf.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&default_sampler),
                },
            ],
            label: Some("Lights Bind Group"),
        });

        Self {
            sun_direction_buffer: direction_buffer,
            sun_color_buffer: color_buffer,
            bind_group,
            curr_prefiltered_render_id: placeholders.prefiltered,
            curr_di_render_id: placeholders.di,
            curr_brdf_render_id: placeholders.brdf,
        }
    }

    pub fn update_sun(&self, sun: &scene_tree::Sun, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.sun_direction_buffer,
            0,
            bytemuck::cast_slice(&sun.direction),
        );
        queue.write_buffer(&self.sun_color_buffer, 0, bytemuck::cast_slice(&sun.color));
    }

    pub fn update_environment_map(
        &mut self,
        wgpu_context: &WgpuContext,
        bind_group_layout: &wgpu::BindGroupLayout,
        prefiltered_view: &wgpu::TextureView,
        prefiltered_sampler: &wgpu::Sampler,
        di_view: &wgpu::TextureView,
        di_sampler: &wgpu::Sampler,
        brdf_view: &wgpu::TextureView,
        brdf_sampler: &wgpu::Sampler,
    ) {
        self.bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.sun_direction_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.sun_color_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&prefiltered_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&prefiltered_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&di_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&di_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&brdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&brdf_sampler),
                },
            ],
            label: Some("Lights Bind Group"),
        });
    }
}

use wgpu::util::DeviceExt as _;

use crate::{fixed_snapshot::PointLightSnapshot, game::scene_tree::Sun};

use super::SunBuffers;

const MAX_POINT_LIGHTS: usize = 64;

pub struct LightsBuffers {
    pub sun: SunBuffers,
    pub environment_map_intensity: wgpu::Buffer,
    pub point_light_count: wgpu::Buffer,
    pub point_light_positions_ranges: wgpu::Buffer,
    pub point_light_colors_intensities: wgpu::Buffer,
}

impl LightsBuffers {
    pub fn new(device: &wgpu::Device) -> Self {
        let sun = SunBuffers::new(device);
        let environment_map_intensity =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Environment Map Intensity Buffer"),
                contents: bytemuck::cast_slice(&[1.0f32]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let point_light_count = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Point Light Count Buffer"),
            contents: bytemuck::cast_slice(&[[0u32, 0u32, 0u32, 0u32]]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let point_light_positions_ranges =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Point Light Positions and Ranges Buffer"),
                contents: bytemuck::cast_slice(&[[0.0f32; 4]; MAX_POINT_LIGHTS]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let point_light_colors_intensities =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Point Light Colors and Intensities Buffer"),
                contents: bytemuck::cast_slice(&[[0.0f32; 4]; MAX_POINT_LIGHTS]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            sun,
            environment_map_intensity,
            point_light_count,
            point_light_positions_ranges,
            point_light_colors_intensities,
        }
    }

    pub fn update_sun(&self, sun: &Sun, queue: &wgpu::Queue) {
        self.sun.update_direction(&sun.direction, queue);
        self.sun.update_color(&sun.color, queue);
    }

    pub fn update_environment_map_intensity(&self, intensity: f32, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.environment_map_intensity,
            0,
            bytemuck::cast_slice(&[intensity]),
        );
    }

    pub fn update_point_lights(&self, point_lights: &[PointLightSnapshot], queue: &wgpu::Queue) {
        let clamped_count = point_lights.len().min(MAX_POINT_LIGHTS);
        let mut point_positions_ranges = [[0.0f32; 4]; MAX_POINT_LIGHTS];
        let mut point_colors_intensities = [[0.0f32; 4]; MAX_POINT_LIGHTS];

        for (idx, light) in point_lights.iter().take(clamped_count).enumerate() {
            point_positions_ranges[idx] = [
                light.position.x,
                light.position.y,
                light.position.z,
                light.range,
            ];
            point_colors_intensities[idx] = [
                light.color[0],
                light.color[1],
                light.color[2],
                light.intensity,
            ];
        }

        queue.write_buffer(
            &self.point_light_count,
            0,
            bytemuck::cast_slice(&[[clamped_count as u32, 0u32, 0u32, 0u32]]),
        );
        queue.write_buffer(
            &self.point_light_positions_ranges,
            0,
            bytemuck::cast_slice(&point_positions_ranges),
        );
        queue.write_buffer(
            &self.point_light_colors_intensities,
            0,
            bytemuck::cast_slice(&point_colors_intensities),
        );
    }
}

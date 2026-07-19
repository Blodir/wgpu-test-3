use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt as _;

use crate::{
    game::scene_tree::Sun,
    host::world::prepare::sun_shadow::{
        SUN_SHADOW_DEFAULT_CASCADE_COUNT, SUN_SHADOW_MAX_CASCADE_COUNT,
    },
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SunShadowUniform {
    pub light_view_proj: [[f32; 16]; SUN_SHADOW_MAX_CASCADE_COUNT],
    pub split_depths: [f32; 4],
    pub cascade_params: [u32; 4],
}

impl Default for SunShadowUniform {
    fn default() -> Self {
        Self {
            light_view_proj: [glam::Mat4::IDENTITY.to_cols_array(); SUN_SHADOW_MAX_CASCADE_COUNT],
            split_depths: [0.0; 4],
            cascade_params: [SUN_SHADOW_DEFAULT_CASCADE_COUNT as u32, 0, 0, 0],
        }
    }
}

pub struct SunBuffers {
    pub direction: wgpu::Buffer,
    pub color: wgpu::Buffer,
    pub shadow: wgpu::Buffer,
}

impl SunBuffers {
    pub fn new(device: &wgpu::Device) -> Self {
        let sun = Sun::default();
        let direction = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Direction Buffer"),
            contents: bytemuck::cast_slice(&sun.direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let color = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Color Buffer"),
            contents: bytemuck::cast_slice(&sun.color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shadow = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Shadow Buffer"),
            contents: bytemuck::bytes_of(&SunShadowUniform::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            direction,
            color,
            shadow,
        }
    }

    pub fn update_direction(&self, direction: &[f32; 3], queue: &wgpu::Queue) {
        queue.write_buffer(&self.direction, 0, bytemuck::cast_slice(direction));
    }

    pub fn update_color(&self, color: &[f32; 3], queue: &wgpu::Queue) {
        queue.write_buffer(&self.color, 0, bytemuck::cast_slice(color));
    }

    pub fn update_shadow(&self, shadow: &SunShadowUniform, queue: &wgpu::Queue) {
        queue.write_buffer(&self.shadow, 0, bytemuck::bytes_of(shadow));
    }
}

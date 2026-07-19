use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt as _;

use crate::{
    fixed_snapshot::PointLightSnapshot,
    game::scene_tree::Sun,
    host::{
        renderer::{
            rw_buffer::{RWBuffer, RWBufferOptions},
            world::{
                hbgi_settings::HbgiSettingsUniform, skinned_instance::SkinnedInstance,
                static_instance::StaticInstance,
            },
        },
        wgpu_context::WgpuContext,
        world::sun_shadow::SunShadowUniform,
    },
};

const MAX_POINT_LIGHTS: usize = 64;
const FULLSCREEN_QUAD_INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub struct CameraBuffers {
    pub view_proj: wgpu::Buffer,
    pub position: wgpu::Buffer,
    pub inverse_view_proj: wgpu::Buffer,
    pub forward: wgpu::Buffer,
    pub view_rotation: wgpu::Buffer,
    pub prev_view_proj: wgpu::Buffer,
}
impl CameraBuffers {
    fn padded_view_rotation(right: Vec3, up: Vec3, look: Vec3) -> [[f32; 4]; 3] {
        [
            [right.x, right.y, right.z, 0.0],
            [up.x, up.y, up.z, 0.0],
            [look.x, look.y, look.z, 0.0],
        ]
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let position = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Position Buffer"),
            contents: bytemuck::cast_slice(&Vec3::ZERO.to_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let inverse_view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Inverse View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let forward = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Forward Buffer"),
            contents: bytemuck::cast_slice(&[0.0, 0.0, -1.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let view_rotation = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera View Rotation Buffer"),
            contents: bytemuck::cast_slice(&Self::padded_view_rotation(Vec3::X, Vec3::Y, -Vec3::Z)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let prev_view_proj = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Previous View Projection Buffer"),
            contents: bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        CameraBuffers {
            view_proj,
            position,
            inverse_view_proj,
            forward,
            view_rotation,
            prev_view_proj,
        }
    }

    pub fn update(
        &self,
        view_proj: &[f32; 16],
        position: &[f32; 3],
        inverse_view_proj: &[f32; 16],
        forward: &[f32; 3],
        view_rotation: &[[f32; 4]; 3],
        prev_view_proj: &[f32; 16],
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(&self.view_proj, 0, bytemuck::cast_slice(view_proj));
        queue.write_buffer(&self.position, 0, bytemuck::cast_slice(position));
        queue.write_buffer(
            &self.inverse_view_proj,
            0,
            bytemuck::cast_slice(inverse_view_proj),
        );
        queue.write_buffer(&self.forward, 0, bytemuck::cast_slice(forward));
        queue.write_buffer(&self.view_rotation, 0, bytemuck::cast_slice(view_rotation));
        queue.write_buffer(
            &self.prev_view_proj,
            0,
            bytemuck::cast_slice(prev_view_proj),
        );
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

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BoneMat34 {
    pub mat: [[f32; 4]; 3],
}
impl Default for BoneMat34 {
    fn default() -> Self {
        Self {
            mat: [
                [1f32, 0f32, 0f32, 0f32],
                [0f32, 1f32, 0f32, 0f32],
                [0f32, 0f32, 1f32, 0f32],
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct HbgiReprojectUniform {
    prev_inverse_view_proj: [[f32; 4]; 4],
}

pub(crate) struct Buffers {
    pub bones: RWBuffer,
    pub camera: CameraBuffers,
    pub hbgi_settings: wgpu::Buffer,
    pub hbgi_reproject_settings: wgpu::Buffer,
    pub fullscreen_quad_indices: wgpu::Buffer,
    pub skinned_instances: RWBuffer,
    pub static_instances: wgpu::Buffer,
    pub lights: LightsBuffers,
}
impl Buffers {
    pub fn new(wgpu_context: &WgpuContext, hbgi_settings: Option<&HbgiSettingsUniform>) -> Self {
        let device = &wgpu_context.device;
        let storage_usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

        let mut bones = RWBuffer::new(
            RWBufferOptions {
                label: Some("Bones SSBO".to_string()),
                usage: storage_usage,
            },
            wgpu_context,
        );
        let initial_bones = vec![BoneMat34::default(); 2048];
        let initial_bones_bytes = bytemuck::cast_slice(initial_bones.as_slice());
        bones.write(initial_bones_bytes, wgpu_context);
        bones.swap();
        bones.write(initial_bones_bytes, wgpu_context);
        bones.swap();

        let camera = CameraBuffers::new(device);

        let hbgi_settings = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HBGI Settings Buffer"),
            contents: bytemuck::bytes_of(
                &hbgi_settings
                    .copied()
                    .unwrap_or_else(HbgiSettingsUniform::default),
            ),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let hbgi_reproject_settings =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HBGI Reproject Settings Buffer"),
                contents: bytemuck::bytes_of(&HbgiReprojectUniform {
                    prev_inverse_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let fullscreen_quad_indices =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Fullscreen Quad Index Buffer"),
                contents: bytemuck::cast_slice(FULLSCREEN_QUAD_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let mut skinned_instances = RWBuffer::new(
            RWBufferOptions {
                label: Some("Skinned Instance Buffer".to_string()),
                usage: storage_usage,
            },
            wgpu_context,
        );
        let initial_skinned_instances = [SkinnedInstance::default()];
        let initial_skinned_instance_bytes = bytemuck::cast_slice(&initial_skinned_instances);
        skinned_instances.write(initial_skinned_instance_bytes, wgpu_context);
        skinned_instances.swap();
        skinned_instances.write(initial_skinned_instance_bytes, wgpu_context);
        skinned_instances.swap();

        let static_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance buffer"),
            contents: bytemuck::cast_slice(&[StaticInstance::default()]),
            usage: storage_usage,
        });

        let lights = LightsBuffers::new(device);

        Self {
            bones,
            camera,
            hbgi_settings,
            hbgi_reproject_settings,
            fullscreen_quad_indices,
            skinned_instances,
            static_instances,
            lights,
        }
    }

    pub fn update_hbgi_settings(&self, hbgi_settings: &HbgiSettingsUniform, queue: &wgpu::Queue) {
        queue.write_buffer(&self.hbgi_settings, 0, bytemuck::bytes_of(hbgi_settings));
    }

    pub fn update_hbgi_reproject_settings(
        &self,
        prev_inverse_view_proj: &Mat4,
        queue: &wgpu::Queue,
    ) {
        let uniform = HbgiReprojectUniform {
            prev_inverse_view_proj: prev_inverse_view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(
            &self.hbgi_reproject_settings,
            0,
            bytemuck::bytes_of(&uniform),
        );
    }
}

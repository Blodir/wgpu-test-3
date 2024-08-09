use cgmath::{EuclideanSpace, Point3, Rotation3, SquareMatrix, Transform, Vector4};
use wgpu::{util::DeviceExt, BindGroup, BindGroupLayout};

use crate::renderer::wgpu_context::{WgpuContext, OPENGL_TO_WGPU_MATRIX};

pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub rot_x: cgmath::Deg<f32>,
    pub rot_y: cgmath::Deg<f32>,
}

impl Camera {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let eye: cgmath::Point3<f32> = (0.0, 0.0, 2.0).into();
        let target: cgmath::Point3<f32> = (0.0, 0.0, 0.0).into();
        let up: cgmath::Vector3<f32> = cgmath::Vector3::unit_y();
        let aspect = wgpu_context.surface_config.width as f32 / wgpu_context.surface_config.height as f32;
        let fovy = 45.0f32;
        let znear = 0.1f32;
        let zfar = 100.0f32;
        let rot_x = cgmath::Deg(0f32);
        let rot_y = cgmath::Deg(0f32);

        Self {
            eye, target, up, aspect, fovy, znear, zfar, rot_x, rot_y
        }
    }
}

pub struct CameraBindGroups {
    pub camera_bind_group: BindGroup,
    pub camera_bind_group_layout: BindGroupLayout,
}

impl CameraBindGroups {
    pub fn new(camera: &Camera, wgpu_context: &WgpuContext) -> CameraBindGroups {
        let rot = cgmath::Quaternion::from_angle_y(camera.rot_x) * cgmath::Quaternion::from_angle_x(camera.rot_y);
        let eye_rotated = cgmath::Matrix4::from(rot).transform_point(camera.eye);
        let view = cgmath::Matrix4::look_at_rh(eye_rotated, camera.target, camera.up);
        let proj = cgmath::perspective(cgmath::Deg(camera.fovy), camera.aspect, camera.znear, camera.zfar);
        let view_proj = OPENGL_TO_WGPU_MATRIX * proj * view;
        let view_proj_m: [[f32; 4]; 4] = view_proj.into();

        let camera_buffer = wgpu_context.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[view_proj_m]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let camera_bind_group_layout = wgpu_context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_bind_group_layout"),
        });

        let camera_bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        Self { camera_bind_group, camera_bind_group_layout }
    }
}


use cgmath::{Quaternion, Rotation3};
use wgpu::util::DeviceExt;

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

pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    position: [f32; 3],
}

pub struct CameraBinding {
    pub bind_group: wgpu::BindGroup,
    view_proj_buffer: wgpu::Buffer,
    position_buffer: wgpu::Buffer,
}

impl Camera {
    pub fn new(surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let eye: cgmath::Point3<f32> = (0.0, 0.0, 2.0).into();
        let target: cgmath::Point3<f32> = (0.0, 0.0, 0.0).into();
        let up: cgmath::Vector3<f32> = cgmath::Vector3::unit_y();
        let aspect = surface_config.width as f32 / surface_config.height as f32;
        let fovy = 45.0f32;
        let znear = 0.1f32;
        let zfar = 100.0f32;
        let rot_x = cgmath::Deg(0f32);
        let rot_y = cgmath::Deg(0f32);

        Self {
            eye, target, up, aspect, fovy, znear, zfar, rot_x, rot_y
        }
    }

    pub fn to_camera_uniform(&self) -> CameraUniform {
        let rot =
              Quaternion::from_angle_y(self.rot_x)
            * Quaternion::from_angle_x(self.rot_y);
        let eye_rotated = cgmath::Transform::transform_point(&cgmath::Matrix4::from(rot), self.eye);
        let view = cgmath::Matrix4::look_at_rh(eye_rotated, self.target, self.up);
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        let view_proj = super::wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;
        CameraUniform {
            view_proj: view_proj.into(), position: eye_rotated.into()
        }
    }
}

impl CameraUniform {
    pub fn default(surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let eye: cgmath::Point3<f32> = (0.0, 0.0, 2.0).into();
        let target: cgmath::Point3<f32> = (0.0, 0.0, 0.0).into();
        let up: cgmath::Vector3<f32> = cgmath::Vector3::unit_y();
        let aspect = surface_config.width as f32 / surface_config.height as f32;
        let fovy = 45.0f32;
        let znear = 0.1f32;
        let zfar = 100.0f32;
        let rot_x = cgmath::Deg(0f32);
        let rot_y = cgmath::Deg(0f32);
        let rot =
              Quaternion::from_angle_y(rot_x)
            * Quaternion::from_angle_x(rot_y);
        let eye_rotated = cgmath::Transform::transform_point(&cgmath::Matrix4::from(rot), eye);
        let view = cgmath::Matrix4::look_at_rh(eye_rotated, target, up);
        let proj = cgmath::perspective(cgmath::Deg(fovy), aspect, znear, zfar);
        let view_proj = super::wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;
        Self {
            view_proj: view_proj.into(), position: eye_rotated.into()
        }
    }

    pub fn upload(&self, device: &wgpu::Device, bind_group_layout: &wgpu::BindGroupLayout) -> CameraBinding {
        let view_proj_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("View Projection Buffer"),
                contents: bytemuck::cast_slice(&self.view_proj),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let position_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Position Buffer"),
                contents: bytemuck::cast_slice(&self.position),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_proj_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: position_buffer.as_entire_binding(),
                },
            ],
            label: Some("Camera Bind Group"),
        });

        CameraBinding { bind_group, view_proj_buffer, position_buffer }
    }

    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
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
                },
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
            ],
            label: Some("Camera Bind Group Layout")
        }
    }
}

impl CameraBinding {
    pub fn update(&self, camera: &CameraUniform, queue: &wgpu::Queue) {
        queue.write_buffer(&self.view_proj_buffer, 0, bytemuck::cast_slice(&camera.view_proj));
        queue.write_buffer(&self.position_buffer, 0, bytemuck::cast_slice(&camera.position));
    }
}


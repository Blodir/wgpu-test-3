use glam::{Mat3, Mat4, Quat, Vec3, Vec4};

use crate::{render_snapshot::CameraSnapshot, renderer::wgpu_context, resource_system::file_formats::modelfile::Aabb};

fn look_at_rotation(eye: Vec3, target: Vec3, world_up: Vec3) -> Quat {
    let forward = (target - eye).normalize();
    let up = (world_up - forward * world_up.dot(forward)).normalize();
    let right = forward.cross(up);

    // Camera looks down -Z
    Quat::from_mat3(&Mat3::from_cols(
        right,
        up,
        -forward,
    ))
}

#[derive(Clone, Copy)]
pub struct Plane {
    pub n: Vec3, // inward, normalized
    pub d: f32,  // plane eq: n·x + d = 0
}

#[derive(Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6], // left, right, bottom, top, near, far
}

fn normalize_plane(p: Vec4) -> Plane {
    let n = Vec3::new(p.x, p.y, p.z);
    let inv_len = 1.0 / n.length();
    Plane {
        n: n * inv_len,
        d: p.w * inv_len,
    }
}

#[derive(Clone)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3, // temporary orbit camera
    pub up: Vec3,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub aspect: f32, // needed for frustum culling
    pub rot_x: f32, // deg, temporary orbit camera
    pub rot_y: f32, // deg
}
impl Default for Camera {
    fn default() -> Self {
        let eye: Vec3 = (0.0, 0.0, 100.0).into();
        let target: Vec3 = (0.0, 0.0, 0.0).into();
        let up: Vec3 = Vec3::Y;
        let fovy = 45.0f32;
        let znear = 0.1f32;
        let zfar = 1000.0f32;
        let rot_x = 0f32;
        let rot_y = 0f32;
        let aspect = 16.0 / 9.0;

        Self {
            eye,
            target,
            up,
            fovy,
            znear,
            zfar,
            rot_x,
            rot_y,
            aspect,
        }
    }
}
impl Camera {
    pub fn build_snapshot(&self) -> CameraSnapshot {
        let rot = Quat::from_rotation_y((self.rot_x).to_radians())
            * Quat::from_rotation_x((self.rot_y).to_radians());
        let eye_rotated: Vec3 = rot * self.eye;

        CameraSnapshot {
            position: eye_rotated,
            rotation: look_at_rotation(eye_rotated, self.target, self.up),
            fovy: self.fovy,
            znear: self.znear,
            zfar: self.zfar,
        }
    }

    pub fn build_frustum(
        &self,
    ) -> Frustum {
        let rot = Quat::from_rotation_y((self.rot_x).to_radians())
            * Quat::from_rotation_x((self.rot_y).to_radians());
        let position: Vec3 = rot * self.eye;
        let rotation = look_at_rotation(position, self.target, self.up);
        let rot_inv = rotation.conjugate();
        let view = Mat4::from_rotation_translation(rot_inv, -(rot_inv * position));
        let proj = Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar);
        let m: Mat4 = wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;

        // extract planes (row-major conceptual form)
        let planes = [
            normalize_plane(m.row(3) + m.row(0)), // left
            normalize_plane(m.row(3) - m.row(0)), // right
            normalize_plane(m.row(3) + m.row(1)), // bottom
            normalize_plane(m.row(3) - m.row(1)), // top
            normalize_plane(m.row(3) + m.row(2)), // near
            normalize_plane(m.row(3) - m.row(2)), // far
        ];

        Frustum { planes }
    }
}

#[inline]
fn transform_aabb(aabb: &Aabb, model: &Mat4) -> (Vec3, Vec3) {
    let min = Vec3::from(aabb.min);
    let max = Vec3::from(aabb.max);

    let local_center = (min + max) * 0.5;
    let local_extents = (max - min) * 0.5;

    let world_center = model.transform_point3(local_center);

    let m = model.to_scale_rotation_translation().0;
    let r = Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        model.to_scale_rotation_translation().1,
        Vec3::ZERO,
    );

    let abs_rot = Mat3::from_mat4(r).abs();

    let world_extents = abs_rot * (local_extents * m);

    let world_min = world_center - world_extents;
    let world_max = world_center + world_extents;

    (world_min, world_max)
}

#[inline]
pub fn frustum_intersects_aabb_world(
    frustum: &Frustum,
    aabb: &Aabb,
    model: &Mat4,
) -> bool {
    let (min, max) = transform_aabb(aabb, model);

    for plane in &frustum.planes {
        let p = Vec3::new(
            if plane.n.x >= 0.0 { max.x } else { min.x },
            if plane.n.y >= 0.0 { max.y } else { min.y },
            if plane.n.z >= 0.0 { max.z } else { min.z },
        );

        if plane.n.dot(p) + plane.d < 0.0 {
            return false;
        }
    }

    true
}

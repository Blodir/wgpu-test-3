use glam::{Mat3, Mat4, Quat, Vec3, Vec4};

use crate::render_snapshot::CameraSnapshot;

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
        let zfar = 100.0f32;
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
}

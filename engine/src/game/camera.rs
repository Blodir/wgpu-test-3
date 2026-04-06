use glam::{Mat3, Mat4, Quat, Vec3, Vec4};

use crate::{
    host::{
        assets::io::asset_formats::modelfile::Aabb,
        world::sun_shadow::{
            SUN_SHADOW_DEFAULT_CASCADE_COUNT, SUN_SHADOW_DEFAULT_CASCADE_SPLIT_RATIOS,
            SUN_SHADOW_MAX_CASCADE_COUNT,
        },
    },
    var_snapshot::CameraSnapshot,
};

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
    pub position: Vec3,
    pub rotation: Quat,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub sun_shadow_cascade_count: usize,
    pub sun_shadow_cascade_split_ratios: [f32; SUN_SHADOW_MAX_CASCADE_COUNT],
    pub aspect: f32, // needed for frustum culling
}
impl Default for Camera {
    fn default() -> Self {
        let position: Vec3 = (0.0, 0.0, 100.0).into();
        let rotation = Quat::IDENTITY;
        let fovy = 60.0f32.to_radians();
        let znear = 0.1f32;
        let zfar = 1000f32;
        let sun_shadow_cascade_count = SUN_SHADOW_DEFAULT_CASCADE_COUNT;
        let sun_shadow_cascade_split_ratios = SUN_SHADOW_DEFAULT_CASCADE_SPLIT_RATIOS;
        let aspect = 16.0 / 9.0;

        Self {
            position,
            rotation,
            fovy,
            znear,
            zfar,
            sun_shadow_cascade_count,
            sun_shadow_cascade_split_ratios,
            aspect,
        }
    }
}
impl Camera {
    pub fn build_snapshot(&self) -> CameraSnapshot {
        CameraSnapshot {
            position: self.position,
            rotation: self.rotation,
            fovy: self.fovy,
            znear: self.znear,
            zfar: self.zfar,
            sun_shadow_cascade_count: self.sun_shadow_cascade_count,
            sun_shadow_cascade_split_ratios: self.sun_shadow_cascade_split_ratios,
        }
    }

    pub fn build_frustum(&self) -> Frustum {
        let rot_inv = self.rotation.conjugate();
        let view = Mat4::from_rotation_translation(rot_inv, -(rot_inv * self.position));
        let proj = Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar);
        let m: Mat4 = proj * view;

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
pub fn frustum_intersects_aabb_world(frustum: &Frustum, aabb: &Aabb, model: &Mat4) -> bool {
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

use std::time::Instant;

use glam::{Mat4, Quat, Vec3};

use crate::{
    host::{
        utils::lerpf32,
        world::{gpu_context::CameraBuffers, prepare::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT},
    },
    var_snapshot::CameraSnapshotPair,
};

#[derive(Clone, Copy)]
pub struct InterpolatedCameraState {
    pub position: Vec3,
    pub rotation: Quat,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub sun_shadow_cascade_count: usize,
    pub sun_shadow_cascade_split_ratios: [f32; SUN_SHADOW_MAX_CASCADE_COUNT],
    pub aspect: f32,
}

#[derive(Clone, Copy)]
pub struct PreparedCamera {
    pub state: InterpolatedCameraState,
    pub view_proj: Mat4,
}

pub fn interpolate_camera_state(
    camera_pair: &CameraSnapshotPair,
    now: Instant,
    surface_config: &wgpu::SurfaceConfiguration,
) -> InterpolatedCameraState {
    let prev = &camera_pair.prev;
    let curr = &camera_pair.curr;
    let elapsed = now.saturating_duration_since(camera_pair.curr_timestamp);
    let interval = camera_pair
        .curr_timestamp
        .saturating_duration_since(camera_pair.prev_timestamp);
    let t = if interval.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f32() / interval.as_secs_f32()).clamp(0.0, 1.0)
    };

    let aspect = if surface_config.height > 0 {
        surface_config.width as f32 / surface_config.height as f32
    } else {
        16.0 / 9.0
    };

    InterpolatedCameraState {
        position: prev.position.lerp(curr.position, t),
        rotation: prev.rotation.slerp(curr.rotation, t),
        fovy: lerpf32(prev.fovy, curr.fovy, t),
        znear: lerpf32(prev.znear, curr.znear, t),
        zfar: lerpf32(prev.zfar, curr.zfar, t),
        sun_shadow_cascade_count: curr.sun_shadow_cascade_count,
        sun_shadow_cascade_split_ratios: curr.sun_shadow_cascade_split_ratios,
        aspect,
    }
}

pub fn build_camera_frustum_corners(
    camera: &InterpolatedCameraState,
    slice_near: f32,
    slice_far: f32,
) -> [Vec3; 8] {
    let forward = camera.rotation * -Vec3::Z;
    let right = camera.rotation * Vec3::X;
    let up = camera.rotation * Vec3::Y;

    let near = slice_near.clamp(camera.znear, camera.zfar);
    let far = slice_far.clamp(camera.znear, camera.zfar);
    let (near, far) = if near <= far {
        (near, far)
    } else {
        (far, near)
    };
    let tan_half_fovy = (camera.fovy * 0.5).tan();

    let near_h = tan_half_fovy * near;
    let near_w = near_h * camera.aspect;
    let far_h = tan_half_fovy * far;
    let far_w = far_h * camera.aspect;

    let near_center = camera.position + forward * near;
    let far_center = camera.position + forward * far;

    [
        near_center - right * near_w - up * near_h,
        near_center + right * near_w - up * near_h,
        near_center + right * near_w + up * near_h,
        near_center - right * near_w + up * near_h,
        far_center - right * far_w - up * far_h,
        far_center + right * far_w - up * far_h,
        far_center + right * far_w + up * far_h,
        far_center - right * far_w + up * far_h,
    ]
}

pub fn prepare_camera(
    camera: &CameraBuffers,
    camera_pair: &CameraSnapshotPair,
    now: Instant,
    prev_view_proj: Option<&Mat4>,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
) -> PreparedCamera {
    let state = interpolate_camera_state(camera_pair, now, surface_config);

    let rot_inv = state.rotation.conjugate();
    let view = Mat4::from_rotation_translation(rot_inv, -(rot_inv * state.position));
    let proj = Mat4::perspective_rh(state.fovy, state.aspect, state.znear, state.zfar);
    let view_proj: Mat4 = proj * view;
    let right = state.rotation * Vec3::X;
    let up = state.rotation * Vec3::Y;
    let forward = state.rotation * -Vec3::Z;
    let view_rotation = [
        [right.x, right.y, right.z, 0.0],
        [up.x, up.y, up.z, 0.0],
        [forward.x, forward.y, forward.z, 0.0],
    ];
    let inverse_view_proj = view_proj.inverse();
    let prev_view_proj = prev_view_proj.unwrap_or(&view_proj);

    camera.update(
        &view_proj.to_cols_array(),
        &state.position.to_array(),
        &inverse_view_proj.to_cols_array(),
        &forward.to_array(),
        &view_rotation,
        &prev_view_proj.to_cols_array(),
        queue,
    );

    PreparedCamera { state, view_proj }
}

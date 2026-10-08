use glam::{Mat4, Quat, Vec2, Vec3};

use crate::{
    host::{
        utils::lerpf32,
        world::{
            gpu_context::r2_quasirandom_sequence,
            prepare::sun_shadow::SUN_SHADOW_MAX_CASCADE_COUNT,
        },
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
    pub inverse_view_proj: Mat4,
    pub view_rotation: [[f32; 4]; 3],
    pub unjittered_view_proj: Mat4,
}

fn reverse_z_perspective_rh(fovy: f32, aspect: f32, znear: f32, zfar: f32) -> Mat4 {
    debug_assert!(znear > 0.0);
    debug_assert!(zfar > znear);

    let tan_half_fovy = (fovy * 0.5).tan();
    let h = 1.0 / tan_half_fovy;
    let w = h / aspect;
    let r = znear / (zfar - znear);

    Mat4::from_cols_array(&[
        w,
        0.0,
        0.0,
        0.0,
        0.0,
        h,
        0.0,
        0.0,
        0.0,
        0.0,
        r,
        -1.0,
        0.0,
        0.0,
        r * zfar,
        0.0,
    ])
}

fn camera_jitter_ndc(frame_idx: u32, surface_config: &wgpu::SurfaceConfiguration) -> Vec2 {
    let (x, y) = r2_quasirandom_sequence(frame_idx);
    let jitter = Vec2::new(x - 0.5, y - 0.5);
    let resolution = Vec2::new(
        surface_config.width.max(1) as f32,
        surface_config.height.max(1) as f32,
    );
    jitter * (2.0 / resolution)
}

fn interpolate_camera_state(
    camera_pair: &CameraSnapshotPair,
    t: f32,
    surface_config: &wgpu::SurfaceConfiguration,
) -> InterpolatedCameraState {
    let prev = &camera_pair.prev;
    let curr = &camera_pair.curr;

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

pub fn prepare_camera(
    camera_pair: &CameraSnapshotPair,
    t: f32,
    frame_idx: u32,
    surface_config: &wgpu::SurfaceConfiguration,
) -> PreparedCamera {
    let state = interpolate_camera_state(camera_pair, t, surface_config);

    let rot_inv = state.rotation.conjugate();
    let view = Mat4::from_rotation_translation(rot_inv, -(rot_inv * state.position));
    let unjittered_proj =
        reverse_z_perspective_rh(state.fovy, state.aspect, state.znear, state.zfar);
    let unjittered_view_proj = unjittered_proj * view;
    let mut proj = unjittered_proj;
    let jitter_ndc = camera_jitter_ndc(frame_idx, surface_config);
    proj.z_axis.x -= jitter_ndc.x;
    proj.z_axis.y -= jitter_ndc.y;
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

    PreparedCamera {
        state,
        view_proj,
        inverse_view_proj,
        view_rotation,
        unjittered_view_proj,
    }
}

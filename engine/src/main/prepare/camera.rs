use glam::{Mat3, Mat4, Vec4};

use crate::{render_snapshot::SnapshotGuard, main::{bindgroups::camera::CameraBinding, utils::lerpf32, wgpu_context}};

pub fn prepare_camera(
    camera: &mut CameraBinding,
    snaps: &SnapshotGuard,
    t: f32,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
) {
    let prev = &snaps.prev.camera;
    let curr = &snaps.curr.camera;

    let position = prev.position.lerp(curr.position, t);
    let rotation = prev.rotation.slerp(curr.rotation, t);
    let fovy = lerpf32(prev.fovy, curr.fovy, t);
    let znear = lerpf32(prev.znear, curr.znear, t);
    let zfar = lerpf32(prev.zfar, curr.zfar, t);

    let rot_inv = rotation.conjugate();
    let view = Mat4::from_rotation_translation(rot_inv, -(rot_inv * position));
    let aspect = if surface_config.height > 0 {
        surface_config.width as f32 / surface_config.height as f32
    } else {
        16.0 / 9.0
    };
    let proj = Mat4::perspective_rh(fovy, aspect, znear, zfar);
    let view_proj: Mat4 = wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;
    let m3 = Mat3::from_mat4(view_proj).inverse();
    let inverse_view_proj_rot = Mat4::from_cols(
        Vec4::new(m3.x_axis.x, m3.x_axis.y, m3.x_axis.z, 0.0),
        Vec4::new(m3.y_axis.x, m3.y_axis.y, m3.y_axis.z, 0.0),
        Vec4::new(m3.z_axis.x, m3.z_axis.y, m3.z_axis.z, 0.0),
        Vec4::ZERO,
    );

    camera.update(&view_proj.to_cols_array(), &position.to_array(), &inverse_view_proj_rot.to_cols_array(), queue);
}

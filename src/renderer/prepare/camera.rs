use crate::{render_snapshot::SnapshotGuard, renderer::{bindgroups::camera::CameraBinding, utils::lerpf32}, sim::scene_tree::Camera};

pub fn prepare_camera(
    camera: &mut CameraBinding,
    snaps: &SnapshotGuard,
    t: f32,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
) {
    let prev = &snaps.prev.camera;
    let curr = &snaps.curr.camera;
    let interpolated_camera = Camera {
        eye: prev.eye.lerp(curr.eye, t),
        target: prev.target.lerp(curr.target, t),
        up: prev.up.lerp(curr.up, t),
        fovy: lerpf32(prev.fovy, curr.fovy, t),
        znear: lerpf32(prev.znear, curr.znear, t),
        zfar: lerpf32(prev.zfar, curr.zfar, t),
        rot_x: lerpf32(prev.rot_x, curr.rot_x, t),
        rot_y: lerpf32(prev.rot_y, curr.rot_y, t),
    };
    camera.update(&interpolated_camera, queue, surface_config);
}

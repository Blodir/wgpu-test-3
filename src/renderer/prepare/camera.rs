use crate::{render_snapshot::{CameraSnapshot, SnapshotGuard}, renderer::{bindgroups::camera::CameraBinding, utils::lerpf32}};

pub fn prepare_camera(
    camera: &mut CameraBinding,
    snaps: &SnapshotGuard,
    t: f32,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
) {
    let prev = &snaps.prev.camera;
    let curr = &snaps.curr.camera;
    let interpolated_camera = CameraSnapshot {
        position: prev.position.lerp(curr.position, t),
        rotation: prev.rotation.slerp(curr.rotation, t),
        fovy: lerpf32(prev.fovy, curr.fovy, t),
        znear: lerpf32(prev.znear, curr.znear, t),
        zfar: lerpf32(prev.zfar, curr.zfar, t),
    };
    camera.update(&interpolated_camera, queue, surface_config);
}

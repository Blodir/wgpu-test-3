use glam::{Mat4, Vec3};

use crate::host::world::prepare::camera::{build_camera_frustum_corners, PreparedCamera};

use super::super::bindgroups::lights::LightsBinding;

// TEMP: tighter single-slice shadow coverage for better texel density.
const SUN_SHADOW_SLICE_FAR: f32 = 200.0;
// TEMP: conservative padding so casters near the view frustum do not get clipped out.
const SUN_SHADOW_XY_MARGIN: f32 = 40.0;
const SUN_SHADOW_Z_MARGIN: f32 = 200.0;
// Keep in sync with SunShadowTexture::SHADOW_MAP_SIZE.
const SUN_SHADOW_MAP_SIZE: f32 = 2048.0;

fn build_sun_shadow_light_view_proj(camera: &PreparedCamera, sun_direction: Vec3) -> Mat4 {
    // build transform matrix from sun direction
    let light_rot = Mat4::look_to_rh(Vec3::ZERO, sun_direction, Vec3::Y);

    // calculate camera frustum bounds relative to sun dir
    let shadow_slice_far = camera.state.zfar.min(SUN_SHADOW_SLICE_FAR);
    let frustum_corners_ws =
        build_camera_frustum_corners(&camera.state, camera.state.znear, shadow_slice_far);

    // transform bounds with light_rot and get the center of the AABB (not the centroid of the actual bounding box)
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for corner in &frustum_corners_ws {
        let p = light_rot.transform_point3(*corner);
        min = min.min(p);
        max = max.max(p);
    }

    let center_light = (min + max) * 0.5;
    let center_world = light_rot.inverse().transform_point3(center_light);

    // construct view matrix from aabb center and sun direction
    let view = light_rot * Mat4::from_translation(-center_world);

    // recompute AABB bounds in view space
    let mut min_v = Vec3::splat(f32::INFINITY);
    let mut max_v = Vec3::splat(f32::NEG_INFINITY);

    for corner in &frustum_corners_ws {
        let p = view.transform_point3(*corner);
        min_v = min_v.min(p);
        max_v = max_v.max(p);
    }

    // Expand bounds to keep near-frustum casters and reduce cut-in/out at the edges.
    min_v.x -= SUN_SHADOW_XY_MARGIN;
    max_v.x += SUN_SHADOW_XY_MARGIN;
    min_v.y -= SUN_SHADOW_XY_MARGIN;
    max_v.y += SUN_SHADOW_XY_MARGIN;
    min_v.z -= SUN_SHADOW_Z_MARGIN;
    max_v.z += SUN_SHADOW_Z_MARGIN;

    // Stabilize shadow projection by snapping XY center to shadow-map texels.
    let extent_x = (max_v.x - min_v.x).max(0.0001);
    let extent_y = (max_v.y - min_v.y).max(0.0001);
    let texel_world_x = extent_x / SUN_SHADOW_MAP_SIZE;
    let texel_world_y = extent_y / SUN_SHADOW_MAP_SIZE;
    let mut center_v = (min_v + max_v) * 0.5;

    // The center of the shadowmap is snapped to integer texels, so that it doesn't move sub-texel amounts frame-by-frame
    center_v.x = (center_v.x / texel_world_x).floor() * texel_world_x;
    center_v.y = (center_v.y / texel_world_y).floor() * texel_world_y;
    min_v.x = center_v.x - extent_x * 0.5;
    max_v.x = center_v.x + extent_x * 0.5;
    min_v.y = center_v.y - extent_y * 0.5;
    max_v.y = center_v.y + extent_y * 0.5;

    let proj = Mat4::orthographic_rh(min_v.x, max_v.x, min_v.y, max_v.y, min_v.z, max_v.z);

    // mult together to get view_proj matrix
    let view_proj: Mat4 = proj * view;
    view_proj
}

pub fn prepare_sun_shadow_light_view_proj(
    camera: &PreparedCamera,
    sun_direction: [f32; 3],
    lights_binding: &LightsBinding,
    queue: &wgpu::Queue,
) {
    let sun_shadow_light_view_proj =
        build_sun_shadow_light_view_proj(camera, Vec3::from_slice(&sun_direction));
    lights_binding
        .update_sun_shadow_light_view_proj(&sun_shadow_light_view_proj.to_cols_array(), queue);
}

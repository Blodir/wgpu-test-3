use std::array;

use glam::{Mat4, Vec3};

use crate::host::world::prepare::camera::{build_camera_frustum_corners, PreparedCamera};
use crate::host::world::sun_shadow::{
    PreparedSunShadow, PreparedSunShadowCascade, SunShadowUniform, SUN_SHADOW_MAP_SIZE,
    SUN_SHADOW_MAX_CASCADE_COUNT, SUN_SHADOW_MAX_DISTANCE,
};

use super::super::bindgroups::lights::LightsBinding;

// TEMP: conservative padding so casters near the view frustum do not get clipped out.
const SUN_SHADOW_XY_MARGIN: f32 = 40.0;
const SUN_SHADOW_Z_MARGIN: f32 = 200.0;

fn select_light_up_axis(sun_direction: Vec3) -> Vec3 {
    if sun_direction.dot(Vec3::Y).abs() > 0.98 {
        Vec3::X
    } else {
        Vec3::Y
    }
}

fn build_sun_shadow_light_view_proj(
    camera: &PreparedCamera,
    sun_direction: Vec3,
    slice_near: f32,
    slice_far: f32,
) -> Mat4 {
    // build transform matrix from sun direction
    let light_rot = Mat4::look_to_rh(
        Vec3::ZERO,
        sun_direction,
        select_light_up_axis(sun_direction),
    );

    // calculate camera frustum bounds relative to sun dir
    let frustum_corners_ws = build_camera_frustum_corners(&camera.state, slice_near, slice_far);

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
    let texel_world_x = extent_x / SUN_SHADOW_MAP_SIZE as f32;
    let texel_world_y = extent_y / SUN_SHADOW_MAP_SIZE as f32;
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
    proj * view
}

pub fn prepare_sun_shadow(
    camera: &PreparedCamera,
    sun_direction: [f32; 3],
    lights_binding: &LightsBinding,
    queue: &wgpu::Queue,
) -> PreparedSunShadow {
    let sun_direction = Vec3::from_slice(&sun_direction);
    let shadow_distance = camera.state.zfar.min(SUN_SHADOW_MAX_DISTANCE);
    let near = camera.state.znear.min(shadow_distance);
    let cascade_count = camera
        .state
        .sun_shadow_cascade_count
        .clamp(1, SUN_SHADOW_MAX_CASCADE_COUNT);
    let split_ratios = camera.state.sun_shadow_cascade_split_ratios;
    let cascades = array::from_fn(|idx| {
        let active_idx = idx.min(cascade_count - 1);
        let prev_ratio = if active_idx == 0 {
            0.0
        } else {
            split_ratios[active_idx - 1]
        };
        let far_ratio = split_ratios[active_idx];
        let slice_near = near + (shadow_distance - near) * prev_ratio;
        let slice_far = near + (shadow_distance - near) * far_ratio;
        let light_view_proj =
            build_sun_shadow_light_view_proj(camera, sun_direction, slice_near, slice_far);
        PreparedSunShadowCascade {
            light_view_proj: light_view_proj.to_cols_array(),
            split_depth: slice_far,
        }
    });
    let uniform = SunShadowUniform {
        light_view_proj: array::from_fn(|idx| cascades[idx].light_view_proj),
        split_depths: array::from_fn(|idx| cascades[idx].split_depth),
        cascade_params: [cascade_count as u32, 0, 0, 0],
        ..Default::default()
    };
    lights_binding.update_sun_shadow(&uniform, queue);

    PreparedSunShadow {
        cascades,
        cascade_count,
    }
}

use std::f32;

use wgpu_test_3::resource_system::file_formats::modelfile;

// TODO: take animations into account
pub fn calculate_aabb(positions: &Vec<[f32; 3]>) -> modelfile::Aabb {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut min_z = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut max_z = f32::MIN;
    for position in positions {
        min_x = min_x.min(position[0]);
        max_x = max_x.max(position[0]);
        min_y = min_y.min(position[1]);
        max_y = max_y.max(position[1]);
        min_z = min_z.min(position[2]);
        max_z = max_z.max(position[2]);
    }
    modelfile::Aabb { min: [min_x, min_y, min_z], max: [max_x, max_y, max_z] }
}

pub fn fold_aabb(aabbs: &Vec<modelfile::Aabb>) -> modelfile::Aabb {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut min_z = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut max_z = f32::MIN;
    for aabb in aabbs {
        min_x = min_x.min(aabb.min[0]);
        max_x = max_x.max(aabb.max[0]);
        min_y = min_y.min(aabb.min[1]);
        max_y = max_y.max(aabb.max[1]);
        min_z = min_z.min(aabb.min[2]);
        max_z = max_z.max(aabb.max[2]);
    }
    modelfile::Aabb { min: [min_x, min_y, min_z], max: [max_x, max_y, max_z] }
}

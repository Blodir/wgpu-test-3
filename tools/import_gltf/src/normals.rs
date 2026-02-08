use glam::Vec3;

pub fn generate_flat_normals_for_mesh(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Result<Vec<[f32; 3]>, String> {
    if indices.len() % 3 != 0 {
        return Err("index buffer length must be a multiple of 3 (triangles)".to_string());
    }

    let mut normals = vec![[0.0; 3]; positions.len()];

    for triangle in indices.chunks_exact(3) {
        let i0 = triangle[0] as usize;
        let i1 = triangle[1] as usize;
        let i2 = triangle[2] as usize;

        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            return Err("index points outside of position buffer".to_string());
        }

        let p0 = Vec3::from(positions[i0]);
        let p1 = Vec3::from(positions[i1]);
        let p2 = Vec3::from(positions[i2]);

        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let cross = edge1.cross(edge2);

        let normal = if cross.length_squared() < f32::EPSILON {
            Vec3::Y
        } else {
            cross.normalize()
        };

        normals[i0] = normal.to_array();
        normals[i1] = normal.to_array();
        normals[i2] = normal.to_array();
    }

    Ok(normals)
}

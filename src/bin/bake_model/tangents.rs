use mikktspace::{generate_tangents, Geometry};

pub fn generate_tangents_for_mesh(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Result<Vec<[f32; 4]>, String> {
    if positions.len() != normals.len() || positions.len() != uvs.len() {
        return Err("positions, normals, and uvs must have equal length".to_string());
    }
    if indices.len() % 3 != 0 {
        return Err("index buffer length must be a multiple of 3 (triangles)".to_string());
    }

    struct MeshGeometry<'a> {
        positions: &'a [[f32; 3]],
        normals: &'a [[f32; 3]],
        uvs: &'a [[f32; 2]],
        indices: &'a [u32],
        tangents: &'a mut [[f32; 4]],
    }

    impl<'a> Geometry for MeshGeometry<'a> {
        fn num_faces(&self) -> usize {
            self.indices.len() / 3
        }

        fn num_vertices_of_face(&self, _face: usize) -> usize {
            3
        }

        fn position(&self, face: usize, vert: usize) -> [f32; 3] {
            let idx = self.indices[face * 3 + vert] as usize;
            self.positions[idx]
        }

        fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
            let idx = self.indices[face * 3 + vert] as usize;
            self.normals[idx]
        }

        fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
            let idx = self.indices[face * 3 + vert] as usize;
            self.uvs[idx]
        }

        fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
            let idx = self.indices[face * 3 + vert] as usize;
            self.tangents[idx] = tangent;
        }
    }

    let mut tangents = vec![[0.0; 4]; positions.len()];
    let mut geom = MeshGeometry {
        positions,
        normals,
        uvs,
        indices,
        tangents: tangents.as_mut_slice(),
    };

    let ok = generate_tangents(&mut geom);
    if !ok {
        return Err("mikktspace failed to generate tangents".to_string());
    }

    Ok(tangents)
}

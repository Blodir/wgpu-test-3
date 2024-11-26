gltf:
- different mesh topologies
- Winding order: "When a mesh primitive uses any triangle-based topology (i.e., triangles, triangle strip, or triangle fan), the determinant of the node’s global transform defines the winding order of that primitive. If the determinant is a positive value, the winding order triangle faces is counterclockwise; in the opposite case, the winding order is clockwise."

Loose ends:
- normal map generation
- tangent map generation (mikktspace)
- default normal maps (remove the normal sample w hack)
- compensate for lost energy in prefiltered env map and diffuse irradiance clamping

misc:
- the sun should be treated as a disc light, copy the sun section here page 36: https://seblagarde.wordpress.com/wp-content/uploads/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf
- centralize depth stencil state


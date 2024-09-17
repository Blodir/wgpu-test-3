3D renderer with Rust and wgpu.

Features
- partial GLTF support
    - .gltf not yet supported (.glb is supported)
    - name fields in general are not respected
    - accessors
        - missing normalized ints and sparse accessors
    - images:
        - missing some mime types
        - uri missing
    - materials:
        - missing alphaMode, alphaCutoff, doubleSided, occlusion.strength
    - mesh
        - mesh.weights missing
    - primitive
        - mode (primitive topology) missing
            - winding order
        - targets (morph targets) missing
        - primitive attributes
            - only one each of JOINTS and WEIGHTS supported
            - COLOR missing!!! (vertex colors)
    - node
        - cameras not planned
        - missing skin, weights
    - scene: only renders the root scene
    - skin: 0%
    - animations: 0%
    - lights: 0%
    - cameras: not planned
    - extensions: no extensions planned
    - BRDF implementation needs to be checked for compliance
- importing equirectangular .hdr radiance maps (projected onto a rgba16f cubemap)
- baking mipmaps
- screen space skyboxes
- PBR (physically based rendering) along with IBL (image based lighting)
    - analytical lights: just directional for now
    - image based diffuse irradiance
    - split sum specular approximation (prefiltered env map calculated on the fly, BRDF LUT read from a texture)
- normal mapping (with world-space lighting)
- HDR (needs some improvement with a separate tone mapping pass, physical units, and so on)
- partial shader hot-reload (just of pbr.wgsl atm...)
- some basic camera movements for looking around with lmb drag and scroll

Roadmap
- normal generation
- tangent generation (mikktspace?)
- missing gltf properties
- .gltf file support
- ui


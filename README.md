Work in progress 3D game engine with Rust and wgpu.

<img width="2560" height="1392" alt="image" src="https://github.com/user-attachments/assets/aaa1d077-0cb4-4b73-a9fb-77a93dd7e56a" />
^ drawing 10k lamp posts at 70fps; 53m triangles per frame / 3.8b per second

------

Features
- skeletal animation
    - animation evaluation jobs executed on worker threads
    - simple anim graph with states/transitions + blending
    - no root movement, blend masks, events, etc...
    - animating submeshes that aren't part of a skeleton is planned
- asset streaming
    - separate io worker pool
    - eviction not yet implemented
- gltf import
    - .gltf and .glb files
    - primitive
        - mode (primitive topology) missing
            - winding order
        - targets (morph targets) missing
        - primitive attributes
            - only one each of JOINTS and WEIGHTS supported
            - no vertex colors
    - node
        - cameras, lights not planned
    - scene: only renders the root scene
    - extensions: no extensions planned
- importing equirectangular .hdr radiance maps (projected onto a rgba16f cubemap)
- baking mipmaps
- textures using dds with block compression
- screen space skyboxes
- physically based rendering (PBR) along with image based lighting (IBL)
    - analytical lights: just directional for now
    - image based diffuse irradiance
    - split sum specular approximation (prefiltered env map calculated on the fly, BRDF LUT read from a texture)
- normal mapping (with world-space lighting)
- HDR (needs some improvement with physical units)
- 4x MSAA
- some basic camera movements for looking around with lmb drag and scroll

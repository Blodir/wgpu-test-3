@group(0) @binding(0) var normal_roughness_history: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let positions = array(
        vec2f(1.0, 1.0),
        vec2f(1.0, -1.0),
        vec2f(-1.0, -1.0),
        vec2f(-1.0, 1.0)
    );
    out.clip_position = vec4f(positions[vertex_index], 0.0, 1.0);
    out.tex_coords = positions[vertex_index] * vec2f(0.5, -0.5) + vec2f(0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let dims = vec2i(textureDimensions(normal_roughness_history));
    let coords = clamp(vec2i(in.tex_coords * vec2f(dims)), vec2i(0), dims - vec2i(1));
    let normal = textureLoad(normal_roughness_history, coords, 0);
    //return vec4f(vec3f(normal.w), 1.0);
    return vec4f(normal.xyz * 0.5 + vec3f(0.5), 1.0);
}

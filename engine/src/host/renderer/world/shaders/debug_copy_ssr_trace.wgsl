@group(0) @binding(0) var ssr_trace: texture_2d<f32>;

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
    let dims = vec2i(textureDimensions(ssr_trace));
    let coords = clamp(vec2i(in.tex_coords * vec2f(dims)), vec2i(0), dims - vec2i(1));
    let radiance = textureLoad(ssr_trace, coords, 0).rgb;
    return vec4f(radiance, 1.0);
}

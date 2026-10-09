struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) depth_pyramid: f32,
}

@group(0) @binding(0) var gbuffer_depth: texture_depth_2d;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    var pos = array(
        vec2(1.0, 1.0),
        vec2(1.0, -1.0),
        vec2(-1.0, -1.0),
        vec2(-1.0, 1.0)
    );
    var tex_coords = array(
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
        vec2(0.0, 0.0)
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let dims = textureDimensions(gbuffer_depth);
    let max_coord = vec2f(dims) - vec2f(1.0);
    let coord = vec2i(clamp(in.tex_coords * vec2f(dims), vec2f(0.0), max_coord));
    let depth = textureLoad(gbuffer_depth, coord, 0);
    return FragmentOutput(depth);
}

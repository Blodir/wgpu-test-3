struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) depth_pyramid: f32,
}

@group(0) @binding(0) var depth_pyramid: texture_2d<f32>;

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

fn sample_depth_hiz_2x2(coord: vec2u) -> f32 {
    let dims = textureDimensions(depth_pyramid);
    let max_coord = dims - vec2u(1u, 1u);
    let c00 = min(coord, max_coord);
    let c10 = min(coord + vec2u(1u, 0u), max_coord);
    let c01 = min(coord + vec2u(0u, 1u), max_coord);
    let c11 = min(coord + vec2u(1u, 1u), max_coord);
    let d00 = textureLoad(depth_pyramid, vec2i(c00), 0).x;
    let d10 = textureLoad(depth_pyramid, vec2i(c10), 0).x;
    let d01 = textureLoad(depth_pyramid, vec2i(c01), 0).x;
    let d11 = textureLoad(depth_pyramid, vec2i(c11), 0).x;
    return max(max(d00, d10), max(d01, d11));
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let dst_coord = vec2u(in.clip_position.xy);
    let depth = sample_depth_hiz_2x2(dst_coord * 2u);
    return FragmentOutput(depth);
}

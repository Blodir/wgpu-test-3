@group(0) @binding(0) var taa_history_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

fn to_ldr(color: vec3f) -> vec3f {
    var col = color;
    let exposure = 0.0;
    let exposure_factor = pow(2.0, exposure);
    col = col * exposure_factor;
    col = col / (col + vec3f(1.0));
    return col;
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
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
        vec2(0.0, 0.0),
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0, 1);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims_u = textureDimensions(taa_history_texture);
    let dims = vec2f(dims_u);
    let max_coords = vec2i(dims_u) - vec2i(1);
    let coords = min(vec2i(in.tex_coords * dims), max_coords);
    let hdr_color = textureLoad(taa_history_texture, coords, 0).rgb;
    return vec4f(to_ldr(hdr_color), 1.0);
}

@group(0) @binding(0) var motion_vectors_texture: texture_2d<f32>;
@group(0) @binding(1) var prev_gi_source_texture: texture_2d<f32>;
@group(0) @binding(2) var prev_history_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

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
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims_u = textureDimensions(prev_gi_source_texture, 0);
    let dims = vec2f(dims_u);
    let max_coord = vec2f(dims_u) - vec2f(1.0);
    let curr_coord = vec2i(clamp(in.tex_coords * dims, vec2f(0.0), max_coord));
    let motion = textureLoad(motion_vectors_texture, curr_coord, 0).xy;
    let prev_uv = in.tex_coords - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return vec4f(0.0);
    }

    let prev_coord = vec2i(clamp(prev_uv * dims, vec2f(0.0), max_coord));
    let prev_gi = textureLoad(prev_gi_source_texture, prev_coord, 0);
    let prev_history = textureLoad(prev_history_texture, prev_coord, 0);
    let history_weight = 0.1;
    let blended_rgb = mix(prev_gi.rgb, prev_history.rgb, history_weight);
    // no need to blend ao since hbil pass already does that
    //let blended_alpha = mix(prev_gi.a, prev_history.a, history_weight);
    return vec4f(blended_rgb, prev_gi.a);
}

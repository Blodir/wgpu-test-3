@group(0) @binding(0) var motion_vectors_texture: texture_2d<f32>;
@group(0) @binding(1) var prev_gi_source_texture: texture_2d<f32>;
@group(0) @binding(2) var prev_hbgi_reproject_texture: texture_2d<f32>;
@group(0) @binding(3) var current_depth_texture: texture_depth_2d;
@group(0) @binding(4) var current_normal_texture: texture_2d<f32>;
@group(0) @binding(5) var prev_depth_history_texture: texture_2d<f32>;
@group(0) @binding(6) var prev_normal_history_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) hbgi_reproject: vec4<f32>,
    @location(1) depth_history: vec4<f32>,
    @location(2) normal_history: vec4<f32>,
}

const HISTORY_DEPTH_THRESHOLD: f32 = 0.0015;
const HISTORY_NORMAL_THRESHOLD: f32 = 0.95;

fn safe_normalize3(v: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3f(0.0);
}

fn invalid_history_output(depth: f32, normal: vec4f) -> FragmentOutput {
    return FragmentOutput(
        vec4f(0.0, 0.0, 0.0, -1.0),
        vec4f(depth, 0.0, 0.0, 1.0),
        normal
    );
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
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let dims_u = textureDimensions(current_depth_texture);
    let dims = vec2f(dims_u);
    let max_coord = vec2f(dims_u) - vec2f(1.0);
    let curr_coord = vec2i(clamp(in.tex_coords * dims, vec2f(0.0), max_coord));
    let curr_depth = textureLoad(current_depth_texture, curr_coord, 0);
    let curr_normal_sample = textureLoad(current_normal_texture, curr_coord, 0);
    let curr_normal = safe_normalize3(curr_normal_sample.xyz);

    if (curr_depth >= 1.0) {
        return invalid_history_output(curr_depth, curr_normal_sample);
    }

    let motion = textureLoad(motion_vectors_texture, curr_coord, 0).xy;
    let prev_uv = in.tex_coords - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return invalid_history_output(curr_depth, curr_normal_sample);
    }

    let prev_coord = vec2i(clamp(prev_uv * dims, vec2f(0.0), max_coord));
    let prev_depth = textureLoad(prev_depth_history_texture, prev_coord, 0).x;
    let prev_normal_sample = textureLoad(prev_normal_history_texture, prev_coord, 0);
    let prev_normal = safe_normalize3(prev_normal_sample.xyz);
    let history_valid = (
        prev_depth < 1.0
        && dot(curr_normal, prev_normal) > HISTORY_NORMAL_THRESHOLD
        && abs(curr_depth - prev_depth) < HISTORY_DEPTH_THRESHOLD
    );

    if (!history_valid) {
        return invalid_history_output(curr_depth, curr_normal_sample);
    }

    let prev_gi = textureLoad(prev_gi_source_texture, prev_coord, 0);
    let prev_hbgi_reproject = textureLoad(prev_hbgi_reproject_texture, prev_coord, 0);
    let hbgi_reproject_weight = 0.6;
    let blended_rgb = mix(prev_gi.rgb, prev_hbgi_reproject.rgb, hbgi_reproject_weight);
    return FragmentOutput(
        vec4f(blended_rgb, prev_gi.a),
        vec4f(curr_depth, 0.0, 0.0, 1.0),
        curr_normal_sample
    );
}

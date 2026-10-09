@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(1) @binding(3) var<uniform> prev_inverse_view_proj: mat4x4<f32>;

@group(0) @binding(0) var gbuffer_normal_roughness: texture_2d<f32>;
@group(0) @binding(1) var diffuse_history: texture_2d<f32>;
@group(0) @binding(2) var motion_vectors_texture: texture_2d<f32>;
@group(0) @binding(3) var depth_texture: texture_depth_2d;
@group(0) @binding(4) var prev_depth_history_texture: texture_2d<f32>;
@group(0) @binding(5) var prev_normal_roughness_history_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) normal_roughness_history: vec4<f32>,
    @location(1) reprojected_diffuse: vec4<f32>,
}

const MAX_HISTORY_LENGTH: f32 = 8.0;
const SVGF_NORMAL_REJECTION_DOT_THRESHOLD: f32 = 0.5;
const SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD: f32 = 1e0;

fn reconstruct_world_position_from_depth(
    uv: vec2f,
    depth: f32,
    inverse_view_proj_matrix: mat4x4<f32>,
) -> vec3f {
    let clip = vec4f(
        uv.x * 2.0 - 1.0,
        1.0 - uv.y * 2.0,
        depth,
        1.0
    );
    let world = inverse_view_proj_matrix * clip;
    if (abs(world.w) > 1e-8) {
        return world.xyz / world.w;
    }
    return vec3f(0.0);
}

fn clamp_coord(coord: vec2i, max_coord: vec2i) -> vec2i {
    return clamp(coord, vec2i(0), max_coord);
}

fn clamp_coordf(coord: vec2f, max_coord: vec2f) -> vec2f {
    return clamp(coord, vec2f(0), max_coord);
}

fn sample_2x2(source: texture_2d<f32>, coord: vec2i) -> vec4f {
    let max_coord = vec2i(textureDimensions(source)) - vec2i(1);
    let sample00 = textureLoad(source, clamp_coord(coord, max_coord), 0);
    let sample10 = textureLoad(source, clamp_coord(coord + vec2i(1, 0), max_coord), 0);
    let sample01 = textureLoad(source, clamp_coord(coord + vec2i(0, 1), max_coord), 0);
    let sample11 = textureLoad(source, clamp_coord(coord + vec2i(1, 1), max_coord), 0);
    return (sample00 + sample10 + sample01 + sample11) * 0.25;
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
    let uv = in.tex_coords;
    let full_dims_u = textureDimensions(depth_texture);
    let full_dims = vec2f(full_dims_u);
    let full_max_coords = vec2i(full_dims_u) - vec2i(1);
    let curr_full_coords = min(vec2i(uv * full_dims), full_max_coords);

    let half_dims_u = textureDimensions(prev_normal_roughness_history_texture);
    let half_dims = vec2f(half_dims_u);
    let half_max_coords = vec2i(half_dims_u) - vec2i(1);

    let curr_half_coords = vec2i(in.clip_position.xy);
    var normal_roughness = sample_2x2(gbuffer_normal_roughness, curr_half_coords * 2);
    let normal_length = length(normal_roughness.xyz);
    if (normal_length > 1e-8) {
        normal_roughness = vec4f(normal_roughness.xyz / normal_length, normal_roughness.w);
    }
    let curr_depth = textureLoad(depth_texture, curr_full_coords, 0);
    if (curr_depth <= 0.0 || normal_length <= 1e-8) {
        return FragmentOutput(normal_roughness, vec4f(0.0));
    }

    let world_position = reconstruct_world_position_from_depth(
        uv,
        curr_depth,
        inverse_view_proj
    );
    let N = normalize(normal_roughness.xyz);

    let motion = textureLoad(motion_vectors_texture, curr_full_coords, 0).xy;
    let prev_uv = uv - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return FragmentOutput(normal_roughness, vec4f(0.0, 0.0, 0.0, 1.0));
    }

    let prev_coord = clamp_coordf(prev_uv * half_dims - vec2f(0.5), vec2f(half_max_coords));
    let prev_coord_floor = floor(prev_coord);
    let prev_coord_frac = fract(prev_coord);

    let taps = array(
        vec2i(prev_coord_floor),
        vec2i(prev_coord_floor) + vec2i(1, 0),
        vec2i(prev_coord_floor) + vec2i(0, 1),
        vec2i(prev_coord_floor) + vec2i(1, 1),
    );
    let tap_weights = array(
        (1.0 - prev_coord_frac.x) * (1.0 - prev_coord_frac.y),
        prev_coord_frac.x * (1.0 - prev_coord_frac.y),
        (1.0 - prev_coord_frac.x) * prev_coord_frac.y,
        prev_coord_frac.x * prev_coord_frac.y,
    );

    var prev_color_acc = vec3f(0.0);
    var prev_normal_acc = vec3f(0.0);
    var prev_history_length_acc = 0.0;
    var prev_weight_acc = 0.0;

    for (var i = 0; i < 4; i++) {
        let tap_coords = clamp_coord(taps[i], half_max_coords);
        let tap_uv = (vec2f(tap_coords) + vec2f(0.5)) / half_dims;
        let tap_weight = tap_weights[i];

        let prev_depth_dims = vec2i(textureDimensions(prev_depth_history_texture));
        let prev_depth_coords = clamp_coord(vec2i(tap_uv * vec2f(prev_depth_dims)), prev_depth_dims - vec2i(1));
        let prev_depth_history = textureLoad(prev_depth_history_texture, prev_depth_coords, 0);
        let prev_depth = prev_depth_history.x;
        if (prev_depth <= 0.0) {
            continue;
        }
        let prev_normal_sample = textureLoad(prev_normal_roughness_history_texture, tap_coords, 0).xyz;
        if (length(prev_normal_sample) <= 1e-8) {
            continue;
        }
        let prev_normal = normalize(prev_normal_sample);
        let prev_world_position = reconstruct_world_position_from_depth(
            tap_uv,
            prev_depth,
            prev_inverse_view_proj
        );

        let delta = prev_world_position - world_position;
        let plane_distance = abs(dot(N, delta));
        let normal_similarity = dot(N, prev_normal);
        //let normal_weight = clamp((normal_similarity - SVGF_NORMAL_REJECTION_DOT_THRESHOLD) / (1.0 - SVGF_NORMAL_REJECTION_DOT_THRESHOLD), 0.0, 1.0);
        let normal_weight = smoothstep(0.5, 0.95, normal_similarity);
        let plane_weight = clamp(1.0 - (plane_distance / SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD), 0.0, 1.0);
        let history_weight = tap_weight * normal_weight * plane_weight;
        //let history_weight = tap_weight * plane_weight;
        if (history_weight <= 0.0) {
            continue;
        }

        let prev_color_sample = sample_2x2(diffuse_history, tap_coords * 2);
        if (prev_color_sample.a <= 0.0) {
            continue;
        }

        prev_color_acc += prev_color_sample.rgb * history_weight;
        prev_normal_acc += prev_normal * history_weight;
        prev_history_length_acc += prev_color_sample.a * history_weight;
        prev_weight_acc += history_weight;
    }

    if (prev_weight_acc <= 0.0) {
        return FragmentOutput(normal_roughness, vec4f(0.0, 0.0, 0.0, 1.0));
    }

    prev_color_acc /= prev_weight_acc;
    prev_normal_acc /= prev_weight_acc;
    prev_history_length_acc /= prev_weight_acc;

    let blend_factor = 1.0 / (1.0 + min(prev_history_length_acc, MAX_HISTORY_LENGTH - 1.0));

    let final_normal = normalize(mix(normalize(prev_normal_acc), normal_roughness.xyz, blend_factor));
    let final_history_length = min(prev_history_length_acc + 1.0, MAX_HISTORY_LENGTH);

    return FragmentOutput(
        vec4f(final_normal, normal_roughness.w),
        vec4f(prev_color_acc, final_history_length)
    );
}

@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(1) @binding(3) var<uniform> prev_inverse_view_proj: mat4x4<f32>;
@group(1) @binding(7) var<uniform> camera_jitter: vec4<f32>;

@group(0) @binding(0) var gbuffer_normal_roughness: texture_2d<f32>;
@group(0) @binding(1) var diffuse_history: texture_2d<f32>;
@group(0) @binding(2) var motion_vectors_texture: texture_2d<f32>;
@group(0) @binding(3) var depth_texture: texture_depth_2d;
@group(0) @binding(4) var prev_depth_history_texture: texture_2d<f32>;
@group(0) @binding(5) var prev_normal_roughness_history_texture: texture_2d<f32>;
@group(0) @binding(6) var prev_bent_normal_ao_denoise_texture: texture_2d<f32>;
@group(0) @binding(7) var depth_pyramid: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) normal_roughness_history: vec4<f32>,
    @location(1) reprojected_diffuse: vec4<f32>,
    @location(2) reprojected_bent_normal_ao: vec4<f32>,
    @location(3) normal_variance: f32,
}

const MAX_HISTORY_LENGTH: f32 = 8.0;
const SVGF_NORMAL_REJECTION_DOT_THRESHOLD: f32 = 0.5;
const SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD: f32 = 1e0;

fn safe_normalize3(value: vec3f, fallback: vec3f) -> vec3f {
    let len = length(value);
    if (len > 1e-8 && len < 1e20) {
        return value / len;
    }
    return fallback;
}

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

struct NormalRoughnessVariance {
    normal_roughness: vec4f,
    variance: f32,
    valid: bool,
}

fn normal_roughness_variance(coords: vec2i, max_coords: vec2i) -> NormalRoughnessVariance {
    var normal_acc = vec3f(0.0);
    var roughness_acc = 0.0;
    var sample_count = 0.0;
    for (var y: i32 = 0; y < 2; y += 1) {
        for (var x: i32 = 0; x < 2; x += 1) {
            let sample_coords = clamp_coord(coords + vec2i(x, y), max_coords);
            let depth = textureLoad(depth_texture, sample_coords, 0);
            let normal_roughness = textureLoad(gbuffer_normal_roughness, sample_coords, 0);
            let normal = normal_roughness.xyz;
            let normal_length = length(normal);
            if (!(depth > 0.0 && normal_length > 1e-8 && normal_length < 1e20)) {
                continue;
            }
            normal_acc += normal / normal_length;
            roughness_acc += normal_roughness.w;
            sample_count += 1.0;
        }
    }
    if (sample_count <= 0.0) {
        return NormalRoughnessVariance(vec4f(0.0), 0.0, false);
    }
    let mean_normal = normal_acc / sample_count;
    let mean_roughness = roughness_acc / sample_count;
    // there are multiple possible ways of representing the variance of a set of vectors
    // the choice here was basically arbitrary
    let variance = clamp(1.0 - dot(mean_normal, mean_normal), 0.0, 1.0);
    return NormalRoughnessVariance(vec4f(mean_normal, mean_roughness), variance, true);
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
    let prev_depth_mip = i32(min(1u, textureNumLevels(prev_depth_history_texture) - 1u));

    let curr_half_coords = vec2i(in.clip_position.xy);
    let nrv = normal_roughness_variance(curr_half_coords * 2, full_max_coords);
    let N = nrv.normal_roughness.xyz;
    let roughness = nrv.normal_roughness.w;
    let variance = nrv.variance;
    if (!nrv.valid) {
        return FragmentOutput(nrv.normal_roughness, vec4f(0.0), vec4f(0.0, 0.0, 0.0, 1.0), variance);
    }

    let curr_depth_mip = i32(min(1u, textureNumLevels(depth_pyramid) - 1u));
    let curr_depth = textureLoad(depth_pyramid, curr_half_coords, curr_depth_mip).r;
    if (curr_depth <= 0.0) {
        return FragmentOutput(nrv.normal_roughness, vec4f(0.0), vec4f(0.0, 0.0, 0.0, 1.0), variance);
    }

    let world_position = reconstruct_world_position_from_depth(
        uv,
        curr_depth,
        inverse_view_proj
    );

    let motion = textureLoad(motion_vectors_texture, curr_full_coords, 0).xy;
    //let prev_uv = uv - motion + camera_jitter.zw - camera_jitter.xy;
    let prev_uv = uv - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return FragmentOutput(vec4f(N, roughness), vec4f(0.0, 0.0, 0.0, 1.0), vec4f(N, 1.0), variance);
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
    var prev_bent_normal_acc = vec3f(0.0);
    var prev_ao_acc = 0.0;
    var prev_history_length_acc = 0.0;
    var prev_weight_acc = 0.0;

    for (var i = 0; i < 4; i++) {
        let tap_coords = clamp_coord(taps[i], half_max_coords);
        let tap_uv = (vec2f(tap_coords) + vec2f(0.5)) / half_dims;
        let tap_weight = tap_weights[i];

        let prev_depth = textureLoad(prev_depth_history_texture, tap_coords, prev_depth_mip).x;
        if (prev_depth <= 0.0) {
            continue;
        }
        let prev_normal_sample = textureLoad(prev_normal_roughness_history_texture, tap_coords, 0).xyz;
        let prev_normal_length = length(prev_normal_sample);
        if (!(prev_normal_length > 1e-8 && prev_normal_length < 1e20)) {
            continue;
        }
        let prev_normal = prev_normal_sample / prev_normal_length;
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
        //let history_weight = tap_weight * normal_weight * plane_weight;
        let history_weight = tap_weight;
        if (history_weight <= 0.0) {
            continue;
        }

        let prev_color_sample = sample_2x2(diffuse_history, tap_coords * 2);
        if (prev_color_sample.a <= 0.0) {
            continue;
        }
        let prev_bent_ao = textureLoad(prev_bent_normal_ao_denoise_texture, tap_coords, 0);

        prev_color_acc += prev_color_sample.rgb * history_weight;
        prev_normal_acc += prev_normal * history_weight;
        prev_bent_normal_acc += prev_bent_ao.xyz * history_weight;
        prev_ao_acc += prev_bent_ao.w * history_weight;
        prev_history_length_acc += prev_color_sample.a * history_weight;
        prev_weight_acc += history_weight;
    }

    if (prev_weight_acc <= 0.0) {
        return FragmentOutput(vec4f(N, roughness), vec4f(0.0, 0.0, 0.0, 1.0), vec4f(N, 1.0), variance);
    }

    prev_color_acc /= prev_weight_acc;
    prev_normal_acc /= prev_weight_acc;
    prev_bent_normal_acc /= prev_weight_acc;
    prev_ao_acc /= prev_weight_acc;
    prev_history_length_acc /= prev_weight_acc;

    let blend_factor = 1.0 / (1.0 + min(prev_history_length_acc, MAX_HISTORY_LENGTH - 1.0));
    //let blend_factor = 0.01;

    let prev_normal = safe_normalize3(prev_normal_acc, N);
    let final_normal = safe_normalize3(mix(prev_normal, N, blend_factor), N);
    let final_history_length = min(prev_history_length_acc + 1.0, MAX_HISTORY_LENGTH);
    let reprojected_bent_normal = safe_normalize3(prev_bent_normal_acc, N);

    return FragmentOutput(
        vec4f(final_normal, roughness),
        vec4f(prev_color_acc, final_history_length),
        vec4f(reprojected_bent_normal, prev_ao_acc),
        variance
    );
}

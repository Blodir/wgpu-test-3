// https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/

@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(1) @binding(3) var<uniform> prev_inverse_view_proj: mat4x4<f32>;
@group(1) @binding(7) var<uniform> camera_jitter: vec4<f32>;

@group(0) @binding(0) var ssgi_trace: texture_2d<f32>;
@group(0) @binding(1) var previous_ssgi_history: texture_2d<f32>;
@group(0) @binding(2) var ssr_trace: texture_2d<f32>;
@group(0) @binding(3) var previous_ssr_history: texture_2d<f32>;
@group(0) @binding(4) var previous_moments_history: texture_2d<f32>;
@group(0) @binding(5) var motion_vectors_texture: texture_2d<f32>;
@group(0) @binding(6) var current_depth_texture: texture_depth_2d;
@group(0) @binding(7) var current_normal_texture: texture_2d<f32>;
@group(0) @binding(8) var prev_depth_history_texture: texture_2d<f32>;
@group(0) @binding(9) var prev_normal_roughness_history_texture: texture_2d<f32>;
@group(0) @binding(10) var curr_bent_normal_ao_texture: texture_2d<f32>;
@group(0) @binding(11) var prev_bent_normal_ao_history_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) ssgi_history: vec4<f32>,
    @location(1) ssr_history: vec4<f32>,
    @location(2) moments_history: vec4<f32>,
    @location(3) bent_normal_ao_history: vec4<f32>,
}

const MAX_HISTORY_LENGTH: f32 = 8.0;
const SVGF_NORMAL_REJECTION_DOT_THRESHOLD: f32 = 0.9;
const SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD: f32 = 0.3;

fn safe_normalize3(v: vec3f, fallback: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8 && len < 1e20) {
        return v / len;
    }
    return fallback;
}

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
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

fn luminance(rgb: vec3f) -> f32 {
    return dot(rgb, vec3f(0.2126, 0.7152, 0.0722));
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

/* Based on Spatiotemporal Variance-Guided Filtering - Schied et al.
    https://research.nvidia.com/sites/default/files/pubs/2017-07_Spatiotemporal-Variance-Guided-Filtering%3A//svgf_preprint.pdf
*/
@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let curr_uv = in.tex_coords;

    // Full resolution texture coordinates
    let full_dims_u = textureDimensions(current_depth_texture);
    let full_dims = vec2f(full_dims_u);
    let full_max_coords = vec2i(full_dims_u) - vec2i(1);
    let curr_full_coords = min(vec2i(curr_uv * full_dims), full_max_coords);

    // History texture coordinates
    let half_dims_u = textureDimensions(ssgi_trace);
    let half_dims = vec2f(half_dims_u);
    let half_max_coords = vec2i(half_dims_u) - vec2i(1);
    let curr_half_coords = min(vec2i(curr_uv * half_dims), half_max_coords);

    // Load current frame textures
    let curr_depth = textureLoad(current_depth_texture, curr_full_coords, 0);
    let curr_normal_sample = textureLoad(current_normal_texture, curr_half_coords, 0);
    let curr_ssgi = textureLoad(ssgi_trace, curr_half_coords, 0);
    let curr_ssr = textureLoad(ssr_trace, curr_half_coords, 0);
    let curr_bent_ao = textureLoad(curr_bent_normal_ao_texture, curr_half_coords, 0);

    // depth gets cleared to zero -> zero indicates an invalid value
    if (curr_depth <= 0.0) {
        let curr_lum = luminance(curr_ssgi.rgb);
        let curr_ssr_lum = luminance(curr_ssr.rgb);
        return FragmentOutput(
            vec4f(curr_ssgi.rgb, 1.0),
            vec4f(curr_ssr.rgb, 1.0),
            vec4f(curr_lum, curr_lum * curr_lum, curr_ssr_lum, curr_ssr_lum * curr_ssr_lum),
            curr_bent_ao
        );
    }

    let curr_world_position = reconstruct_world_position_from_depth(
        curr_uv,
        curr_depth,
        inverse_view_proj
    );
    let curr_normal = normalize(curr_normal_sample.xyz);

    let motion = textureLoad(motion_vectors_texture, curr_full_coords, 0).xy;
    //let prev_uv = curr_uv - motion + camera_jitter.zw - camera_jitter.xy;
    let prev_uv = curr_uv - motion;

    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        let curr_lum = luminance(curr_ssgi.rgb);
        let curr_ssr_lum = luminance(curr_ssr.rgb);
        return FragmentOutput(
            vec4f(curr_ssgi.rgb, 1.0),
            vec4f(curr_ssr.rgb, 1.0),
            vec4f(curr_lum, curr_lum * curr_lum, curr_ssr_lum, curr_ssr_lum * curr_ssr_lum),
            curr_bent_ao
        );
    }

    // uv points to the middle of a pixel
    // shift by -0.5 such that we point to the top left of the pixel instead
    // after motion vectors we land somewhere within some pixel
    // and we can take the 4 corner points (rather than the centers of pixels as we would if we didn't shift by -0.5)
    let prev_coord = clamp_coordf(prev_uv * half_dims - vec2f(0.5), vec2f(half_max_coords));
    let prev_coord_floor = floor(prev_coord);
    let prev_coord_frac = fract(prev_coord);

    // -------------------------------- //
    // TEST: PLANE AND NORMAL REJECTION TESTS APPEAR TO BE WORKING CORRECTLY
    /*
    let prev_depth_history = textureLoad(prev_depth_history_texture, vec2i(prev_coord), 0);
    let prev_depth = prev_depth_history.x;
    let prev_world_position = reconstruct_world_position_from_depth(
        prev_uv,
        prev_depth,
        prev_inverse_view_proj
    );
    let prev_normal = normalize(textureLoad(prev_normal_roughness_history_texture, vec2i(prev_coord), 0).xyz);

    let dist_test = distance(prev_world_position, curr_world_position) <= 1.0;

    let delta = prev_world_position - curr_world_position;
    let plane_distance = abs(dot(curr_normal, delta));
    let plane_test = plane_distance <= 0.3;
    let normal_test = dot(curr_normal, prev_normal) >= 0.90;

    if normal_test {
        return current_frame_output(
            vec4f(1.0),
            curr_ssgi,
            curr_depth,
            curr_normal_sample
        );
    } else {
        return current_frame_output(
            vec4f(0.0),
            curr_ssgi,
            curr_depth,
            curr_normal_sample
        );
    }
    */
    // -------------------------------- //

    // 4.1 Temporal filtering
    // 2x2 tap bilinear filter described by the SVGF paper
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

    var prev_irradiance_acc = vec3f(0.0);
    var prev_bent_normal_acc = vec3f(0.0);
    var prev_ao_acc = 0.0;
    var prev_ssr_radiance_acc = vec3f(0.0);
    var prev_first_moment_acc = 0.0;
    var prev_second_moment_acc = 0.0;
    var prev_history_length_acc = 0.0;
    var prev_ssr_first_moment_acc = 0.0;
    var prev_ssr_second_moment_acc = 0.0;
    var prev_ssr_history_length_acc = 0.0;
    var valid_weight_sum = 0.0;
    for (var i = 0; i < 4; i++) {
        let coords = clamp_coord(taps[i], half_max_coords);
        // again, we want uv to point to the middle of a pixel, so we need to re-add the 0.5 we shifted by earlier
        let tap_uv = (vec2f(coords) + vec2f(0.5)) / half_dims;
        let tap_weight = tap_weights[i];

        let prev_depth_dims = vec2i(textureDimensions(prev_depth_history_texture));
        let prev_depth_coords = clamp_coord(vec2i(tap_uv * vec2f(prev_depth_dims)), prev_depth_dims - vec2i(1));
        let prev_depth_history = textureLoad(prev_depth_history_texture, prev_depth_coords, 0);
        let prev_depth = prev_depth_history.x;
        if (prev_depth <= 0.0) {
            continue;
        }
        let prev_normal = normalize(textureLoad(prev_normal_roughness_history_texture, coords, 0).xyz);
        let prev_world_position = reconstruct_world_position_from_depth(
            tap_uv,
            prev_depth,
            prev_inverse_view_proj
        );

        // TODO add (sub)Mesh ID
        // Temporal rejection
        let delta = prev_world_position - curr_world_position;
        let plane_distance = abs(dot(curr_normal, delta));
        if (
            dot(curr_normal, prev_normal) <= SVGF_NORMAL_REJECTION_DOT_THRESHOLD
            || plane_distance >= SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD
        ) {
            continue;
        }

        let prev_ssgi = textureLoad(previous_ssgi_history, coords, 0);
        let prev_ssr = textureLoad(previous_ssr_history, coords, 0);
        let prev_moments = textureLoad(previous_moments_history, coords, 0);
        let prev_bent_ao = textureLoad(prev_bent_normal_ao_history_texture, coords, 0);

        prev_irradiance_acc += prev_ssgi.rgb * tap_weight;
        prev_bent_normal_acc += prev_bent_ao.xyz * tap_weight;
        prev_ao_acc += prev_bent_ao.w * tap_weight;
        prev_ssr_radiance_acc += prev_ssr.rgb * tap_weight;
        prev_first_moment_acc += prev_moments.x * tap_weight;
        prev_second_moment_acc += prev_moments.y * tap_weight;
        prev_history_length_acc += prev_ssgi.a * tap_weight;
        prev_ssr_first_moment_acc += prev_moments.z * tap_weight;
        prev_ssr_second_moment_acc += prev_moments.w * tap_weight;
        prev_ssr_history_length_acc += prev_ssr.a * tap_weight;
        valid_weight_sum += tap_weight;
    }

    if (valid_weight_sum <= 0.0) {
        // TODO SVGF paper describes fallback to 3x3 filter
        let curr_lum = luminance(curr_ssgi.rgb);
        let curr_ssr_lum = luminance(curr_ssr.rgb);
        return FragmentOutput(
            vec4f(curr_ssgi.rgb, 1.0),
            vec4f(curr_ssr.rgb, 1.0),
            vec4f(curr_lum, curr_lum * curr_lum, curr_ssr_lum, curr_ssr_lum * curr_ssr_lum),
            curr_bent_ao
        );
    }

    prev_irradiance_acc /= valid_weight_sum;
    prev_bent_normal_acc /= valid_weight_sum;
    prev_ao_acc /= valid_weight_sum;
    prev_ssr_radiance_acc /= valid_weight_sum;
    prev_first_moment_acc /= valid_weight_sum;
    prev_second_moment_acc /= valid_weight_sum;
    prev_history_length_acc /= valid_weight_sum;
    prev_ssr_first_moment_acc /= valid_weight_sum;
    prev_ssr_second_moment_acc /= valid_weight_sum;
    prev_ssr_history_length_acc /= valid_weight_sum;

    let blend_factor = 1.0 / (1.0 + min(prev_history_length_acc, MAX_HISTORY_LENGTH));
    let ssr_blend_factor = 1.0 / (1.0 + min(prev_ssr_history_length_acc, MAX_HISTORY_LENGTH));

    let final_irradiance = mix(prev_irradiance_acc, curr_ssgi.rgb, blend_factor);
    let curr_bent_normal = safe_normalize3(curr_bent_ao.xyz, curr_normal);
    let prev_bent_normal = safe_normalize3(prev_bent_normal_acc, curr_bent_normal);
    let final_bent_normal = safe_normalize3(mix(prev_bent_normal, curr_bent_normal, blend_factor), curr_bent_normal);
    let final_ao = mix(prev_ao_acc, curr_bent_ao.w, blend_factor);
    let final_ssr_radiance = mix(prev_ssr_radiance_acc, curr_ssr.rgb, ssr_blend_factor);

    // 4.2 Variance estimation
    let curr_lum = luminance(curr_ssgi.rgb);
    let first_moment = mix(prev_first_moment_acc, curr_lum, blend_factor);
    let second_moment = mix(prev_second_moment_acc, curr_lum * curr_lum, blend_factor);

    let curr_ssr_lum = luminance(curr_ssr.rgb);
    let ssr_first_moment = mix(prev_ssr_first_moment_acc, curr_ssr_lum, ssr_blend_factor);
    let ssr_second_moment = mix(prev_ssr_second_moment_acc, curr_ssr_lum * curr_ssr_lum, ssr_blend_factor);

    return FragmentOutput(
        vec4f(final_irradiance, prev_history_length_acc + 1.0),
        vec4f(final_ssr_radiance, prev_ssr_history_length_acc + 1.0),
        vec4f(first_moment, second_moment, ssr_first_moment, ssr_second_moment),
        vec4f(final_bent_normal, final_ao),
    );
}

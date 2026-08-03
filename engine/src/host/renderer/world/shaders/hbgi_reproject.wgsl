// https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/

@group(0) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;

@group(1) @binding(0) var motion_vectors_texture: texture_2d<f32>;
@group(1) @binding(1) var curr_bent_ao_texture: texture_2d<f32>;
@group(1) @binding(2) var prev_bent_ao_reproject_texture: texture_2d<f32>;
@group(1) @binding(3) var current_depth_texture: texture_depth_2d;
@group(1) @binding(4) var current_normal_texture: texture_2d<f32>;
@group(1) @binding(5) var prev_depth_history_texture: texture_2d<f32>;
@group(1) @binding(6) var prev_normal_history_texture: texture_2d<f32>;
@group(1) @binding(7) var curr_hbgi_irradiance_texture: texture_2d<f32>;
@group(1) @binding(8) var prev_hbgi_irradiance_reproject_texture: texture_2d<f32>;

struct HbgiReprojectUniform {
    prev_inverse_view_proj: mat4x4<f32>,
}

@group(2) @binding(0) var<uniform> hbgi_reproject_settings: HbgiReprojectUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) hbgi_reproject: vec4<f32>,
    @location(1) depth_history: vec4<f32>,
    @location(2) normal_history: vec4<f32>,
    @location(3) hbgi_irradiance_reproject: vec4<f32>,
}

const MAX_HISTORY_LENGTH: f32 = 32.0;
const SVGF_NORMAL_REJECTION_DOT_THRESHOLD: f32 = 0.9;
const SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD: f32 = 0.3;

fn safe_normalize3(v: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3f(0.0);
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

    // Reduced resolution history texture coordinates
    let half_dims_u = textureDimensions(curr_bent_ao_texture);
    let half_dims = vec2f(half_dims_u);
    let half_max_coords = vec2i(half_dims_u) - vec2i(1);
    let curr_half_coords = min(vec2i(curr_uv * half_dims), half_max_coords);

    // Load current frame textures
    let curr_depth = textureLoad(current_depth_texture, curr_full_coords, 0);
    let curr_normal_sample = textureLoad(current_normal_texture, curr_full_coords, 0);
    let curr_bent_ao = textureLoad(curr_bent_ao_texture, curr_half_coords, 0);
    let curr_hbgi_irradiance = textureLoad(curr_hbgi_irradiance_texture, curr_half_coords, 0);

    // depth gets cleared to zero -> zero indicates an invalid value
    if (curr_depth <= 0.0) {
        let curr_lum = luminance(curr_hbgi_irradiance.rgb);
        return FragmentOutput(
            curr_bent_ao,
            vec4f(curr_depth, curr_lum, curr_lum * curr_lum, 1.0),
            curr_normal_sample,
            curr_hbgi_irradiance
        );
    }

    let curr_world_position = reconstruct_world_position_from_depth(
        curr_uv,
        curr_depth,
        inverse_view_proj
    );
    let curr_normal = normalize(curr_normal_sample.xyz);

    let motion = textureLoad(motion_vectors_texture, curr_full_coords, 0).xy;
    let prev_uv = curr_uv - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        let curr_lum = luminance(curr_hbgi_irradiance.rgb);
        return FragmentOutput(
            curr_bent_ao,
            vec4f(curr_depth, curr_lum, curr_lum * curr_lum, 1.0),
            curr_normal_sample,
            curr_hbgi_irradiance
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
        hbgi_reproject_settings.prev_inverse_view_proj
    );
    let prev_normal = normalize(textureLoad(prev_normal_history_texture, vec2i(prev_coord), 0).xyz);

    let dist_test = distance(prev_world_position, curr_world_position) <= 1.0;

    let delta = prev_world_position - curr_world_position;
    let plane_distance = abs(dot(curr_normal, delta));
    let plane_test = plane_distance <= 0.3;
    let normal_test = dot(curr_normal, prev_normal) >= 0.90;

    if normal_test {
        return current_frame_output(
            vec4f(1.0),
            curr_hbgi_irradiance,
            curr_depth,
            curr_normal_sample
        );
    } else {
        return current_frame_output(
            vec4f(0.0),
            curr_hbgi_irradiance,
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
    var prev_first_moment_acc = 0.0;
    var prev_second_moment_acc = 0.0;
    var prev_history_length_acc = 0.0;
    var valid_weight_sum = 0.0;
    for (var i = 0; i < 4; i++) {
        let coords = clamp_coord(taps[i], half_max_coords);
        // again, we want uv to point to the middle of a pixel, so we need to re-add the 0.5 we shifted by earlier
        let tap_uv = (vec2f(coords) + vec2f(0.5)) / half_dims;
        let tap_weight = tap_weights[i];

        let prev_depth_history = textureLoad(prev_depth_history_texture, coords, 0);
        let prev_depth = prev_depth_history.x;
        if (prev_depth <= 0.0) {
            continue;
        }
        let prev_normal = normalize(textureLoad(prev_normal_history_texture, coords, 0).xyz);
        let prev_world_position = reconstruct_world_position_from_depth(
            tap_uv,
            prev_depth,
            hbgi_reproject_settings.prev_inverse_view_proj
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

        let prev_bent_ao = textureLoad(prev_bent_ao_reproject_texture, coords, 0);
        let prev_irradiance = textureLoad(
            prev_hbgi_irradiance_reproject_texture,
            coords,
            0
        ).rgb;

        prev_irradiance_acc += prev_irradiance * tap_weight;
        prev_bent_normal_acc += prev_bent_ao.xyz * tap_weight;
        prev_ao_acc += prev_bent_ao.w * tap_weight;
        prev_first_moment_acc += prev_depth_history.y * tap_weight;
        prev_second_moment_acc += prev_depth_history.z * tap_weight;
        prev_history_length_acc += prev_depth_history.w * tap_weight;
        valid_weight_sum += tap_weight;
    }

    if (valid_weight_sum <= 0.0) {
        // TODO SVGF paper describes fallback to 3x3 filter
        let curr_lum = luminance(curr_hbgi_irradiance.rgb);
        return FragmentOutput(
            curr_bent_ao,
            vec4f(curr_depth, curr_lum, curr_lum * curr_lum, 1.0),
            curr_normal_sample,
            curr_hbgi_irradiance
        );
    }

    prev_irradiance_acc /= valid_weight_sum;
    prev_bent_normal_acc /= valid_weight_sum;
    prev_ao_acc /= valid_weight_sum;
    prev_first_moment_acc /= valid_weight_sum;
    prev_second_moment_acc /= valid_weight_sum;
    prev_history_length_acc /= valid_weight_sum;

    let blend_factor = 1.0 / (1.0 + min(prev_history_length_acc, MAX_HISTORY_LENGTH));

    let final_bent_normal = mix(normalize(prev_bent_normal_acc), curr_bent_ao.xyz, blend_factor);
    let final_ao = mix(prev_ao_acc, curr_bent_ao.w, blend_factor);
    let final_irradiance = mix(prev_irradiance_acc, curr_hbgi_irradiance.rgb, blend_factor);

    // 4.2 Variance estimation
    let curr_lum = luminance(curr_hbgi_irradiance.rgb);
    let first_moment = mix(prev_first_moment_acc, curr_lum, blend_factor);
    let second_moment = mix(prev_second_moment_acc, curr_lum * curr_lum, blend_factor);

    return FragmentOutput(
        vec4f(final_bent_normal, final_ao),
        vec4f(curr_depth, first_moment, second_moment, prev_history_length_acc + 1.0),
        curr_normal_sample,
        vec4f(final_irradiance, curr_hbgi_irradiance.a),
    );
}

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

const TEMPORAL_RESPONSE: f32 = 0.2;
const HISTORY_CLAMP_WEIGHT: f32 = 0.25;
const SVGF_NORMAL_REJECTION_DOT_THRESHOLD: f32 = 0.95;
const SVGF_WORLD_DISTANCE_REJECTION_THRESHOLD: f32 = 0.01;

fn safe_normalize3(v: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3f(0.0);
}

fn current_frame_output(
    hbgi: vec4f,
    hbgi_irradiance: vec4f,
    depth: f32,
    normal: vec4f,
) -> FragmentOutput {
    let curr_lum = luminance(hbgi_irradiance.rgb);
    return FragmentOutput(
        hbgi,
        vec4f(depth, curr_lum, curr_lum * curr_lum, 1.0),
        normal,
        hbgi_irradiance
    );
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
    // Full resolution texture coordinates
    let full_dims_u = textureDimensions(current_depth_texture);
    let full_dims = vec2f(full_dims_u);
    let full_max_coord = vec2i(full_dims_u) - vec2i(1);
    let full_coord = clamp_coord(vec2i(in.tex_coords * full_dims), full_max_coord);

    // Reduced resolution history texture coordinates
    let history_dims_u = textureDimensions(curr_bent_ao_texture);
    let history_dims = vec2f(history_dims_u);
    let history_max_coord = vec2i(history_dims_u) - vec2i(1);
    let history_coord = clamp_coord(vec2i(in.tex_coords * history_dims), history_max_coord);

    // Load current frame textures
    let curr_depth = textureLoad(current_depth_texture, full_coord, 0);
    let curr_normal_sample = textureLoad(current_normal_texture, full_coord, 0);
    let curr_bent_ao = textureLoad(curr_bent_ao_texture, history_coord, 0);
    let curr_hbgi_irradiance = textureLoad(curr_hbgi_irradiance_texture, history_coord, 0);

    // depth gets cleared to zero -> zero indicates an invalid value
    if (curr_depth <= 0.0) {
        return current_frame_output(
            curr_bent_ao,
            curr_hbgi_irradiance,
            curr_depth,
            curr_normal_sample
        );
    }

    let curr_world_position = reconstruct_world_position_from_depth(
        in.tex_coords,
        curr_depth,
        inverse_view_proj
    );
    let curr_normal = normalize(curr_normal_sample.xyz);

    let motion = textureLoad(motion_vectors_texture, full_coord, 0).xy;
    let prev_uv = in.tex_coords - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return current_frame_output(
            curr_bent_ao,
            curr_hbgi_irradiance,
            curr_depth,
            curr_normal_sample
        );
    }
    let prev_coord = clamp_coordf(prev_uv * history_dims, vec2f(history_max_coord));
    let prev_coord_floor = floor(prev_coord);
    let prev_coord_frac = fract(prev_coord);

    // Color clamping (not originally in SVGF)
    var min_curr_hbgi_irradiance = curr_hbgi_irradiance.rgb;
    var max_curr_hbgi_irradiance = curr_hbgi_irradiance.rgb;
    for (var sample_idx: i32 = 0; sample_idx < 3; sample_idx += 1) {
        let offset = array<vec2i, 3>(vec2i(1, 0), vec2i(0, 1), vec2i(1, 1))[sample_idx];
        let curr_hbgi_irradiance_sample = textureLoad(
            curr_hbgi_irradiance_texture,
            clamp_coord(history_coord + offset, history_max_coord),
            0
        ).rgb;
        min_curr_hbgi_irradiance = min(min_curr_hbgi_irradiance, curr_hbgi_irradiance_sample);
        max_curr_hbgi_irradiance = max(max_curr_hbgi_irradiance, curr_hbgi_irradiance_sample);
    }

    // 4.1 Temporal filtering
    // 2x2 tap bilinear filter described by the SVGF paper
    let taps = array(
        vec2i(prev_coord_floor),
        vec2i(prev_coord_floor) + vec2i(0, 1),
        vec2i(prev_coord_floor) + vec2i(1, 0),
        vec2i(prev_coord_floor) + vec2i(1, 1),
    );
    let tap_weights = array(
        (1.0 - prev_coord_frac.x) * (1.0 - prev_coord_frac.y),
        (1.0 - prev_coord_frac.x) * prev_coord_frac.y,
        prev_coord_frac.x * (1.0 - prev_coord_frac.y),
        prev_coord_frac.x * prev_coord_frac.y,
    );

    var prev_irradiance_acc = vec3f(0.0);
    var prev_bent_normal_acc = vec3f(0.0);
    var prev_ao_acc = 0.0;
    var prev_first_moment_acc = 0.0;
    var prev_second_moment_acc = 0.0;
    var valid_weight_sum = 0.0;
    for (var i = 0; i < 4; i++) {
        let coords = clamp_coord(taps[i], history_max_coord);
        let tap_uv = (vec2f(coords) + vec2f(0.5)) / history_dims;
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
        if (
            dot(curr_normal, prev_normal) <= SVGF_NORMAL_REJECTION_DOT_THRESHOLD
            || distance(curr_world_position, prev_world_position)
                >= SVGF_WORLD_DISTANCE_REJECTION_THRESHOLD
        ) {
            continue;
        }

        let prev_bent_ao = textureLoad(prev_bent_ao_reproject_texture, coords, 0);
        let prev_irradiance = textureLoad(
            prev_hbgi_irradiance_reproject_texture,
            coords,
            0
        ).rgb;
        let clamped_prev_irradiance = clamp(
            prev_irradiance,
            min_curr_hbgi_irradiance,
            max_curr_hbgi_irradiance
        );
        let filtered_prev_irradiance = mix(
            prev_irradiance,
            clamped_prev_irradiance,
            HISTORY_CLAMP_WEIGHT
        );

        prev_irradiance_acc += filtered_prev_irradiance * tap_weight;
        prev_bent_normal_acc += prev_bent_ao.xyz * tap_weight;
        prev_ao_acc += prev_bent_ao.w * tap_weight;
        prev_first_moment_acc += prev_depth_history.y * tap_weight;
        prev_second_moment_acc += prev_depth_history.z * tap_weight;
        valid_weight_sum += tap_weight;
    }
    if (valid_weight_sum <= 0.0) {
        // TODO SVGF paper describes fallback to 3x3 filter
        return current_frame_output(
            curr_bent_ao,
            curr_hbgi_irradiance,
            curr_depth,
            curr_normal_sample
        );
    }
    prev_irradiance_acc /= valid_weight_sum;
    prev_bent_normal_acc /= valid_weight_sum;
    prev_ao_acc /= valid_weight_sum;
    prev_first_moment_acc /= valid_weight_sum;
    prev_second_moment_acc /= valid_weight_sum;

    let final_bent_normal = mix(normalize(prev_bent_normal_acc), curr_bent_ao.xyz, TEMPORAL_RESPONSE);
    let final_ao = mix(prev_ao_acc, curr_bent_ao.w, TEMPORAL_RESPONSE);
    let final_irradiance = mix(prev_irradiance_acc, curr_hbgi_irradiance.rgb, TEMPORAL_RESPONSE);

    // 4.2 Variance estimation
    let curr_lum = luminance(curr_hbgi_irradiance.rgb);
    let first_moment = mix(prev_first_moment_acc, curr_lum, TEMPORAL_RESPONSE);
    let second_moment = mix(prev_second_moment_acc, curr_lum * curr_lum, TEMPORAL_RESPONSE);

    // TODO add history length and variance estimate in limited history cases (last paragraph of 4.2)

    return FragmentOutput(
        vec4f(final_bent_normal, final_ao),
        vec4f(curr_depth, first_moment, second_moment, 1.0),
        curr_normal_sample,
        vec4f(final_irradiance, curr_hbgi_irradiance.a),
    );
}

@group(0) @binding(1) var<uniform> camera_pos: vec3<f32>;
@group(0) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(0) @binding(4) var<uniform> camera_view_rotation: mat3x3<f32>;

@group(1) @binding(0) var hbgi_pyramid_texture: texture_2d<f32>;
@group(1) @binding(1) var hbgi_pyramid_sampler: sampler;
@group(1) @binding(2) var hbgi_depth_pyramid_texture: texture_2d<f32>;
@group(1) @binding(3) var hbgi_normal_pyramid_texture: texture_2d<f32>;
@group(1) @binding(4) var hbgi_normal_pyramid_sampler: sampler;
@group(1) @binding(5) var hbgi_blue_noise_texture: texture_2d<f32>;

struct HbgiSettingsUniform {
    params0: vec4<f32>,
    params1: vec4<f32>,
}

@group(2) @binding(0) var<uniform> hbgi_settings: HbgiSettingsUniform;

@group(3) @binding(4) var env_di_texture: texture_cube<f32>;
@group(3) @binding(5) var env_di_texture_sampler: sampler;
@group(3) @binding(8) var<uniform> environment_map_intensity: f32;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) hbgi: vec4<f32>,
    @location(1) hbgi_irradiance: vec4<f32>,
}

const PI: f32 = 3.14159265358979323846;
const DIRECTIONS: u32 = 4u;
const STEPS_PER_DIRECTION: u32 = 8u;

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

fn safe_normalize2(v: vec2<f32>) -> vec2<f32> {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec2<f32>(0.0);
}

fn safe_normalize3(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3<f32>(0.0);
}

fn sanitize_rgb(v: vec3f) -> vec3f {
    let cap = 100.0;

    // WGSL NaN check: NaN != NaN
    let finiteish = all(v == v) && all(abs(v) < vec3f(1e20));

    return select(
        vec3f(0.0),
        clamp(v, vec3f(0.0), vec3f(cap)),
        finiteish && all(v >= vec3f(0.0))
    );
}

// Interleaved gradient function from Jimenez 2014 http://goo.gl/eomGso
// https://blog.demofox.org/2022/01/01/interleaved-gradient-noise-a-different-kind-of-low-discrepancy-sequence
fn gradient_noise(position: vec2f) -> f32 {
    return fract(52.9829189 * fract(dot(position, vec2f(0.06711056, 0.00583715))));
}

fn sample_blue_noise(pixel_coords: vec2u) -> vec2f {
    let noise_dims = textureDimensions(hbgi_blue_noise_texture, 0);
    let noise_coord = vec2u(
        pixel_coords.x % noise_dims.x,
        pixel_coords.y % noise_dims.y
    );
    return textureLoad(hbgi_blue_noise_texture, vec2i(noise_coord), 0).rg;
}

fn fast_acos(x: f32) -> f32 {
    var out_val = -0.156583 * abs(x) + (PI * 0.5);
    out_val *= sqrt(1.0 - abs(x));
    if (x >= 0.0) {
        return out_val;
    }
    return PI - out_val;
}

fn sample_hbgi_pyramid(uv: vec2f, lod: f32) -> vec3f {
    return sanitize_rgb(textureSampleLevel(
        hbgi_pyramid_texture,
        hbgi_pyramid_sampler,
        uv,
        lod
    ).rgb);
}

fn sample_hbgi_depth(uv: vec2f, lod: f32) -> f32 {
    let mip_level = i32(lod);
    let dims = vec2f(textureDimensions(hbgi_depth_pyramid_texture, mip_level));
    let max_uv = vec2f(1.0) - 1.0 / dims;
    let clamped_uv = clamp(uv, vec2f(0.0), max_uv);
    let coord = vec2i(clamped_uv * dims);
    return textureLoad(hbgi_depth_pyramid_texture, coord, mip_level).x;
}

fn sample_hbgi_normal(uv: vec2f, lod: f32) -> vec3f {
    return textureSampleLevel(
        hbgi_normal_pyramid_texture,
        hbgi_normal_pyramid_sampler,
        uv,
        lod
    ).xyz;
}

fn reconstruct_world_position_from_depth(uv: vec2f, depth: f32) -> vec3f {
    let clip = vec4f(
        uv.x * 2.0 - 1.0,
        1.0 - uv.y * 2.0,
        depth,
        1.0
    );
    let world = inverse_view_proj * clip;
    if (abs(world.w) > 1e-8) {
        return world.xyz / world.w;
    }
    return vec3f(0.0);
}

fn sample_world_position_from_depth(uv: vec2f, lod: f32) -> vec4f {
    let dims = vec2f(textureDimensions(hbgi_depth_pyramid_texture, i32(lod)));
    let max_uv = vec2f(1.0) - 1.0 / dims;
    let clamped_uv = clamp(uv, vec2f(0.0), max_uv);
    let depth = sample_hbgi_depth(clamped_uv, lod);

    if (depth <= 0.0) {
        return vec4f(0.0);
    }

    return vec4f(reconstruct_world_position_from_depth(clamped_uv, depth), 1.0);
}

fn distance_to_screen_edge(uv: vec2f) -> f32 {
    let edge_distance = min(uv, vec2f(1.0) - uv);
    return min(edge_distance.x, edge_distance.y);
}

// heuristic for the probability of a random ray to get blocked by the horizon
fn blocking_heuristic(radius_w: f32, dist_w: f32, omega_o_w: vec3f, sample_normal_w: vec3f, uv: vec2f) -> f32 {
    // samples from further away are less reliable
    let world_distance_heuristic = min(radius_w / max(dist_w, 0.0001), 1.0);

    // samples from grazing angles are slightly less likely to represent thick surfaces
    let normal_heuristic = pow(abs(dot(omega_o_w, sample_normal_w)), 0.1);

    // samples near the edge of the screen are less reliable
    let edge_distance = distance_to_screen_edge(uv);
    let screen_edge_heuristic = smoothstep(0.0, 0.1, edge_distance);

    return world_distance_heuristic * normal_heuristic;
}

/**
    Based on Horizon-Based Global Illumination (HBGI) / Horizon-Based Indirect Lighting (HBIL) - Benoit Mayaux
    https://github.com/Patapom/GodComplex/blob/master/Tests/TestHBIL/Shaders/HBIL/HBIL.hlsl
    also SSRT3 https://github.com/cdrinmatane/SSRT3/blob/main/HDRP/Shaders/Resources/SSRTCS.compute
    -------------------------------------------------------------------------------------------------

    My own modification keeps track of an upper and lower horizon, such that
    - any ray entering below the upper horizon has some probability of being blocked
    - any ray entering below the lower horizon is definitely blocked
*/
fn shbgi(in: VertexOutput) -> FragmentOutput {
    let radius_pixels = hbgi_settings.params0.x;
    let radius_world = hbgi_settings.params0.y;
    let step_size_exponent = hbgi_settings.params0.z;
    let gi_intensity = hbgi_settings.params0.w;
    let temporal_rotation = hbgi_settings.params1.x;
    let temporal_offset = hbgi_settings.params1.y;

    let uv = in.tex_coords;

    let P_sample = sample_world_position_from_depth(uv, 0.0);
    if (P_sample.w < 0.5) {
        // invalid sample
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0), vec4<f32>(0.0));
    }
    let P_w = P_sample.xyz;

    let normal_sample = sample_hbgi_normal(uv, 0.0);

    let dims_u = textureDimensions(hbgi_depth_pyramid_texture, 0);
    let dims = vec2<f32>(dims_u);
    let max_hbgi_pyramid_lod = f32(textureNumLevels(hbgi_pyramid_texture) - 1u);

    let pixel_coords = vec2u(floor(uv * dims));
    let spatial_noise = sample_blue_noise(pixel_coords);
    let rotation_jitter = fract(temporal_rotation + spatial_noise.x);
    let offset_jitter = fract(temporal_offset + spatial_noise.y);

    /*
    let spatial_noise = gradient_noise(uv * dims);
    let spatial_rotation = spatial_noise;
    let spatial_offset = spatial_noise;
    let rotation_jitter = fract(temporal_rotation + spatial_rotation);
    let offset_jitter = fract(temporal_offset + spatial_offset);
    */

    let max_step_dist = radius_pixels / dims;

    // 1.1. Camera Spaces ---
    let X_w = camera_view_rotation[0];
    let Y_w = camera_view_rotation[1];
    let Z_w = camera_view_rotation[2];

    let omega_o_w = safe_normalize3(camera_pos - P_w);
    let omega_x_w = safe_normalize3(cross(Y_w, omega_o_w));
    let omega_y_w = cross(omega_o_w, omega_x_w);
    // ---

    // view facing normal:
    let n_w = select(omega_o_w, normal_sample, all(normal_sample == normal_sample));

    var visibility_acc = 0.0;
    var bent_acc_w = vec3f(0.0);
    var irradiance_acc = vec3f(0.0);
    var debug = 0.0;

    for (var dir_idx: u32 = 0u; dir_idx < DIRECTIONS; dir_idx += 1u) {
        // rotate around full hemisphere
        let phi =
            (f32(dir_idx) + rotation_jitter) * ((2 * PI) / f32(DIRECTIONS));

        // 1.2. Slice Space ---
        let D_w = cos(phi) * omega_x_w + sin(phi) * omega_y_w;
        // w = world space
        // cs = local camera space
        // ss = slice space
        let D_cs = vec3f(cos(phi), sin(phi), 0.0);
        let omega_o_cs = vec3f(0.0, 0.0, 1.0);
        let D_ss = vec2f(1.0, 0.0);
        let omega_o_ss = vec2f(0.0, 1.0);
        let x_ss = vec2f(0.0, 0.0);
        let n_ss = vec2f(dot(n_w, D_w), dot(n_w, omega_o_w));
        // ---

        let max_step_cs = vec2f(D_cs.x, -D_cs.y) * max_step_dist;

        // HORIZON INITIALIZER
        // based on my own horizon initialization drawing...
        let k_ss = safe_normalize2(vec2f(n_ss.y, -n_ss.x));
        var theta_init = fast_acos(clamp(dot(omega_o_ss, k_ss), -1.0, 1.0));

        var fallback_radiance = vec3f(0.0);

        var lower_theta = theta_init; // no rays pass below the lower horizon
        var upper_theta = theta_init; // some rays pass below the upper horizon
        var p_upper = 1.0; // probability that a random ray getting blocked by the upper horizon

        for (var step_idx: u32 = 1u; step_idx <= STEPS_PER_DIRECTION; step_idx += 1u) {
            let step_t = (f32(step_idx) + offset_jitter) / f32(STEPS_PER_DIRECTION);
            let current_step_cs = pow(step_t, step_size_exponent) * max_step_cs;
            let sample_distance_pixels = max(length(current_step_cs * dims), 1.0);
            // similar simple heuristic: https://github.com/cdrinmatane/SSRT3/blob/main/HDRP/Shaders/Resources/SSRTCS.compute
            // TODO play around with this heuristic for best results relative to step count

            let hbgi_pyramid_lod = f32(min((step_idx + 2) / 2, 6));

            let sample_uv = uv + current_step_cs;
            let x_1_sample = sample_world_position_from_depth(sample_uv, hbgi_pyramid_lod);

            if (x_1_sample.w > 0.5) {
                let x_1_w = x_1_sample.xyz;
                let to_sample = x_1_w - P_w;
                let sample_dir_w = safe_normalize3(to_sample);

                // Project to slice space
                let sample_dir_ss = safe_normalize2(
                    vec2f(
                        dot(sample_dir_w, D_w),
                        dot(sample_dir_w, omega_o_w)
                    )
                );

                let current_theta = atan2(sample_dir_ss.x, sample_dir_ss.y);
                if (current_theta >= lower_theta) {
                    // current horizon is below lower horizon => skip
                    continue;
                }

                let L_d = sample_hbgi_pyramid(sample_uv, hbgi_pyramid_lod);

                // accumulate radiance between lower_horizon..current_horizon
                // equation 18
                let theta_0 = current_theta;
                let theta_1 = lower_theta;
                let cos_theta_0 = cos(theta_0);
                let cos_theta_1 = cos(theta_1);
                let cos2_theta_0 = cos_theta_0 * cos_theta_0;
                let cos2_theta_1 = cos_theta_1 * cos_theta_1;
                let sin_theta_0 = sin(theta_0);
                let sin_theta_1 = sin(theta_1);

                let left = n_ss.x * 0.5 * (
                    theta_1 - theta_0
                    + sin_theta_0 * cos_theta_0
                    - sin_theta_1 * cos_theta_1
                );
                let right = n_ss.y * 0.5 * (cos2_theta_0 - cos2_theta_1);

                // 3.4.
                let x_1_normal_w = sample_hbgi_normal(sample_uv, hbgi_pyramid_lod);
                let t = smoothstep(0.0, 1.0, dot(sample_dir_w, -x_1_normal_w));
                let L_new = mix(fallback_radiance, L_d, t);

                let p_current = blocking_heuristic(radius_world, length(to_sample), omega_o_w, x_1_normal_w, sample_uv);

                // equation 19
                let integral = abs(left + right);
                irradiance_acc += pow(L_new * integral * p_current, 1.0 / vec3f(gi_intensity));

                if (t > 0.0) {
                    fallback_radiance = L_d;
                }

                // Horizon update:
                if (current_theta >= upper_theta) {
                    // current horizon is between lower/upper horizons
                    let p = p_upper + p_current;
                    if (p >= 1) {
                        lower_theta = current_theta;
                    } else {
                        // raise lower horizon by assuming that the blocked part of the angular segment is near the lower horizon
                        lower_theta -= p * (lower_theta - current_theta);
                    }
                } else {
                    // current horizon is above the upper horizon
                    let p = p_upper + p_current;
                    if (p >= 1) {
                        lower_theta = upper_theta;
                    } else {
                        lower_theta -= p * (lower_theta - upper_theta);
                    }
                    upper_theta = current_theta;
                    p_upper = p_current;
                }
            }
        }

        var bent_contrib_w: vec3f;
        var visibility_contrib: f32;
        {
            let theta_0 = 0.0;
            let theta_1 = upper_theta;
            let cos_theta_0 = cos(theta_0);
            let cos_theta_1 = cos(theta_1);
            let cos2_theta_0 = cos_theta_0 * cos_theta_0;
            let cos2_theta_1 = cos_theta_1 * cos_theta_1;
            let sin_theta_0 = sin(theta_0);
            let sin_theta_1 = sin(theta_1);

            let n_bent_x = 0.5 * (
                theta_1 + theta_0
                - sin_theta_1 * cos_theta_1
                - sin_theta_0 * cos_theta_0
            );
            let n_bent_y = 0.5 * (2.0 - cos2_theta_0 - cos2_theta_1);

            bent_contrib_w += D_w * n_bent_x + omega_o_w * n_bent_y;
            visibility_contrib += n_ss.x * n_bent_x + n_ss.y * n_bent_y;
        }

        {
            let theta_0 = upper_theta;
            let theta_1 = lower_theta;
            let cos_theta_0 = cos(theta_0);
            let cos_theta_1 = cos(theta_1);
            let cos2_theta_0 = cos_theta_0 * cos_theta_0;
            let cos2_theta_1 = cos_theta_1 * cos_theta_1;
            let sin_theta_0 = sin(theta_0);
            let sin_theta_1 = sin(theta_1);

            let n_bent_x = 0.5 * (
                theta_1 + theta_0
                - sin_theta_1 * cos_theta_1
                - sin_theta_0 * cos_theta_0
            );
            let n_bent_y = 0.5 * (2.0 - cos2_theta_0 - cos2_theta_1);

            bent_contrib_w += (D_w * n_bent_x + omega_o_w * n_bent_y) * (1.0 - p_upper);
            visibility_contrib += (n_ss.x * n_bent_x + n_ss.y * n_bent_y) * (1.0 - p_upper);
        }

        bent_acc_w += bent_contrib_w;

        // 2.2.2. equation 11
        visibility_acc += visibility_contrib;
    }

    let S = f32(DIRECTIONS);
    //let ao = (1.0 / S) * visibility_acc;
    let ao = clamp((1.0 / S) * visibility_acc, 0.0, 1.0);
    let bent_n_w = safe_normalize3(bent_acc_w);

    let far_field_sample = textureSample(
        env_di_texture,
        env_di_texture_sampler,
        bent_n_w
    );

    let E_far = far_field_sample.rgb * ao * environment_map_intensity;
    let E_near = sanitize_rgb(irradiance_acc * (PI / S));

    return FragmentOutput(vec4f(bent_n_w, ao), vec4f(E_near + E_far, debug));
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    return shbgi(in);
}

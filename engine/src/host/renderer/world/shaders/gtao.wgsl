@group(0) @binding(1) var<uniform> camera_pos: vec3<f32>;

@group(1) @binding(0) var gbuffer_normal_roughness: texture_2d<f32>;
@group(1) @binding(1) var gbuffer_normal_roughness_sampler: sampler;
@group(1) @binding(2) var gbuffer_world_position: texture_2d<f32>;
@group(1) @binding(3) var gbuffer_world_position_sampler: sampler;
@group(1) @binding(4) var gi_source_history_texture: texture_2d<f32>;
@group(1) @binding(5) var gi_source_history_sampler: sampler;
@group(2) @binding(0) var<uniform> gtao_settings: vec4<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) gtao: vec4<f32>,
    @location(1) hbil_diffuse_irradiance: vec4<f32>,
}

const PI: f32 = 3.14159265358979323846;
const DIRECTIONS: u32 = 8u;
const STEPS_PER_DIRECTION: u32 = 4u;

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

fn safe_normalize(v: vec3<f32>) -> vec3<f32> {
    let len2 = dot(v, v);
    if (len2 < 1e-8) {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    return v * inverseSqrt(len2);
}

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

// Primitive of the cosine-weighted horizon slice integral.
fn integrate_arc_cos_weighted(h: f32, n: f32) -> f32 {
    return 0.25 * (-cos(2.0 * h - n) + cos(n) + 2.0 * h * sin(n));
}

fn clamped_slice_interval(
    horizon_angle_bwd: f32,
    horizon_angle_fwd: f32,
    slice_tangent: vec3<f32>,
    view_dir: vec3<f32>,
    normal: vec3<f32>
) -> vec4<f32> {
    let nx = dot(normal, slice_tangent);
    let ny = dot(normal, view_dir);
    let proj_n = vec2<f32>(nx, ny);
    let proj_n_len = length(proj_n);

    if (proj_n_len < 1e-5) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let n_angle = atan2(ny, nx);
    let min_angle = n_angle - 0.5 * PI;
    let max_angle = n_angle + 0.5 * PI;
    let h1 = clamp(horizon_angle_bwd, min_angle, max_angle);
    let h2 = clamp(horizon_angle_fwd, min_angle, max_angle);

    if (h2 <= h1) {
        return vec4<f32>(0.0, 0.0, proj_n_len, 0.0);
    }

    return vec4<f32>(h1, h2, proj_n_len, n_angle);
}

fn evaluate_slice_visibility(
    horizon_angle_bwd: f32,
    horizon_angle_fwd: f32,
    slice_tangent: vec3<f32>,
    view_dir: vec3<f32>,
    normal: vec3<f32>
) -> f32 {
    let interval = clamped_slice_interval(
        horizon_angle_bwd,
        horizon_angle_fwd,
        slice_tangent,
        view_dir,
        normal,
    );
    let h1 = interval.x;
    let h2 = interval.y;
    let proj_n_len = interval.z;
    let n_angle = interval.w;

    if (proj_n_len < 1e-5 || h2 <= h1) {
        return 0.0;
    }

    let arc =
        integrate_arc_cos_weighted(h2, n_angle) -
        integrate_arc_cos_weighted(h1, n_angle);

    return max(0.0, proj_n_len * arc);
}

fn evaluate_slice_bent_contribution(
    horizon_angle_bwd: f32,
    horizon_angle_fwd: f32,
    slice_tangent: vec3<f32>,
    view_dir: vec3<f32>,
    normal: vec3<f32>
) -> vec3<f32> {
    // Horizon search stores angles from the slice tangent axis. Convert them to the
    // paper's slice-space convention, where theta = 0 lies on the view axis.
    let theta0 = horizon_angle_bwd - 0.5 * PI;
    let theta1 = 0.5 * PI - horizon_angle_fwd;

    if (theta1 <= theta0) {
        return vec3<f32>(0.0);
    }

    let bent_tangent =
        0.5 * (
            theta1 - theta0 +
            sin(theta0) * cos(theta0) -
            sin(theta1) * cos(theta1)
        );
    let bent_view =
        0.5 * (
            2.0 -
            cos(theta0) * cos(theta0) -
            cos(theta1) * cos(theta1)
        );

    return
        slice_tangent * bent_tangent +
        view_dir * bent_view;
}

fn initialize_horizon_angles(
    slice_tangent: vec3<f32>,
    view_dir: vec3<f32>,
    normal: vec3<f32>
) -> vec2<f32> {
    let nx = dot(normal, slice_tangent);
    let ny = dot(normal, view_dir);

    if (abs(nx) < 1e-5 && abs(ny) < 1e-5) {
        return vec2<f32>(-0.5 * PI, -0.5 * PI);
    }

    let horizon_bwd = clamp(atan2(nx, ny), -0.5 * PI, 0.5 * PI);
    let horizon_fwd = clamp(atan2(-nx, ny), -0.5 * PI, 0.5 * PI);
    return vec2<f32>(horizon_bwd, horizon_fwd);
}

// HBIL integrates the radiance gathered when the horizon rises over a slice interval.
fn evaluate_hbil_interval(
    prev_horizon_angle: f32,
    new_horizon_angle: f32,
    slice_tangent: vec3<f32>,
    view_dir: vec3<f32>,
    normal: vec3<f32>
) -> f32 {
    let nx = dot(normal, slice_tangent);
    let ny = dot(normal, view_dir);
    let proj_n = vec2<f32>(nx, ny);

    if (length(proj_n) < 1e-5) {
        return 0.0;
    }

    let n_angle = atan2(ny, nx);
    let min_angle = n_angle - 0.5 * PI;
    let max_angle = n_angle + 0.5 * PI;
    let theta0 = clamp(prev_horizon_angle, min_angle, max_angle);
    let theta1 = clamp(new_horizon_angle, min_angle, max_angle);

    if (theta1 <= theta0) {
        return 0.0;
    }

    let term_x =
        0.5 * (
            theta1 - theta0 +
            sin(theta0) * cos(theta0) -
            sin(theta1) * cos(theta1)
        );
    let term_y =
        0.5 * (
            cos(theta0) * cos(theta0) -
            cos(theta1) * cos(theta1)
        );

    return max(0.0, nx * term_x + ny * term_y);
}

fn load_history_radiance(uv: vec2<f32>) -> vec3<f32> {
    let history_uv = clamp(uv, vec2f(0.0), vec2f(1.0));
    return textureSample(
        gi_source_history_texture,
        gi_source_history_sampler,
        history_uv
    ).rgb;
}

fn sample_filtered_history_radiance(
    sample_uv: vec2<f32>,
    wi: vec3<f32>,
    fallback_radiance: vec3<f32>,
    history_dims: vec2<f32>
) -> vec3<f32> {
    let history_radiance = load_history_radiance(sample_uv);
    let sample_normal = safe_normalize(textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        sample_uv
    ).xyz);
    let facing_weight = smoothstep(0.0, 0.25, dot(sample_normal, -wi));
    return mix(fallback_radiance, history_radiance, facing_weight);
}

fn safe_normalize3(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    return select(vec3<f32>(0.0), v / len, len > 0.0001);
}

fn build_basis_frisvad(n: vec3<f32>) -> mat3x3<f32> {
    let N = normalize(n);

    let sign = select(-1.0, 1.0, N.z >= 0.0);
    let a = -1.0 / (sign + N.z);
    let b = N.x * N.y * a;

    let T = vec3<f32>(
        1.0 + sign * N.x * N.x * a,
        sign * b,
        -sign * N.x
    );

    let B = vec3<f32>(
        b,
        sign + N.y * N.y * a,
        -N.y
    );

    return mat3x3<f32>(T, B, N);
}

fn angle_between(a: vec3<f32>, b: vec3<f32>) -> f32 {
    let d = dot(a, b);
    return acos(d);
}

fn gtao2(in: VertexOutput) -> FragmentOutput {
    let uv = in.tex_coords;
    let world_pos_sample = textureSample(
        gbuffer_world_position,
        gbuffer_world_position_sampler,
        uv
    );
    if (world_pos_sample.w < 0.5) {
        // invalid sample
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0), vec4<f32>(0.0));
    }
    let P = world_pos_sample.xyz;

    let normal = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    ).xyz;
    let V = safe_normalize3(camera_pos - P);
    // view facing normal:
    let N = select(normal, -normal, dot(normal, V) < 0.0);
    let TBN = build_basis_frisvad(N);

    // config
    let radius_pixels = gtao_settings.x;
    let ao_radius = gtao_settings.y;
    let gtao_power = gtao_settings.z;
    let hbil_radius = min(gtao_settings.w, ao_radius);

    let dims = vec2<f32>(textureDimensions(gbuffer_world_position, 0));
    let inv_dims = 1.0 / dims;
    // history can be half res, quarter, or something else
    let history_dims = vec2<f32>(textureDimensions(gi_source_history_texture, 0));
    let inv_history_dims = 1.0 / history_dims;

    var visibility_acc = 0.0;
    var bent_normal_acc = vec3f(0.0, 0.0, 0.0);
    var irradiance_acc = vec3f(0.0, 0.0, 0.0);
    var irradiance_samples: u32 = 0;

    for (var dir_idx: u32 = 0u; dir_idx < DIRECTIONS; dir_idx += 1u) {
        // Note: this was wrong previously with 2PI, we only need to cover half of the hemisphere
        // since we are taking samples from both sides of each slice
        let azimuth = PI * (f32(dir_idx) / f32(DIRECTIONS));
        let slice_dir_uv = vec2<f32>(cos(azimuth), sin(azimuth));
        let uv_step = slice_dir_uv * inv_dims;
        let history_uv_step = slice_dir_uv * inv_history_dims;

        var horizon_angle_fwd = 0.0;
        var horizon_angle_bwd = 0.0;

        for (var step_idx: u32 = 1u; step_idx <= STEPS_PER_DIRECTION; step_idx += 1u) {
            let step_t = f32(step_idx) / f32(STEPS_PER_DIRECTION);
            let step_uv_offset = uv_step * step_t * radius_pixels;
            let step_history_uv_offset = history_uv_step * step_t * radius_pixels;

            let sample_fwd_world = textureSample(
                gbuffer_world_position,
                gbuffer_world_position_sampler,
                uv + step_uv_offset
            );

            if (sample_fwd_world.w >= 0.5) {
                let D = sample_fwd_world.xyz - P;
                // calculate angle between N and D
                // let's assume length(D) can't be 0 (radius_pixels needs to be larger than 0)
                let D_normalized = normalize(D);
                let angle = angle_between(N, D);
                let horizon_angle = max(PI / 2.0 - angle, 0.0);
                horizon_angle_fwd = max(horizon_angle_fwd, horizon_angle);

                // TODO not proper HBIL, work on this later
                let radiance_sample = load_history_radiance(uv + step_history_uv_offset);
                irradiance_acc += radiance_sample;
                irradiance_samples += 1;
            }

            let sample_bwd_world = textureSample(
                gbuffer_world_position,
                gbuffer_world_position_sampler,
                uv - step_uv_offset
            );

            if (sample_bwd_world.w >= 0.5) {
                let D = sample_bwd_world.xyz - P;
                let D_normalized = normalize(D);
                let angle = angle_between(N, D);
                let horizon_angle = max(PI / 2.0 - angle, 0.0);
                horizon_angle_bwd = max(horizon_angle_bwd, horizon_angle);

                let radiance_sample = load_history_radiance(uv - step_history_uv_offset);
                irradiance_acc += radiance_sample;
                irradiance_samples += 1;
            }
        }

        // average angle from N along slice plane
        let alpha = PI - horizon_angle_fwd - horizon_angle_bwd;
        // reconstruct vec3 from the angle
        let dist = select(-tan(alpha), tan(alpha), alpha > 0.0);
        let normal_contribution = normalize(N + TBN * dist * vec3f(slice_dir_uv, 0.0));
        bent_normal_acc += normal_contribution;

        // Simple visibility gathering TODO actual gtao
        visibility_acc += alpha / PI;
    }

    // ..........

    let ao = visibility_acc / f32(DIRECTIONS);
    let bent_normal = bent_normal_acc / f32(DIRECTIONS);
    let irradiance = select(
        vec3f(0.0, 0.0, 0.0),
        irradiance_acc / f32(irradiance_samples),
        irradiance_samples > 0
    );
    return FragmentOutput(vec4f(bent_normal, ao), vec4f(irradiance, 1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    return gtao2(in);
    /*
    let uv = in.tex_coords;

    let world_pos_sample = textureSample(
        gbuffer_world_position,
        gbuffer_world_position_sampler,
        uv
    );
    if (world_pos_sample.w < 0.5) {
        return FragmentOutput(vec4<f32>(0.0, 0.0, 0.0, 1.0), vec4<f32>(0.0));
    }

    let P = world_pos_sample.xyz;

    let normal_sample = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    );
    let V = safe_normalize(camera_pos - P);
    let base_normal = safe_normalize(normal_sample.xyz);
    let N = select(base_normal, -base_normal, dot(base_normal, V) < 0.0);

    let dims = vec2<f32>(textureDimensions(gbuffer_world_position, 0));
    let inv_resolution = 1.0 / dims;
    let history_dims = vec2<f32>(textureDimensions(gi_source_history_texture, 0));
    let gi_source_history = textureLoad(
        gi_source_history_texture,
        vec2i(clamp(uv * history_dims, vec2f(0.0), history_dims - vec2f(1.0))),
        0
    );
    let radius_pixels = gtao_settings.x;
    let ao_radius = gtao_settings.y;
    let gtao_power = gtao_settings.z;
    let hbil_radius = min(gtao_settings.w, ao_radius);

    var visibility_accum: f32 = 0.0;
    var bent_accum = vec3<f32>(0.0);
    var hbil_accum = vec3<f32>(0.0);

    for (var dir_idx: u32 = 0u; dir_idx < DIRECTIONS; dir_idx += 1u) {
        let azimuth = 2.0 * PI * (f32(dir_idx) / f32(DIRECTIONS));
        let slice_dir_uv = vec2<f32>(cos(azimuth), sin(azimuth));
        let uv_step = slice_dir_uv * inv_resolution;

        let neigh_pos_sample = textureSample(
            gbuffer_world_position,
            gbuffer_world_position_sampler,
            clamp(uv + uv_step, vec2<f32>(0.0), vec2<f32>(1.0))
        );

        var slice_tangent = vec3<f32>(0.0, 0.0, 0.0);
        if (neigh_pos_sample.w >= 0.5) {
            slice_tangent = neigh_pos_sample.xyz - P;
        }

        slice_tangent = slice_tangent - V * dot(slice_tangent, V);

        if (dot(slice_tangent, slice_tangent) < 1e-8) {
            let fallback_axis =
                select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(V.y) > 0.99);
            slice_tangent = cross(fallback_axis, V);
        }

        slice_tangent = safe_normalize(slice_tangent);
        let slice_plane_normal = safe_normalize(cross(V, slice_tangent));

        let initial_horizons = initialize_horizon_angles(slice_tangent, V, N);
        var horizon_angle_bwd: f32 = initial_horizons.x;
        var horizon_angle_fwd: f32 = initial_horizons.y;
        var history_radiance_fwd = gi_source_history.rgb;
        var history_radiance_bwd = gi_source_history.rgb;

        for (var step_idx: u32 = 1u; step_idx <= STEPS_PER_DIRECTION; step_idx += 1u) {
            let step_t = f32(step_idx) / f32(STEPS_PER_DIRECTION);
            let offset = slice_dir_uv * (radius_pixels * step_t) * inv_resolution;

            let sample_fwd_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
            let sample_fwd_world = textureSample(
                gbuffer_world_position,
                gbuffer_world_position_sampler,
                sample_fwd_uv
            );

            if (sample_fwd_world.w >= 0.5) {
                let D = sample_fwd_world.xyz - P;
                let dist2 = dot(D, D);
                if (dist2 <= ao_radius * ao_radius) {
                    let D_plane = D - slice_plane_normal * dot(D, slice_plane_normal);
                    let x = dot(D_plane, slice_tangent);
                    let y = dot(D_plane, V);
                    let wi = D * inverseSqrt(max(dist2, 1e-8));
                    let sample_radiance = sample_filtered_history_radiance(
                        sample_fwd_uv,
                        wi,
                        history_radiance_fwd,
                        history_dims,
                    );
                    history_radiance_fwd = sample_radiance;

                    if (x > 1e-5) {
                        let angle = atan2(y, x);
                        if (angle > horizon_angle_fwd) {
                            if (dist2 <= hbil_radius * hbil_radius) {
                                hbil_accum += sample_radiance * evaluate_hbil_interval(
                                    horizon_angle_fwd,
                                    angle,
                                    slice_tangent,
                                    V,
                                    N,
                                );
                            }
                            horizon_angle_fwd = angle;
                        }
                    }
                }
            }

            let sample_bwd_uv = clamp(uv - offset, vec2<f32>(0.0), vec2<f32>(1.0));
            let sample_bwd_world = textureSample(
                gbuffer_world_position,
                gbuffer_world_position_sampler,
                sample_bwd_uv
            );

            if (sample_bwd_world.w >= 0.5) {
                let D = sample_bwd_world.xyz - P;
                let dist2 = dot(D, D);
                if (dist2 <= ao_radius * ao_radius) {
                    let D_plane = D - slice_plane_normal * dot(D, slice_plane_normal);
                    let x = dot(D_plane, -slice_tangent);
                    let y = dot(D_plane, V);
                    let wi = D * inverseSqrt(max(dist2, 1e-8));
                    let sample_radiance = sample_filtered_history_radiance(
                        sample_bwd_uv,
                        wi,
                        history_radiance_bwd,
                        history_dims,
                    );
                    history_radiance_bwd = sample_radiance;

                    if (x > 1e-5) {
                        let angle = atan2(y, x);
                        if (angle > horizon_angle_bwd) {
                            if (dist2 <= hbil_radius * hbil_radius) {
                                hbil_accum += sample_radiance * evaluate_hbil_interval(
                                    horizon_angle_bwd,
                                    angle,
                                    -slice_tangent,
                                    V,
                                    N,
                                );
                            }
                            horizon_angle_bwd = angle;
                        }
                    }
                }
            }
        }

        let slice_vis = evaluate_slice_visibility(
            horizon_angle_bwd,
            horizon_angle_fwd,
            slice_tangent,
            V,
            N
        );
        let slice_bent = evaluate_slice_bent_contribution(
            horizon_angle_bwd,
            horizon_angle_fwd,
            slice_tangent,
            V,
            N
        );

        visibility_accum += slice_vis;
        bent_accum += slice_bent;
    }

    let visibility = visibility_accum / f32(DIRECTIONS);
    let raw_ao = 1.0 - saturate(visibility);
    let ao = pow(max(raw_ao, 1e-4), gtao_power);
    let bent_normal = select(N, safe_normalize(bent_accum), dot(bent_accum, bent_accum) > 1e-8);
    let hbil_diffuse_irradiance = vec4<f32>(hbil_accum / f32(DIRECTIONS), 1.0);

    return FragmentOutput(
        vec4<f32>(bent_normal, ao * max(gi_source_history.a, 1.0)),
        hbil_diffuse_irradiance,
    );
    */
}

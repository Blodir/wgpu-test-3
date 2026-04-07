@group(0) @binding(1) var<uniform> camera_pos: vec3<f32>;

@group(1) @binding(0) var gbuffer_normal_roughness: texture_2d<f32>;
@group(1) @binding(1) var gbuffer_normal_roughness_sampler: sampler;
@group(1) @binding(2) var gbuffer_world_position: texture_2d<f32>;
@group(1) @binding(3) var gbuffer_world_position_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const PI: f32 = 3.14159265358979323846;
const DIRECTIONS: u32 = 8u;
const STEPS_PER_DIRECTION: u32 = 4u;
const RADIUS_PIXELS: f32 = 8.0;
const AO_RADIUS: f32 = 30.0;

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

    if (proj_n_len < 1e-5 || h2 <= h1) {
        return vec3<f32>(0.0);
    }

    let slice_bent = vec2<f32>(
        sin(h2) - sin(h1),
        cos(h1) - cos(h2),
    );
    return (slice_tangent * slice_bent.x + view_dir * slice_bent.y) * proj_n_len;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.tex_coords;

    let world_pos_sample = textureSample(
        gbuffer_world_position,
        gbuffer_world_position_sampler,
        uv
    );
    if (world_pos_sample.w < 0.5) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
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

    var visibility_accum: f32 = 0.0;
    var bent_accum = vec3<f32>(0.0);

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

        var horizon_angle_fwd: f32 = -0.5 * PI;
        var horizon_angle_bwd: f32 = -0.5 * PI;

        for (var step_idx: u32 = 1u; step_idx <= STEPS_PER_DIRECTION; step_idx += 1u) {
            let step_t = f32(step_idx) / f32(STEPS_PER_DIRECTION);
            let offset = slice_dir_uv * (RADIUS_PIXELS * step_t) * inv_resolution;

            let sample_fwd_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
            let sample_fwd_world = textureSample(
                gbuffer_world_position,
                gbuffer_world_position_sampler,
                sample_fwd_uv
            );

            if (sample_fwd_world.w >= 0.5) {
                let D = sample_fwd_world.xyz - P;
                if (dot(D, D) <= AO_RADIUS * AO_RADIUS) {
                    let D_plane = D - slice_plane_normal * dot(D, slice_plane_normal);
                    let x = dot(D_plane, slice_tangent);
                    let y = dot(D_plane, V);

                    if (x > 1e-5) {
                        let angle = atan2(y, x);
                        horizon_angle_fwd = max(horizon_angle_fwd, angle);
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
                if (dot(D, D) <= AO_RADIUS * AO_RADIUS) {
                    let D_plane = D - slice_plane_normal * dot(D, slice_plane_normal);
                    let x = dot(D_plane, -slice_tangent);
                    let y = dot(D_plane, V);

                    if (x > 1e-5) {
                        let angle = atan2(y, x);
                        horizon_angle_bwd = max(horizon_angle_bwd, angle);
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
    let ao = 1.0 - saturate(visibility);
    let bent_normal = select(N, safe_normalize(bent_accum), dot(bent_accum, bent_accum) > 1e-8);

    return vec4<f32>(bent_normal, ao);
}

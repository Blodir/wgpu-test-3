@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;
@group(0) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(0) @binding(4) var<uniform> camera_view_rotation: mat3x3<f32>;

@group(1) @binding(2) var environment_texture: texture_cube<f32>;
@group(1) @binding(3) var environment_texture_sampler: sampler;

@group(2) @binding(0) var depth_pyramid: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_normal_roughness: texture_2d<f32>;
@group(2) @binding(2) var gbuffer_normal_roughness_sampler: sampler;
@group(2) @binding(3) var lighting_texture: texture_2d<f32>;
@group(2) @binding(4) var lighting_texture_sampler: sampler;
@group(2) @binding(5) var<uniform> environment_map_intensity: f32;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const MAX_ITERATIONS: u32 = 96;
const MAX_REFLECTION_LOD: f32 = 4.0;
const RAY_STEP_EPSILON: f32 = 1e-4;
const BOUNDARY_BUMP_EPSILON: f32 = 1e-2;

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

fn project_world_position(view_proj: mat4x4<f32>, world_position: vec3f) -> vec4f {
    let clip = view_proj * vec4f(world_position, 1.0);
    if (abs(clip.w) <= 1e-8) {
        return vec4f(-1.0, -1.0, 0.0, 0.0);
    }
    let ndc = clip.xyz / clip.w;
    return vec4f(
        ndc.x * 0.5 + 0.5,
        0.5 - ndc.y * 0.5,
        ndc.z,
        clip.w,
    );
}

fn project_clip_position(clip: vec4f) -> vec4f {
    if (abs(clip.w) <= 1e-8) {
        return vec4f(-1.0, -1.0, 0.0, 0.0);
    }
    let ndc = clip.xyz / clip.w;
    return vec4f(
        ndc.x * 0.5 + 0.5,
        0.5 - ndc.y * 0.5,
        ndc.z,
        clip.w,
    );
}

fn select_next_boundary_t(current_t: f32, t_x: f32, t_y: f32) -> f32 {
    var next_t = 1e30;
    if (t_x > current_t + RAY_STEP_EPSILON) {
        next_t = min(next_t, t_x);
    }
    if (t_y > current_t + RAY_STEP_EPSILON) {
        next_t = min(next_t, t_y);
    }
    return next_t;
}

fn sample_prefiltered_env(R: vec3f, roughness: f32) -> vec3f {
    return textureSampleLevel(
        environment_texture,
        environment_texture_sampler,
        R,
        roughness * MAX_REFLECTION_LOD
    ).rgb * environment_map_intensity;
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
    let uv = in.tex_coords;

    let depth_dims_u = textureDimensions(depth_pyramid, 0);
    let depth_dims = vec2f(depth_dims_u);
    let depth_max_coords = vec2i(depth_dims_u) - vec2i(1);
    let P_coords = min(vec2i(uv * depth_dims), depth_max_coords);
    let P_depth = textureLoad(depth_pyramid, P_coords, 0).r;

    if (P_depth <= 0.0) {
        return vec4f(0.0);
    }
    let normal_roughness = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    );
    let P_w = reconstruct_world_position_from_depth(uv, P_depth);

    // calculate ray reflected around normal
    let V = normalize(camera_position - P_w);
    let N = normalize(normal_roughness.xyz);
    let R = reflect(-V, N);
    let P_clip = view_proj * vec4f(P_w, 1.0);
    let R_clip = view_proj * vec4f(R, 0.0);
    let direction_ndc = vec2f(
        R_clip.x * P_clip.w - P_clip.x * R_clip.w,
        R_clip.y * P_clip.w - P_clip.y * R_clip.w,
    );
    let direction_uv = vec2f(
        direction_ndc.x,
        -direction_ndc.y,
    );
    let moving_right = direction_ndc.x > 0.0;
    let moving_down  = direction_ndc.y < 0.0;

    var t = 0.0;

    let mip_level_count = i32(textureNumLevels(depth_pyramid));
    var current_mip_level = 0;
    let source_pixel_stride = 1.0 / max(depth_dims.x, depth_dims.y);
    var radiance = vec3f(0.0);
    var iteration = 0u;

    var screen_hit = false;
    var failed_self_intersection = false;

    var candidate_clip = P_clip + t * R_clip;
    var candidate_projected = project_clip_position(candidate_clip);
    var candidate_uv = candidate_projected.xy;

    loop {
        if (iteration >= MAX_ITERATIONS) {
            // no hit, sample prefiltered env
            radiance = sample_prefiltered_env(R, normal_roughness.w);
            break;
        }
        iteration += 1u;

        let ray_depth = candidate_projected.z;

        if (
            candidate_projected.w <= 0.0 ||
            any(candidate_uv < vec2f(0.0)) ||
            any(candidate_uv > vec2f(1.0)) ||
            ray_depth <= 0.0 ||
            ray_depth >= 1.0
        ) {
            // no hit, sample prefiltered env
            radiance = sample_prefiltered_env(R, normal_roughness.w);
            break;
        }

        let mip_level_dims_u = textureDimensions(depth_pyramid, current_mip_level);
        let mip_level_dims = vec2f(mip_level_dims_u);
        let coords = clamp(
            vec2i(candidate_uv * mip_level_dims),
            vec2i(0),
            vec2i(mip_level_dims_u) - vec2i(1)
        );
        if (iteration > 1) {
            let depth = textureLoad(depth_pyramid, coords, current_mip_level).r;
            if (ray_depth <= depth && iteration > 1) {
                // intersection
                if (current_mip_level < 1) {
                    // pixel hit!
                    if (distance(candidate_uv, uv) <= source_pixel_stride) {
                        // self intersection detected -> skip this pixel
                        failed_self_intersection = true;
                    } else {
                        radiance = textureSample(lighting_texture, lighting_texture_sampler, candidate_uv).rgb;
                        screen_hit = true;
                        break;
                    }
                } else {
                    // descend
                    current_mip_level = max(0, current_mip_level - 1);
                    continue;
                }
            }
        }

        // miss!
        // find the next boundary and advance t
        // find current cell
        let mip_cell = candidate_uv * mip_level_dims;
        let mip_cell_floor = floor(mip_cell);
        // find x and y lines depending on direction
        // bump the boundary slightly so we land inside the next cell instead of on the border
        let x_cell = select(
            mip_cell_floor.x - BOUNDARY_BUMP_EPSILON,
            mip_cell_floor.x + 1.0 + BOUNDARY_BUMP_EPSILON,
            moving_right
        );
        let y_cell = select(
            mip_cell_floor.y - BOUNDARY_BUMP_EPSILON,
            mip_cell_floor.y + 1.0 + BOUNDARY_BUMP_EPSILON,
            moving_down
        );
        var x_boundary = x_cell / mip_level_dims.x;
        var y_boundary = y_cell / mip_level_dims.y;
        // represent boundaries in NDC
        x_boundary = x_boundary * 2.0 - 1.0;
        y_boundary = 1.0 - y_boundary * 2.0;
        // for both x and y lines, solve intersection for t
        // conceptually: P.x + t * R.x = x_boundary
        // but in NDC that becomes:
        // (P.x + tR.x) / (P.w + tR.w) = x_boundary
        // solved form:
        // t = (P.w * x - P.x) / (R.x - R.w * x)
        let t_x = (P_clip.w * x_boundary - P_clip.x) / (R_clip.x - R_clip.w * x_boundary);
        let t_y = (P_clip.w * y_boundary - P_clip.y) / (R_clip.y - R_clip.w * y_boundary);
        let next_t = select_next_boundary_t(t, t_x, t_y);
        if (next_t > 1e29) {
            radiance = sample_prefiltered_env(R, normal_roughness.w);
            break;
        }

        t = next_t;

        // check if we can ascend in mip
        // (the next level doesn't contain any pixels already covered)
        candidate_clip = P_clip + t * R_clip;
        candidate_projected = project_clip_position(candidate_clip);
        candidate_uv = candidate_projected.xy;
        let candidate_uv_clamped = clamp(candidate_projected.xy, vec2f(0.0), vec2f(1.0) - 1.0 / mip_level_dims);
        let next_cell = floor(candidate_uv_clamped * mip_level_dims);
        if (
            (t_x < t_y && (
                (direction_uv.x > 0 && u32(next_cell.x) % 2u == 0u)
                || (direction_uv.x < 0 && u32(next_cell.x) % 2u == 1u)
            ))
            || (t_y < t_x && (
                (direction_uv.y > 0 && u32(next_cell.y) % 2u == 0u)
                || (direction_uv.y < 0 && u32(next_cell.y) % 2u == 1u)
            ))
        ) {
            current_mip_level = min(current_mip_level + 1, mip_level_count - 1);
        }
    }

    //return vec4f(select(vec3f(0.0), vec3f(1.0), screen_hit), 1.0);
    //return vec4f(select(vec3f(0.0), vec3f(1.0), failed_self_intersection), 1.0);
    //return vec4f(vec3f(f32(iteration) / f32(MAX_ITERATIONS)), 1.0);
    return vec4f(radiance, 1.0);
    //return vec4f(vec3f(select(0.0, 1.0, iteration == MAX_ITERATIONS)), 1.0);
}

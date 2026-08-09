@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;
@group(0) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(0) @binding(4) var<uniform> camera_view_rotation: mat3x3<f32>;

@group(1) @binding(2) var environment_texture: texture_cube<f32>;
@group(1) @binding(3) var environment_texture_sampler: sampler;

@group(2) @binding(0) var depth_pyramid: texture_2d<f32>;
@group(2) @binding(1) var normal_roughness_pyramid: texture_2d<f32>;
@group(2) @binding(2) var normal_roughness_pyramid_sampler: sampler;
@group(2) @binding(3) var lighting_texture: texture_2d<f32>;
@group(2) @binding(4) var lighting_texture_sampler: sampler;
@group(2) @binding(5) var<uniform> environment_map_intensity: f32;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

// prefilter
const KERNEL_SIZE: i32 = 3;
const MIN_FILTER_WEIGHT: f32 = 1e-5;
const PLANE_HEURISTIC_WEIGHT: f32 = 1e1;

// confidence
const NORMAL_SIMILARITY_EXPONENT: f32 = 20.0;
const MIN_ROUGHNESS_CONFIDENCE: f32 = 1e-1;

// trace
const MAX_ITERATIONS: u32 = 96;
const MAX_REFLECTION_LOD: f32 = 4.0;
const BOUNDARY_BUMP_EPSILON: f32 = 1e-2;
const HYSTERESIS_MIN_STEPS: u32 = 2u;
const SELF_INTERSECT_PLANE_DIST: f32 = 0.1;
const DEPTH_TEST_THRESHOLD = 1e-3;

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
    if (t_x > current_t) {
        next_t = min(next_t, t_x);
    }
    if (t_y > current_t) {
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

fn sample_reflection_source(uv: vec2f, roughness: f32) -> vec4f {
    let max_source_lod = min(
        MAX_REFLECTION_LOD,
        f32(textureNumLevels(lighting_texture) - 1u)
    );
    return textureSampleLevel(
        lighting_texture,
        lighting_texture_sampler,
        uv,
        roughness * max_source_lod
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
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.tex_coords;

    let full_dims_u = textureDimensions(depth_pyramid, 0);
    let full_dims = vec2f(full_dims_u);
    let full_max_coords = vec2i(full_dims_u) - vec2i(1);

    let half_dims_u = textureDimensions(depth_pyramid, 1);
    let half_dims = vec2f(half_dims_u);
    let half_max_coords = vec2i(half_dims_u) - vec2i(1);

    let P_coords = min(vec2i(uv * full_dims), full_max_coords);
    let P_half_coords = min(vec2i(uv * half_dims), half_max_coords);
    let P_depth = textureLoad(depth_pyramid, P_coords, 0).r;
    if (P_depth <= 0.0) {
        return vec4f(0.0);
    }

    var normal_roughness = textureSampleLevel(
        normal_roughness_pyramid,
        normal_roughness_pyramid_sampler,
        uv,
        f32(1)
    );
    //normal_roughness.w = 0.0; // temp test override
    let original_normal = normalize(normal_roughness.xyz);
    let P_w = reconstruct_world_position_from_depth(uv, P_depth);

    // Antialias ray direction
    var normal_acc = vec3f(0.0);
    var weight_acc = 0.0;
    let extent = KERNEL_SIZE / 2;
    for (var y: i32 = -extent; y <= extent; y += 1) {
        for (var x: i32 = -extent; x <= extent; x += 1) {
            let neighbor_coords = P_half_coords + vec2i(x, y);
            if (
                any(neighbor_coords > half_max_coords) ||
                any(neighbor_coords < vec2i(0))
            ) {
                continue;
            }

            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / half_dims;
            let neighbor_depth = textureLoad(depth_pyramid, neighbor_coords, 1).r;
            if (neighbor_depth <= 0.0) {
                continue;
            }

            let neighbor_normal_roughness = textureSampleLevel(
                normal_roughness_pyramid,
                normal_roughness_pyramid_sampler,
                neighbor_uv,
                1.0
            );
            let neighbor_world_position = reconstruct_world_position_from_depth(
                neighbor_uv,
                neighbor_depth
            );

            let delta = neighbor_world_position - P_w;
            let plane_distance = pow(abs(dot(original_normal, delta)), PLANE_HEURISTIC_WEIGHT);
            let w_plane = 1.0 / max(plane_distance, MIN_FILTER_WEIGHT);
            var w = w_plane;

            // don't count backfaces
            let neighbor_normal = neighbor_normal_roughness.xyz;
            w *= max(0.0, dot(neighbor_normal, original_normal));
            //w *= select(0.0, 1.0, dot(neighbor_normal, original_normal) > 0);

            normal_acc += w * neighbor_normal;
            weight_acc += w;
        }
    }
    normal_acc /= weight_acc;

    // calculate ray reflected around normal
    let V = normalize(camera_position - P_w);
    let N = normalize(normal_acc);
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
    let mip_level_count = i32(textureNumLevels(depth_pyramid));
    let ibl_radiance = sample_prefiltered_env(R, normal_roughness.w);

    // SSR trace

    // renormalized
    let normal_similarity = pow(clamp(0.0, 1.0, dot(N, normal_roughness.rgb)), NORMAL_SIMILARITY_EXPONENT) / 4.0 + 0.75;

    var confidence = max((1.0 - normal_roughness.w), MIN_ROUGHNESS_CONFIDENCE) * normal_similarity;
    var t = 0.0;

    var hit_radiance = vec3f(0.0);

    var current_mip_level = 0;
    var iteration = 0u;
    var last_descended = 0u;

    // debug
    var failed_self_intersection = false;
    var backface_rejected = false;
    var plane_rejection_test = vec3f(0.0);

    var screen_hit = false;

    var candidate_clip = P_clip + t * R_clip;
    var candidate_projected = project_clip_position(candidate_clip);
    var candidate_uv = candidate_projected.xy;
    var previous_candidate_projected = candidate_projected;
    var previous_candidate_uv = candidate_uv;

    loop {
        if (iteration >= MAX_ITERATIONS) {
            break;
        }

        let ray_depth = candidate_projected.z;

        if (
            candidate_projected.w <= 0.0 ||
            any(candidate_uv < vec2f(0.0)) ||
            any(candidate_uv > vec2f(1.0)) ||
            ray_depth <= 0.0 ||
            ray_depth >= 1.0
        ) {
            break;
        }

        let mip_level_dims_u = textureDimensions(depth_pyramid, current_mip_level);
        let mip_level_dims = vec2f(mip_level_dims_u);
        let coords = clamp(
            vec2i(candidate_uv * mip_level_dims),
            vec2i(0),
            vec2i(mip_level_dims_u) - vec2i(1)
        );
        if (iteration > 0) {
            let depth = textureLoad(depth_pyramid, coords, current_mip_level).r;
            if (ray_depth <= depth) {
                // intersection
                if (current_mip_level < 1) {
                    // pixel hit!
                    // subpixel corrected uv:
                    let hit_uv = (vec2f(coords) + vec2f(0.5)) / mip_level_dims;
                    let hit_world = reconstruct_world_position_from_depth(hit_uv, depth);
                    ///let hit_world = reconstruct_world_position_from_depth(candidate_uv, depth);
                    // reject self intersection
                    if (abs(dot(N, hit_world - P_w)) <= SELF_INTERSECT_PLANE_DIST) {
                        failed_self_intersection = true;
                    } else {
                        let hit_normal_roughness = textureSampleLevel(
                            normal_roughness_pyramid,
                            normal_roughness_pyramid_sampler,
                            hit_uv,
                            0.0
                        );
                        // reject hits where the normal and reflected ray face the same direction
                        let NdotR = dot(hit_normal_roughness.xyz, R);
                        if (NdotR < 0) {
                            /*
                            let previous_ray_world = reconstruct_world_position_from_depth(
                                previous_candidate_uv,
                                previous_candidate_projected.z
                            );
                            let ray_world = reconstruct_world_position_from_depth(candidate_uv, ray_depth);
                            //plane_rejection_test = vec3f(distance(hit_world, ray_world) / 10.0);
                            //plane_rejection_test = vec3f(abs(ray_depth - depth) * 100.0);
                            let previous_plane_distance = dot(
                                normalize(hit_normal_roughness.xyz),
                                previous_ray_world - hit_world
                            );
                            let current_plane_distance = dot(
                                normalize(hit_normal_roughness.xyz),
                                ray_world - hit_world
                            );
                            */
                            // the ray should cross the hit point oriented against the surface normal
                            if (
                                //current_plane_distance < previous_plane_distance
                                abs(depth - ray_depth) < DEPTH_TEST_THRESHOLD
                            ) {
                                hit_radiance = sample_reflection_source(candidate_uv, normal_roughness.w).rgb;
                                screen_hit = true;
                                break;
                            } else {
                                // depth test fail
                                plane_rejection_test = vec3f(1.0);
                            }
                        } else {
                            backface_rejected = true;
                        }
                    }
                } else {
                    // descend
                    current_mip_level = max(0, current_mip_level - 1);
                    last_descended = iteration;
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
            break;
        }

        t = next_t;

        // check if we can ascend in mip
        // (the next level doesn't contain any pixels already covered)
        previous_candidate_projected = candidate_projected;
        previous_candidate_uv = candidate_uv;
        candidate_clip = P_clip + t * R_clip;
        candidate_projected = project_clip_position(candidate_clip);
        candidate_uv = candidate_projected.xy;
        if (iteration - last_descended >= HYSTERESIS_MIN_STEPS) {
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
        iteration += 1u;
    }

    let ao = textureSampleLevel(
        lighting_texture,
        lighting_texture_sampler,
        uv,
        0.0
    ).w;

    let mix_factor = confidence * select(0.0, 1.0, screen_hit);
    let radiance = mix(ao * ibl_radiance, hit_radiance, mix_factor);

    //return vec4f(select(vec3f(0.0), vec3f(1.0), screen_hit), 1.0);
    //return vec4f(select(vec3f(0.0), vec3f(1.0), failed_self_intersection), 1.0);
    //return vec4f(select(vec3f(0.0), vec3f(1.0), backface_rejected), 1.0);
    //return vec4f(vec3f(normal_similarity), confidence);
    //return vec4f(vec3f(ao), confidence);
    //return vec4f(plane_rejection_test, confidence);
    //return vec4f(vec3f(f32(iteration) / f32(MAX_ITERATIONS)), 1.0);
    return vec4f(radiance, confidence);
    //return vec4f(vec3f(avg_weight), 1.0);
    //return vec4f(vec3f(normal_roughness.w), 1.0);
    //return vec4f((N + 1.0) / 2.0, 1.0);
    //return vec4f(vec3f(weight_acc / 9.0), 1.0);
    //let normal_diff = normalize(normal_acc) - normalize(normal_roughness.xyz);
    //return vec4f(abs(normal_diff), 1.0);
    //return vec4f(vec3f(select(0.0, 1.0, iteration == MAX_ITERATIONS)), 1.0);
}

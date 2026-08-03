@group(0) @binding(0) var input_bent_ao_texture: texture_2d<f32>;
@group(0) @binding(1) var input_irradiance_variance_texture: texture_2d<f32>;
@group(0) @binding(2) var normal_history_texture: texture_2d<f32>;
@group(0) @binding(3) var depth_history_texture: texture_2d<f32>;

@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) bent_ao: vec4<f32>,
    @location(1) irradiance_variance: vec4<f32>,
}

const FILTER_SIZE: i32 = 3;
const GAUSSIAN_3X3: array<f32, 3> = array<f32, 3>(1.0 / 4.0, 1.0 / 2.0, 1.0 / 4.0);
const MAX_PIXEL_RADIUS: f32 = 20.0;

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

struct Candidate {
    weight: f32,
    irradiance: vec3f,
    first_moment: f32,
    second_moment: f32,
};

fn insert_candidate(
    candidate: Candidate,
    best: ptr<function, array<Candidate, 4>>,
) {
    var value = candidate;

    // best[0] is highest weight, best[K - 1] is lowest.
    for (var i = 0u; i < 4; i++) {
        if (value.weight > (*best)[i].weight) {
            let displaced = (*best)[i];
            (*best)[i] = value;
            value = displaced;
        }
    }
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

fn v1(in: VertexOutput) -> FragmentOutput {
    let uv = in.tex_coords;
    let dims_u = textureDimensions(depth_history_texture);
    let dims = vec2f(dims_u);
    let max_coords = vec2i(dims_u) - vec2i(1);
    let center_coords = min(vec2i(uv * dims), max_coords);

    let center_depth_moments = textureLoad(depth_history_texture, center_coords, 0);
    let center_bent_ao = textureLoad(input_bent_ao_texture, center_coords, 0);
    let center_irradiance_variance = textureLoad(input_irradiance_variance_texture, center_coords, 0);
    let center_normal = textureLoad(normal_history_texture, center_coords, 0).xyz;

    let center_history_length = center_depth_moments.w;
    let center_world_position = reconstruct_world_position_from_depth(
        uv,
        center_depth_moments.x
    );

    let extent = i32(FILTER_SIZE / 2);
    let radius = max(1.0, MAX_PIXEL_RADIUS / (1.0 + center_history_length));

    var first_moment_acc = 0.0;
    var second_moment_acc = 0.0;
    var weight_acc = 0.0;

    var min_irradiance = center_irradiance_variance.rgb;
    var max_irradiance = center_irradiance_variance.rgb;

    for (var x: i32 = -extent; x <= extent; x++) {
        for (var y: i32 = -extent; y <= extent; y++) {
            let neighbor_coords = center_coords + vec2i(vec2f(f32(x), f32(y)) * radius);
            if (any(neighbor_coords > vec2i(dims_u)- vec2i(1)) || any(neighbor_coords < vec2i(0))) {
                continue;
            }

            let neighbor_depth_moments = textureLoad(depth_history_texture, neighbor_coords, 0);
            let neighbor_samples = neighbor_depth_moments.w;
            let neighbor_normal = textureLoad(normal_history_texture, neighbor_coords, 0).xyz;
            let neighbor_irradiance_variance = textureLoad(input_irradiance_variance_texture, neighbor_coords, 0);

            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / dims;
            let neighbor_world_position = reconstruct_world_position_from_depth(
                neighbor_uv,
                neighbor_depth_moments.x
            );

            /*
            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / dims;
            let neighbor_world_position = reconstruct_world_position_from_depth(
                neighbor_uv,
                neighbor_depth_moments.x
            );
            let ws_dist = distance(center_world_position, neighbor_world_position);
            let w = neighbor_samples / ws_dist;
            */

            // plane distance weight
            let delta = neighbor_world_position - center_world_position;
            let plane_distance = abs(dot(center_normal, delta));
            let w_plane = 1.0 / plane_distance;

            // normal weight
            let NdotN = dot(center_normal, neighbor_normal);
            let w_normal = clamp(NdotN, 0.000001, 1.0);

            let h_q = GAUSSIAN_3X3[x + extent] * GAUSSIAN_3X3[y + extent];
            let w = neighbor_samples * w_plane * w_normal * h_q;
            first_moment_acc += w * neighbor_depth_moments.y;
            second_moment_acc += w * neighbor_depth_moments.z;
            weight_acc += w;

            min_irradiance = min(min_irradiance, neighbor_irradiance_variance.rgb);
            max_irradiance = max(max_irradiance, neighbor_irradiance_variance.rgb);
        }
    }

    let first_moment = first_moment_acc / weight_acc;
    let second_moment = second_moment_acc / weight_acc;
    let variance = second_moment - first_moment * first_moment;

    let clamped_irradiance = clamp(center_irradiance_variance.rgb, min_irradiance, max_irradiance);
    let filtered_irradiance = mix(center_irradiance_variance.rgb, clamped_irradiance, 1.0 / (center_history_length * center_history_length));

    return FragmentOutput(
        center_bent_ao,
        vec4f(filtered_irradiance, variance)
    );
}

// 3x3 spatial gather
// takes the best x samples from the neighborhood to fill out the missing history
// sample selection is edge aware weighted, but each sample counts equal weight
fn v2(in: VertexOutput) -> FragmentOutput {
    let uv = in.tex_coords;
    let dims_u = textureDimensions(depth_history_texture);
    let dims = vec2f(dims_u);
    let max_coords = vec2i(dims_u) - vec2i(1);
    let center_coords = min(vec2i(uv * dims), max_coords);

    let center_bent_ao = textureLoad(input_bent_ao_texture, center_coords, 0);
    let center_irradiance_variance = textureLoad(input_irradiance_variance_texture, center_coords, 0);
    let center_depth_moments = textureLoad(depth_history_texture, center_coords, 0);
    if (center_depth_moments.x <= 0.0) {
        return FragmentOutput(
            center_bent_ao,
            vec4f(center_irradiance_variance.rgb, center_depth_moments.z - center_depth_moments.y * center_depth_moments.y)
        );
    }
    let center_history_length = i32(round(center_depth_moments.w));

    let goal_samples = 5;
    let missing_samples = goal_samples - center_history_length;
    if (missing_samples <= 0) {
        return FragmentOutput(
            center_bent_ao,
            vec4f(center_irradiance_variance.rgb, center_depth_moments.z - center_depth_moments.y * center_depth_moments.y)
        );
    }

    let center_normal = textureLoad(normal_history_texture, center_coords, 0).xyz;

    let center_world_position = reconstruct_world_position_from_depth(
        uv,
        center_depth_moments.x
    );

    var best: array<Candidate, 4>;
    for (var i = 0u; i < 4; i++) {
        best[i].weight = -1.0;
    }
    var worst_weight = -1.0;

    for (var x: i32 = -1; x <= 1; x++) {
        for (var y: i32 = -1; y <= 1; y++) {
            if (x == 0 && y == 0) {
                // skip center
                continue;
            }
            let neighbor_coords = center_coords + vec2i(x, y);
            if (any(neighbor_coords > vec2i(dims_u)- vec2i(1)) || any(neighbor_coords < vec2i(0))) {
                // skip off bounds
                continue;
            }

            let neighbor_depth_moments = textureLoad(depth_history_texture, neighbor_coords, 0);
            if (neighbor_depth_moments.x <= 0.0) {
                continue;
            }
            let neighbor_normal = textureLoad(normal_history_texture, neighbor_coords, 0).xyz;
            let neighbor_irradiance_variance = textureLoad(input_irradiance_variance_texture, neighbor_coords, 0);

            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / dims;
            let neighbor_world_position = reconstruct_world_position_from_depth(
                neighbor_uv,
                neighbor_depth_moments.x
            );

            // plane distance weight
            let delta = neighbor_world_position - center_world_position;
            let plane_distance = abs(dot(center_normal, delta));
            let w_plane = 1.0 / plane_distance;

            // normal weight
            let NdotN = dot(center_normal, neighbor_normal);
            let w_normal = clamp(NdotN, 0.000001, 1.0);

            //let h_q = GAUSSIAN_3X3[x + 1] * GAUSSIAN_3X3[y + 1];
            //let w = w_plane * w_normal * h_q;
            let w = w_plane * w_normal;

            let neighbor_history = min(i32(round(neighbor_depth_moments.w)), 4);
            if (w > worst_weight) {
                for (var i = 0; i < neighbor_history; i++) {
                    let candidate = Candidate(
                        w,
                        neighbor_irradiance_variance.rgb,
                        neighbor_depth_moments.y,
                        neighbor_depth_moments.z,
                    );
                    insert_candidate(candidate, &best);
                }
            }
            worst_weight = best[3].weight;
        }
    }

    var valid_samples = f32(center_history_length);
    var irradiance_acc = center_irradiance_variance.rgb * valid_samples;
    var first_moment_acc = center_depth_moments.y * valid_samples;
    var second_moment_acc = center_depth_moments.z * valid_samples;

    for (var i = 0; i < missing_samples; i++) {
        if (best[i].weight < 0) {
            continue;
        }
        irradiance_acc += best[i].irradiance;
        first_moment_acc += best[i].first_moment;
        second_moment_acc += best[i].second_moment;
        valid_samples += 1.0;
    }

    let irradiance = irradiance_acc / valid_samples;
    let first_moment = first_moment_acc / valid_samples;
    let second_moment = second_moment_acc / valid_samples;
    let variance = second_moment - first_moment * first_moment;

    return FragmentOutput(
        center_bent_ao,
        vec4f(irradiance, variance)
    );
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    return v2(in);
}

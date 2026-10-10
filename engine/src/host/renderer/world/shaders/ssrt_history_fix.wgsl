@group(0) @binding(0) var ssgi_history: texture_2d<f32>;
@group(0) @binding(1) var ssr_history: texture_2d<f32>;
@group(0) @binding(2) var normal_roughness_history: texture_2d<f32>;
@group(0) @binding(3) var depth_texture: texture_2d<f32>;
@group(0) @binding(4) var moments_history: texture_2d<f32>;
@group(0) @binding(5) var bent_normal_ao_history: texture_2d<f32>;

@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) ssgi_denoise: vec4<f32>,
    @location(1) ssr_denoise: vec4<f32>,
    @location(2) bent_normal_ao_denoise: vec4<f32>,
}

fn safe_normalize3(v: vec3f, fallback: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8 && len < 1e20) {
        return v / len;
    }
    return fallback;
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

struct Candidate {
    weight: f32,
    irradiance: vec3f,
    first_moment: f32,
    second_moment: f32,
    bent_ao: vec4f,
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

// 3x3 spatial gather
// takes the best x samples from the neighborhood to fill out the missing history
// sample selection is edge aware weighted, but each sample counts equal weight
@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let uv = in.tex_coords;
    let dims_u = textureDimensions(ssgi_history);
    let dims = vec2f(dims_u);
    let max_coords = vec2i(dims_u) - vec2i(1);
    let center_coords = min(vec2i(uv * dims), max_coords);

    let ssgi_center_irradiance_variance = textureLoad(ssgi_history, center_coords, 0);
    let ssr_center_irradiance_variance = textureLoad(ssr_history, center_coords, 0);
    let center_bent_ao = textureLoad(bent_normal_ao_history, center_coords, 0);
    let depth_dims = vec2i(textureDimensions(depth_texture));
    let center_depth_coords = clamp(vec2i(uv * vec2f(depth_dims)), vec2i(0), depth_dims - vec2i(1));
    let center_depth = textureLoad(depth_texture, center_depth_coords, 0).x;
    let center_moments = textureLoad(moments_history, center_coords, 0);
    let ssgi_center_moments = center_moments.xy;
    let ssr_center_moments = center_moments.zw;
    var output = FragmentOutput(
        vec4f(ssgi_center_irradiance_variance.rgb, max(ssgi_center_moments.y - ssgi_center_moments.x * ssgi_center_moments.x, 0.0)),
        vec4f(ssr_center_irradiance_variance.rgb, max(ssr_center_moments.y - ssr_center_moments.x * ssr_center_moments.x, 0.0)),
        center_bent_ao
    );
    if (center_depth <= 0.0) {
        return output;
    }
    let ssgi_center_history_length = i32(round(ssgi_center_irradiance_variance.a));
    let ssr_center_history_length = i32(round(ssr_center_irradiance_variance.a));

    let goal_samples = 5;
    let ssgi_missing_samples = goal_samples - ssgi_center_history_length;
    let ssr_missing_samples = goal_samples - ssr_center_history_length;
    if (ssgi_missing_samples <= 0 && ssr_missing_samples <= 0) {
        return output;
    }

    let center_normal = textureLoad(normal_roughness_history, center_coords, 0).xyz;

    let center_world_position = reconstruct_world_position_from_depth(
        uv,
        center_depth
    );

    var ssgi_best: array<Candidate, 4>;
    var ssr_best: array<Candidate, 4>;
    for (var i = 0u; i < 4; i++) {
        ssgi_best[i].weight = -1.0;
        ssr_best[i].weight = -1.0;
    }
    var ssgi_worst_weight = -1.0;
    var ssr_worst_weight = -1.0;

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

            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / dims;
            let neighbor_depth_coords = clamp(vec2i(neighbor_uv * vec2f(depth_dims)), vec2i(0), depth_dims - vec2i(1));
            let neighbor_depth = textureLoad(depth_texture, neighbor_depth_coords, 0).x;
            if (neighbor_depth <= 0.0) {
                continue;
            }
            let neighbor_normal = textureLoad(normal_roughness_history, neighbor_coords, 0).xyz;

            let neighbor_world_position = reconstruct_world_position_from_depth(
                neighbor_uv,
                neighbor_depth
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

            let neighbor_moments = textureLoad(moments_history, neighbor_coords, 0);
            if (ssgi_missing_samples > 0) {
                let ssgi_neighbor_irradiance_variance = textureLoad(ssgi_history, neighbor_coords, 0);
                let ssgi_neighbor_moments = neighbor_moments.xy;
                let neighbor_bent_ao = textureLoad(bent_normal_ao_history, neighbor_coords, 0);
                let ssgi_neighbor_history = min(i32(round(ssgi_neighbor_irradiance_variance.a)), 4);
                if (w > ssgi_worst_weight) {
                    for (var i = 0; i < ssgi_neighbor_history; i++) {
                        let ssgi_candidate = Candidate(
                            w,
                            ssgi_neighbor_irradiance_variance.rgb,
                            ssgi_neighbor_moments.x,
                            ssgi_neighbor_moments.y,
                            neighbor_bent_ao,
                        );
                        insert_candidate(ssgi_candidate, &ssgi_best);
                    }
                }
                ssgi_worst_weight = ssgi_best[3].weight;
            }
            if (ssr_missing_samples > 0) {
                let ssr_neighbor_irradiance_variance = textureLoad(ssr_history, neighbor_coords, 0);
                let ssr_neighbor_moments = neighbor_moments.zw;
                let ssr_neighbor_history = min(i32(round(ssr_neighbor_irradiance_variance.a)), 4);
                if (w > ssr_worst_weight) {
                    for (var i = 0; i < ssr_neighbor_history; i++) {
                        let ssr_candidate = Candidate(
                            w,
                            ssr_neighbor_irradiance_variance.rgb,
                            ssr_neighbor_moments.x,
                            ssr_neighbor_moments.y,
                            vec4f(0.0),
                        );
                        insert_candidate(ssr_candidate, &ssr_best);
                    }
                }
                ssr_worst_weight = ssr_best[3].weight;
            }
        }
    }

    var ssgi_valid_samples = f32(ssgi_center_history_length);
    var ssgi_irradiance_acc = ssgi_center_irradiance_variance.rgb * ssgi_valid_samples;
    var ssgi_first_moment_acc = ssgi_center_moments.x * ssgi_valid_samples;
    var ssgi_second_moment_acc = ssgi_center_moments.y * ssgi_valid_samples;
    var bent_acc = center_bent_ao.xyz * ssgi_valid_samples;
    var ao_acc = center_bent_ao.w * ssgi_valid_samples;
    var ssr_valid_samples = f32(ssr_center_history_length);
    var ssr_irradiance_acc = ssr_center_irradiance_variance.rgb * ssr_valid_samples;
    var ssr_first_moment_acc = ssr_center_moments.x * ssr_valid_samples;
    var ssr_second_moment_acc = ssr_center_moments.y * ssr_valid_samples;

    for (var i = 0; i < max(ssgi_missing_samples, ssr_missing_samples); i++) {
        if (i < ssgi_missing_samples && ssgi_best[i].weight >= 0.0) {
            ssgi_irradiance_acc += ssgi_best[i].irradiance;
            ssgi_first_moment_acc += ssgi_best[i].first_moment;
            ssgi_second_moment_acc += ssgi_best[i].second_moment;
            bent_acc += ssgi_best[i].bent_ao.xyz;
            ao_acc += ssgi_best[i].bent_ao.w;
            ssgi_valid_samples += 1.0;
        }
        if (i < ssr_missing_samples && ssr_best[i].weight >= 0.0) {
            ssr_irradiance_acc += ssr_best[i].irradiance;
            ssr_first_moment_acc += ssr_best[i].first_moment;
            ssr_second_moment_acc += ssr_best[i].second_moment;
            ssr_valid_samples += 1.0;
        }
    }

    if (ssgi_missing_samples > 0) {
        let ssgi_irradiance = ssgi_irradiance_acc / ssgi_valid_samples;
        let ssgi_first_moment = ssgi_first_moment_acc / ssgi_valid_samples;
        let ssgi_second_moment = ssgi_second_moment_acc / ssgi_valid_samples;
        let ssgi_variance = max(ssgi_second_moment - ssgi_first_moment * ssgi_first_moment, 0.0);
        output.ssgi_denoise = vec4f(ssgi_irradiance, ssgi_variance);
        let filtered_bent_normal = safe_normalize3(bent_acc, center_bent_ao.xyz);
        let filtered_ao = ao_acc / ssgi_valid_samples;
        output.bent_normal_ao_denoise = vec4f(filtered_bent_normal, filtered_ao);
    }
    if (ssr_missing_samples > 0) {
        let ssr_irradiance = ssr_irradiance_acc / ssr_valid_samples;
        let ssr_first_moment = ssr_first_moment_acc / ssr_valid_samples;
        let ssr_second_moment = ssr_second_moment_acc / ssr_valid_samples;
        let ssr_variance = max(ssr_second_moment - ssr_first_moment * ssr_first_moment, 0.0);
        output.ssr_denoise = vec4f(ssr_irradiance, ssr_variance);
    }

    return output;
}

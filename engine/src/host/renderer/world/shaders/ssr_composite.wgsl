@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;
@group(0) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(0) @binding(3) var<uniform> prev_inverse_view_proj: mat4x4<f32>;

@group(1) @binding(6) var brdf_lut: texture_2d<f32>;
@group(1) @binding(7) var brdf_lut_sampler: sampler;

@group(2) @binding(0) var gbuffer_albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_albedo_ao_sampler: sampler;
@group(2) @binding(2) var gbuffer_normal_roughness: texture_2d<f32>;
@group(2) @binding(3) var gbuffer_normal_roughness_sampler: sampler;
@group(2) @binding(4) var gbuffer_emissive_metallic: texture_2d<f32>;
@group(2) @binding(5) var gbuffer_emissive_metallic_sampler: sampler;
@group(2) @binding(6) var depth_texture: texture_depth_2d;
@group(2) @binding(7) var ssr_texture: texture_2d<f32>;
@group(2) @binding(8) var ssr_texture_sampler: sampler;
@group(2) @binding(9) var lighting_texture: texture_2d<f32>;
@group(2) @binding(10) var lighting_texture_sampler: sampler;
@group(2) @binding(11) var ssr_history_texture: texture_2d<f32>;
@group(2) @binding(12) var motion_vectors_texture: texture_2d<f32>;
@group(2) @binding(13) var prev_depth_history_texture: texture_2d<f32>;
@group(2) @binding(14) var prev_normal_history_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const MAX_PLANE_DISTANCE = 1e-2;
const MIN_FILTER_WEIGHT: f32 = 1e-6;

const MAX_HISTORY_LENGTH: f32 = 4.0;
const SVGF_NORMAL_REJECTION_DOT_THRESHOLD: f32 = 0.5;
const SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD: f32 = 1e0;

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

fn fresnel_schlick_roughness(cos_theta: f32, F0: vec3f, roughness: f32) -> vec3f {
    return F0 + (max(vec3f(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn clamp_coord(coord: vec2i, max_coord: vec2i) -> vec2i {
    return clamp(coord, vec2i(0), max_coord);
}

fn clamp_coordf(coord: vec2f, max_coord: vec2f) -> vec2f {
    return clamp(coord, vec2f(0), max_coord);
}

fn compute_current_ssr_radiance(
    uv: vec2f,
    full_dims: vec2f,
    full_max_coords: vec2i,
    world_position: vec3f,
    N: vec3f,
    surface_roughness: f32,
) -> vec3f {
    let ssr_dims_u = textureDimensions(ssr_texture);
    let ssr_dims = vec2f(ssr_dims_u);
    let ssr_max_coords = vec2i(ssr_dims_u) - vec2i(1);
    let ssr_center_coords = min(vec2i(uv * ssr_dims), ssr_max_coords);
    let center_world_position = world_position;
    let center_roughness = surface_roughness;
    let center_ssr = textureLoad(ssr_texture, ssr_center_coords, 0);

    var radiance_acc = center_ssr.rgb;
    var weight_acc = 1.0;
    for (var y: i32 = -1; y <= 1; y += 1) {
        for (var x: i32 = -1; x <= 1; x += 1) {
            if (x == 0 && y == 0) {
                continue;
            }
            let neighbor_coords = ssr_center_coords + vec2i(x, y);
            if (any(neighbor_coords > vec2i(ssr_dims_u) - vec2i(1)) || any(neighbor_coords < vec2i(0))) {
                continue;
            }
            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / ssr_dims;
            let neighbor_full_coords = clamp(
                vec2i(neighbor_uv * full_dims),
                vec2i(0),
                full_max_coords
            );
            let neighbor_normal_roughness = textureLoad(
                gbuffer_normal_roughness,
                neighbor_full_coords,
                0
            );
            let neighbor_depth = textureLoad(depth_texture, neighbor_full_coords, 0);
            if (neighbor_depth <= 0.0) {
                continue;
            }
            let neighbor_world_position = reconstruct_world_position_from_depth(
                neighbor_uv,
                neighbor_depth,
                inverse_view_proj
            );
            let neighbor_ssr = textureLoad(ssr_texture, neighbor_coords, 0);

            let delta = neighbor_world_position - center_world_position;
            let plane_distance = abs(dot(N, delta));
            let w_plane = max(1.0 - (plane_distance / MAX_PLANE_DISTANCE), MIN_FILTER_WEIGHT);

            let neighbor_normal = normalize(neighbor_normal_roughness.xyz);
            let NdotN = dot(N, neighbor_normal);
            let w_normal = clamp(NdotN, MIN_FILTER_WEIGHT, 1.0);

            let roughness_delta = abs(center_roughness - neighbor_normal_roughness.w);
            let w_roughness = clamp(1.0 - roughness_delta, MIN_FILTER_WEIGHT, 1.0);

            let confidence = max(0.0, 1.0 - center_ssr.w);
            let w = w_plane * w_normal * w_roughness * (confidence * confidence);

            radiance_acc += w * neighbor_ssr.rgb;
            weight_acc += w;
        }
    }

    let ssr_radiance = radiance_acc / weight_acc;
    return ssr_radiance;
    //return center_ssr.rgb;
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
fn fs_history(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.tex_coords;
    let full_dims_u = textureDimensions(depth_texture);
    let full_dims = vec2f(full_dims_u);
    let full_max_coords = vec2i(full_dims_u) - vec2i(1);
    let curr_full_coords = min(vec2i(uv * full_dims), full_max_coords);

    let half_dims_u = textureDimensions(ssr_texture);
    let half_dims = vec2f(half_dims_u);
    let half_max_coords = vec2i(half_dims_u) - vec2i(1);
    let curr_half_coords = min(vec2i(uv * half_dims), half_max_coords);

    let curr_depth = textureLoad(depth_texture, curr_full_coords, 0);
    if (curr_depth <= 0.0) {
        return vec4f(0.0, 0.0, 0.0, 1.0);
    }

    let surface_color_ao = textureLoad(gbuffer_albedo_ao, curr_full_coords, 0);
    let normal_roughness = textureLoad(gbuffer_normal_roughness, curr_full_coords, 0);
    let emissive_metallic = textureLoad(gbuffer_emissive_metallic, curr_full_coords, 0);
    let world_position = reconstruct_world_position_from_depth(
        uv,
        curr_depth,
        inverse_view_proj
    );
    let N = normalize(normal_roughness.xyz);
    let surface_roughness = normal_roughness.w;
    let curr_ssr = compute_current_ssr_radiance(
        uv,
        full_dims,
        full_max_coords,
        world_position,
        N,
        surface_roughness,
    );

    // NOTE: prev_depth_history_texture / prev_normal_history_texture currently come
    // from the shared reproject path, so this assumes they match the SSR half-res domain.
    let motion = textureLoad(motion_vectors_texture, curr_full_coords, 0).xy;
    let prev_uv = uv - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return vec4f(curr_ssr, 1.0);
    }

    let prev_coord = clamp_coordf(prev_uv * half_dims - vec2f(0.5), vec2f(half_max_coords));
    let prev_coord_floor = floor(prev_coord);
    let prev_coord_frac = fract(prev_coord);

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

    var prev_color_acc = vec3f(0.0);
    var prev_history_length_acc = 0.0;
    var prev_weight_acc = 0.0;

    for (var i = 0; i < 4; i++) {
        let tap_coords = clamp_coord(taps[i], half_max_coords);
        let tap_uv = (vec2f(tap_coords) + vec2f(0.5)) / half_dims;
        let tap_weight = tap_weights[i];

        let prev_depth_history = textureLoad(prev_depth_history_texture, tap_coords, 0);
        let prev_depth = prev_depth_history.x;
        if (prev_depth <= 0.0) {
            continue;
        }
        let prev_normal = normalize(textureLoad(prev_normal_history_texture, tap_coords, 0).xyz);
        let prev_world_position = reconstruct_world_position_from_depth(
            tap_uv,
            prev_depth,
            prev_inverse_view_proj
        );

        let delta = prev_world_position - world_position;
        let plane_distance = abs(dot(N, delta));
        let normal_similarity = dot(N, prev_normal);
        //let normal_weight = clamp((normal_similarity - SVGF_NORMAL_REJECTION_DOT_THRESHOLD) / (1.0 - SVGF_NORMAL_REJECTION_DOT_THRESHOLD), 0.0, 1.0);
        let normal_weight = smoothstep(0.5, 0.95, normal_similarity);
        let plane_weight = clamp(1.0 - (plane_distance / SVGF_PLANE_DISTANCE_REJECTION_THRESHOLD), 0.0, 1.0);
        let history_weight = tap_weight * normal_weight * plane_weight;
        //let history_weight = tap_weight * plane_weight;
        if (history_weight <= 0.0) {
            continue;
        }

        let prev_color_sample = textureLoad(ssr_history_texture, tap_coords, 0);

        prev_color_acc += prev_color_sample.rgb * history_weight;
        prev_history_length_acc += prev_color_sample.a * history_weight;
        prev_weight_acc += history_weight;
    }

    if (prev_weight_acc <= 0.0) {
        return vec4f(curr_ssr, 1.0);
    }

    prev_color_acc /= prev_weight_acc;
    prev_history_length_acc /= prev_weight_acc;

    let blend_factor = 1.0 / (1.0 + min(prev_history_length_acc, MAX_HISTORY_LENGTH));
    let out_radiance = mix(prev_color_acc, curr_ssr, blend_factor);
    return vec4f(out_radiance, min(prev_history_length_acc + 1.0, MAX_HISTORY_LENGTH));
    //return vec4f(curr_ssr, 1.0);
    //return vec4f(mix(prev_color_acc, curr_ssr, 0.001), 1.0);
}

@fragment
fn fs_scene(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.tex_coords;
    let lighting = textureSample(
        lighting_texture,
        lighting_texture_sampler,
        uv
    );
    let depth_dims_u = textureDimensions(depth_texture);
    let depth_dims = vec2f(depth_dims_u);
    let depth_max_coords = vec2i(depth_dims_u) - vec2i(1);
    let depth_coords = clamp(vec2i(uv * depth_dims), vec2i(0), depth_max_coords);
    let depth = textureLoad(depth_texture, depth_coords, 0);
    if (depth <= 0.0) {
        return lighting;
    }

    let surface_color_ao = textureLoad(gbuffer_albedo_ao, depth_coords, 0);
    let normal_roughness = textureLoad(gbuffer_normal_roughness, depth_coords, 0);
    let emissive_metallic = textureLoad(gbuffer_emissive_metallic, depth_coords, 0);
    let world_position = reconstruct_world_position_from_depth(
        uv,
        depth,
        inverse_view_proj
    );
    let N = normalize(normal_roughness.xyz);
    let surface_roughness = normal_roughness.w;
    let surface_color = surface_color_ao.rgb;
    let surface_metallic = emissive_metallic.a;
    let ao = surface_color_ao.a;
    let V = normalize(camera_position - world_position);
    let F0 = mix(vec3f(0.04), surface_color, surface_metallic);
    let F_env = fresnel_schlick_roughness(max(dot(N, V), 0.0), F0, surface_roughness);
    let brdf_specular_lut = textureSample(
        brdf_lut,
        brdf_lut_sampler,
        vec2(max(dot(N, V), 0.0), 1.0 - surface_roughness)
    ).rg;

    let accumulated_ssr_radiance = textureSampleLevel(
        ssr_history_texture,
        ssr_texture_sampler,
        uv,
        0.0
    ).rgb;
    let ssr_specular = accumulated_ssr_radiance * (F_env * brdf_specular_lut.x + brdf_specular_lut.y) * ao;
    //return vec4f(ssr_specular, 1.0);
    return vec4f(lighting.rgb + ssr_specular, 1.0);
}

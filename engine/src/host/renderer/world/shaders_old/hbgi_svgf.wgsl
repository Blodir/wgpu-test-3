@group(0) @binding(0) var input_bent_ao_texture: texture_2d<f32>;
@group(0) @binding(1) var input_irradiance_variance_texture: texture_2d<f32>;
@group(0) @binding(2) var normal_history_texture: texture_2d<f32>;
@group(0) @binding(3) var depth_history_texture: texture_2d<f32>;

@group(1) @binding(1) var<uniform> camera_position: vec3<f32>;
@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(1) @binding(4) var<uniform> camera_view_rotation: mat3x3<f32>;

struct HbgiSvgfUniform {
    params: vec4<u32>,
}

@group(2) @binding(0) var<uniform> hbgi_svgf_settings: HbgiSvgfUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) bent_ao: vec4<f32>,
    @location(1) irradiance_variance: vec4<f32>,
}

const MIN_VARIANCE: f32 = 1e-4;
const KERNEL: array<f32, 5> = array<f32, 5>(1.0 / 16.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 4.0, 1.0 / 16.0);
const SIGMA_Z: f32 = 1.0; // Paper: 1.0
const SIGMA_N: f32 = 64.0; // Paper: 128.0
const SIGMA_L: f32 = 32.0; // Paper: 4.0

fn safe_normalize3(v: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3f(0.0, 0.0, 1.0);
}

fn clamp_coord(coord: vec2i, max_coord: vec2i) -> vec2i {
    return clamp(coord, vec2i(0), max_coord);
}

fn is_offscreen_coord(coord: vec2i, max_coord: vec2i) -> bool {
    return any(coord < vec2i(0)) || any(coord > max_coord);
}

fn luminance(rgb: vec3f) -> f32 {
    return dot(rgb, vec3f(0.2126, 0.7152, 0.0722));
}

fn sample_half_res_uv(pixel_coord: vec2i, dims: vec2i) -> vec2f {
    return (vec2f(pixel_coord) + vec2f(0.5)) / vec2f(dims);
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

fn linear_view_depth_from_world_position(world_position: vec3f) -> f32 {
    return max(dot(world_position - camera_position, camera_view_rotation[2]), 0.0);
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
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let center_uv = in.tex_coords;

    let half_dims_u = textureDimensions(depth_history_texture);
    let half_dims = vec2f(half_dims_u);
    let half_max_coords = vec2i(half_dims_u) - vec2i(1);
    let center_half_coords = min(vec2i(center_uv * half_dims), half_max_coords);

    let first_pass = hbgi_svgf_settings.params.y != 0u;
    let step = i32(hbgi_svgf_settings.params.x);

    let center_depth_moments = textureLoad(depth_history_texture, center_half_coords, 0);
    let center_irradiance_variance = textureLoad(input_irradiance_variance_texture, center_half_coords, 0);
    let center_bent_ao = textureLoad(input_bent_ao_texture, center_half_coords, 0);
    if (center_depth_moments.x <= 0.0) {
        return FragmentOutput(
            center_bent_ao,
            center_irradiance_variance
        );
    }

    let center_world_position = reconstruct_world_position_from_depth(center_uv, center_depth_moments.x);
    let center_linear_depth = linear_view_depth_from_world_position(center_world_position);
    let center_normal = textureLoad(normal_history_texture, center_half_coords, 0).xyz;
    let center_history_length = center_depth_moments.w;
    var center_variance = center_irradiance_variance.w;

    let center_depth_gradient = vec2f(
        dpdx(center_linear_depth),
        dpdy(center_linear_depth),
    );

    let l_p = luminance(center_irradiance_variance.rgb);

    var irradiance_acc = vec3f(0.0);
    var denom_acc = 0.0;
    var variance_acc = 0.0;
    var bent_acc = vec3f(0.0);
    var ao_acc = 0.0;

    for (var y: i32 = -2; y <= 2; y += 1) {
        for (var x: i32 = -2; x <= 2; x += 1) {
            let raw_neighbor_half_coords = vec2i(
                center_half_coords.x + x * step,
                center_half_coords.y + y * step
            );
            if (is_offscreen_coord(raw_neighbor_half_coords, half_max_coords)) {
                continue;
            }
            let neighbor_half_coords = raw_neighbor_half_coords;
            let neighbor_uv = sample_half_res_uv(neighbor_half_coords, vec2i(half_dims_u));
            let depth_moments = textureLoad(depth_history_texture, neighbor_half_coords, 0);
            if (depth_moments.r <= 0.0) {
                continue;
            }
            let irradiance_variance = textureLoad(input_irradiance_variance_texture, neighbor_half_coords, 0);
            let normal = textureLoad(normal_history_texture, neighbor_half_coords, 0).xyz;
            let neighbor_world_position = reconstruct_world_position_from_depth(neighbor_uv, depth_moments.x);
            let neighbor_linear_depth = linear_view_depth_from_world_position(neighbor_world_position);
            let bent_ao = textureLoad(input_bent_ao_texture, neighbor_half_coords, 0);

            let c_q = irradiance_variance.rgb;
            let var_c_q = irradiance_variance.a;
            let l_q = luminance(irradiance_variance.rgb);

            let h_q = KERNEL[x + 2] * KERNEL[y + 2];

            // 4.4 eq. 3
            let w_z_nom = abs(center_linear_depth - neighbor_linear_depth);
            let p_minus_q = vec2f(center_half_coords - neighbor_half_coords);
            let w_z_denom = max(SIGMA_Z * abs(dot(center_depth_gradient, p_minus_q)), 1e-3);
            let w_z = exp(-1.0 * w_z_nom / w_z_denom);

            // 4.4 eq. 4
            let w_n = pow(max(0.0, dot(center_normal, normal)), SIGMA_N);

            // 4.4 eq. 5
            let w_l_nom = abs(l_p - l_q);
            let w_l_denom = max(SIGMA_L * sqrt(center_variance), 1e-3);
            let w_l = exp(-1.0 * w_l_nom / w_l_denom);

            // 4.3 eq. 2
            var w_pq = w_z * w_n * w_l;
            //w_pq = mix(1.0, w_pq, min(1.0, (center_depth_moments.w + depth_moments.w) / 32.0));

            // 4.3 eq. 1
            let hw = h_q * w_pq;
            irradiance_acc += hw * c_q;
            denom_acc += hw;
            variance_acc += h_q * h_q * w_pq * w_pq * var_c_q;

            bent_acc += bent_ao.xyz * hw;
            ao_acc += bent_ao.w * hw;
        }
    }

    if (denom_acc <= 1e-6) {
        return FragmentOutput(
            center_bent_ao,
            vec4f(center_irradiance_variance.rgb, max(center_variance, 0.0))
        );
    }

    let filtered_bent_normal = safe_normalize3(bent_acc);
    let filtered_ao = ao_acc / denom_acc;
    let filtered_irradiance = irradiance_acc / denom_acc;
    var filtered_variance = variance_acc / (denom_acc * denom_acc);

    return FragmentOutput(
        vec4f(filtered_bent_normal, filtered_ao),
        vec4f(filtered_irradiance, filtered_variance)
    );
}

@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;
@group(0) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;

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

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const MIN_FILTER_WEIGHT: f32 = 1e-6;
const CONFIDENCE_WEIGHT: f32 = 2.0;

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

fn fresnel_schlick_roughness(cos_theta: f32, F0: vec3f, roughness: f32) -> vec3f {
    return F0 + (max(vec3f(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
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
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 0.0),
        vec2(0.0, 1.0)
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2f(in.tex_coords.x, 1.0 - in.tex_coords.y);
    let surface_color_ao = textureSample(
        gbuffer_albedo_ao,
        gbuffer_albedo_ao_sampler,
        uv
    );
    var normal_roughness = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    );
    //normal_roughness.w = 0.0;
    let emissive_metallic = textureSample(
        gbuffer_emissive_metallic,
        gbuffer_emissive_metallic_sampler,
        uv
    );
    let lighting = textureSample(
        lighting_texture,
        lighting_texture_sampler,
        uv
    );
    let depth_dims = vec2i(textureDimensions(depth_texture));
    let depth_pixel = clamp(vec2i(uv * vec2f(depth_dims)), vec2i(0), depth_dims - vec2i(1));
    let depth = textureLoad(depth_texture, depth_pixel, 0);
    if (depth <= 0.0) {
        return lighting;
    }

    let world_position = reconstruct_world_position_from_depth(uv, depth);
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
    /*
    let ssr_radiance = textureSample(
        ssr_texture,
        ssr_texture_sampler,
        uv
    ).rgb;
    */

    let ssr_dims_u = textureDimensions(ssr_texture);
    let ssr_dims = vec2f(ssr_dims_u);
    let ssr_max_coords = vec2i(ssr_dims_u) - vec2i(1);
    let ssr_center_coords = min(vec2i(uv * ssr_dims), ssr_max_coords);
    let center_world_position = world_position;
    let center_roughness = surface_roughness;
    let center_ssr = textureLoad(ssr_texture, ssr_center_coords, 0);

    var radiance_acc = center_ssr.rgb;
    var weight_acc = 1.0;
    for (var y: i32 = -2; y <= 2; y += 1) {
        for (var x: i32 = -2; x <= 2; x += 1) {
            if (x == 0 && y == 0) {
                continue;
            }
            let neighbor_coords = ssr_center_coords + vec2i(x, y);
            if (any(neighbor_coords > vec2i(ssr_dims_u)- vec2i(1)) || any(neighbor_coords < vec2i(0))) {
                continue;
            }
            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / ssr_dims;
            let neighbor_normal_roughness = textureSampleLevel(
                gbuffer_normal_roughness,
                gbuffer_normal_roughness_sampler,
                neighbor_uv,
                0.0
            );
            let neighbor_depth_pixel = clamp(
                vec2i(neighbor_uv * vec2f(depth_dims)),
                vec2i(0),
                depth_dims - vec2i(1)
            );
            let neighbor_depth = textureLoad(depth_texture, neighbor_depth_pixel, 0);
            if (neighbor_depth <= 0.0) {
                continue;
            }
            let neighbor_world_position = reconstruct_world_position_from_depth(
                neighbor_uv,
                neighbor_depth
            );
            let neighbor_ssr = textureLoad(ssr_texture, neighbor_coords, 0);

            let delta = neighbor_world_position - center_world_position;
            let plane_distance = abs(dot(N, delta));
            let w_plane = 1.0 / max(plane_distance, MIN_FILTER_WEIGHT);

            let neighbor_normal = normalize(neighbor_normal_roughness.xyz);
            let NdotN = dot(N, neighbor_normal);
            let w_normal = clamp(NdotN, MIN_FILTER_WEIGHT, 1.0);

            let roughness_delta = abs(center_roughness - neighbor_normal_roughness.w);
            let w_roughness = clamp(1.0 - roughness_delta, MIN_FILTER_WEIGHT, 1.0);

            let w = w_plane * w_normal * w_roughness * pow(1.0 - center_ssr.w, CONFIDENCE_WEIGHT);

            radiance_acc += w * neighbor_ssr.rgb;
            weight_acc += w;
        }
    }

    let ssr_radiance = radiance_acc / weight_acc;

    //let ssr_specular = textureLoad(ssr_texture, ssr_center_coords, 0).rgb * (F_env * brdf_specular_lut.x + brdf_specular_lut.y) * ao;
    let ssr_specular = ssr_radiance * (F_env * brdf_specular_lut.x + brdf_specular_lut.y) * ao;

    return vec4f(lighting.rgb + ssr_specular, 1.0);
    //return vec4f(ssr_radiance, lighting.a);
    //return vec4f(textureLoad(ssr_texture, ssr_center_coords, 0).rgb, 1.0);
}

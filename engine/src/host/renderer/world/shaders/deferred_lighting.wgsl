@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;

@group(1) @binding(0) var<uniform> light_dir: vec3<f32>;
@group(1) @binding(1) var<uniform> light_col: vec3<f32>;
@group(1) @binding(2) var environment_texture: texture_cube<f32>;
@group(1) @binding(3) var environment_texture_sampler: sampler;
@group(1) @binding(4) var diffuse_irradiance_texture: texture_cube<f32>;
@group(1) @binding(5) var diffuse_irradiance_texture_sampler: sampler;
@group(1) @binding(6) var brdf_lut: texture_2d<f32>;
@group(1) @binding(7) var brdf_lut_sampler: sampler;
@group(1) @binding(8) var<uniform> environment_map_intensity: f32;
@group(1) @binding(9) var<uniform> point_light_count: vec4<u32>;
@group(1) @binding(10) var<uniform> point_light_positions_ranges: array<vec4<f32>, 64>;
@group(1) @binding(11) var<uniform> point_light_colors_intensities: array<vec4<f32>, 64>;

@group(2) @binding(0) var gbuffer_albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_albedo_ao_sampler: sampler;
@group(2) @binding(2) var gbuffer_normal_roughness: texture_2d<f32>;
@group(2) @binding(3) var gbuffer_normal_roughness_sampler: sampler;
@group(2) @binding(4) var gbuffer_emissive_metallic: texture_2d<f32>;
@group(2) @binding(5) var gbuffer_emissive_metallic_sampler: sampler;
@group(2) @binding(6) var gbuffer_world_position: texture_2d<f32>;
@group(2) @binding(7) var gbuffer_world_position_sampler: sampler;
@group(2) @binding(8) var gtao_texture: texture_2d<f32>;
@group(2) @binding(9) var gtao_texture_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const PI: f32 = 3.1415927;
const MAX_REFLECTION_LOD: f32 = 4.0;
const MAX_POINT_LIGHTS: u32 = 64u;

fn distribution_ggx(N: vec3f, H: vec3f, a: f32) -> f32 {
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let nom = a2;
    var denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return nom / denom;
}

fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;
    let denom = NdotV * (1.0 - k) + k;
    return NdotV / denom;
}

fn geometry_smith(N: vec3f, V: vec3f, L: vec3f, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx1 = geometry_schlick_ggx(NdotV, roughness);
    let ggx2 = geometry_schlick_ggx(NdotL, roughness);
    return ggx1 * ggx2;
}

fn fresnel_schlick(cos_theta: f32, F0: vec3f) -> vec3f {
    return F0 + (1.0 - F0) * pow(1.0 - cos_theta, 5.0);
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
    let normal_roughness = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    );
    let emissive_metallic = textureSample(
        gbuffer_emissive_metallic,
        gbuffer_emissive_metallic_sampler,
        uv
    );
    let world_position = textureSample(
        gbuffer_world_position,
        gbuffer_world_position_sampler,
        uv
    );
    let gtao = textureSample(
        gtao_texture,
        gtao_texture_sampler,
        uv
    ).r;
    if (world_position.w < 0.5) {
        return vec4f(0.0, 0.0, 0.0, 0.0);
    }

    let N = normalize(normal_roughness.xyz);
    let surface_roughness = normal_roughness.w;
    let surface_color = surface_color_ao.rgb;
    let ao = surface_color_ao.a;
    let surface_emissive = emissive_metallic.rgb;
    let surface_metallic = emissive_metallic.a;

    let V = normalize(camera_position - world_position.xyz);
    let R = reflect(-V, N);
    let prefiltered_color = textureSampleLevel(environment_texture, environment_texture_sampler, R, surface_roughness * MAX_REFLECTION_LOD).rgb;
    let F0 = mix(vec3f(0.04), surface_color, surface_metallic);

    var Lo = vec3f(0.0);
    {
        let L = normalize(-light_dir);
        let H = normalize(V + L);
        let radiance = light_col;

        let NDF = distribution_ggx(N, H, surface_roughness);
        let G = geometry_smith(N, V, L, surface_roughness);
        let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

        let omega_0 = max(dot(N, V), 0.0);
        let omega_i = max(dot(N, L), 0.0);
        let num = NDF * G * F;
        let denom = 4.0 * max(omega_0 * omega_i, 0.0001);
        let specular = num / denom;

        let k_d = (vec3f(1.0) - F) * (1.0 - surface_metallic);
        let NdotL = max(dot(N, L), 0.0);
        Lo += (k_d * surface_color / PI + specular) * radiance * NdotL;
    }

    let clamped_point_light_count = min(point_light_count.x, MAX_POINT_LIGHTS);
    for (var light_idx: u32 = 0u; light_idx < clamped_point_light_count; light_idx += 1u) {
        let point_light_position_range = point_light_positions_ranges[light_idx];
        let point_light_color_intensity = point_light_colors_intensities[light_idx];
        let to_light = point_light_position_range.xyz - world_position.xyz;
        let light_distance = length(to_light);
        let light_range = max(point_light_position_range.w, 0.001);
        if (light_distance >= light_range) {
            continue;
        }
        let L = to_light / max(light_distance, 0.0001);
        let H = normalize(V + L);
        let range_falloff = 1.0 - clamp(light_distance / light_range, 0.0, 1.0);
        let attenuation = (range_falloff * range_falloff) / max(light_distance * light_distance, 0.01);
        let radiance = point_light_color_intensity.rgb * point_light_color_intensity.w * attenuation;

        let NDF = distribution_ggx(N, H, surface_roughness);
        let G = geometry_smith(N, V, L, surface_roughness);
        let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

        let omega_0 = max(dot(N, V), 0.0);
        let omega_i = max(dot(N, L), 0.0);
        let num = NDF * G * F;
        let denom = 4.0 * max(omega_0 * omega_i, 0.0001);
        let specular = num / denom;

        let k_d = (vec3f(1.0) - F) * (1.0 - surface_metallic);
        let NdotL = max(dot(N, L), 0.0);
        Lo += (k_d * surface_color / PI + specular) * radiance * NdotL;
    }

    let F_env = fresnel_schlick_roughness(max(dot(N, V), 0.0), F0, surface_roughness);
    let k_s2 = F_env;
    var k_d2 = 1.0 - k_s2;
    k_d2 *= 1.0 - surface_metallic;

    let irradiance = textureSample(
        diffuse_irradiance_texture,
        diffuse_irradiance_texture_sampler,
        N
    ).rgb;
    let diffuse = irradiance * surface_color;
    let brdf = textureSample(
        brdf_lut,
        brdf_lut_sampler,
        vec2(max(dot(N, V), 0.0), 1.0 - surface_roughness)
    ).rg;
    let specular_env = prefiltered_color * (F_env * brdf.x + brdf.y);
    let final_ao = ao * gtao;
    let ambient = (k_d2 * diffuse + specular_env) * final_ao * environment_map_intensity;

    let col = ambient + Lo + surface_emissive;
    return vec4f(col, 1.0);
}

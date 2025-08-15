@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;

@group(1) @binding(0) var<uniform> light_dir: vec3<f32>;
@group(1) @binding(1) var<uniform> light_col: vec3<f32>;

@group(2) @binding(0) var<uniform> base_color_factor: vec4<f32>;
@group(2) @binding(1) var<uniform> metallic_factor: f32;
@group(2) @binding(2) var<uniform> roughness_factor: f32;
@group(2) @binding(3) var<uniform> emissive_factor: vec3<f32>;
@group(2) @binding(4) var normal_texture: texture_2d<f32>;
@group(2) @binding(5) var normal_texture_sampler: sampler;
@group(2) @binding(6) var occlusion_texture: texture_2d<f32>;
@group(2) @binding(7) var occlusion_texture_sampler: sampler;
@group(2) @binding(8) var emissive_texture: texture_2d<f32>;
@group(2) @binding(9) var emissive_texture_sampler: sampler;
@group(2) @binding(10) var base_color_texture: texture_2d<f32>;
@group(2) @binding(11) var base_color_texture_sampler: sampler;
@group(2) @binding(12) var metallic_roughness_texture: texture_2d<f32>;
@group(2) @binding(13) var metallic_roughness_texture_sampler: sampler;
@group(2) @binding(14) var<uniform> normal_texture_scale: f32;

@group(3) @binding(0) var environment_texture: texture_cube<f32>;
@group(3) @binding(1) var environment_texture_sampler: sampler;
@group(3) @binding(2) var diffuse_irradiance_texture: texture_cube<f32>;
@group(3) @binding(3) var diffuse_irradiance_texture_sampler: sampler;
@group(3) @binding(4) var brdf_lut: texture_2d<f32>;
@group(3) @binding(5) var brdf_lut_sampler: sampler;

struct InstanceInput {
    @location(0) m_1: vec4<f32>,
    @location(1) m_2: vec4<f32>,
    @location(2) m_3: vec4<f32>,
    @location(3) m_4: vec4<f32>,
    @location(4) itr_1: vec3<f32>,
    @location(5) itr_2: vec3<f32>,
    @location(6) itr_3: vec3<f32>,
}

struct VertexInput {
    @location(7) tangent: vec4<f32>,
    @location(8) weights: vec4<f32>,
    @location(9) position: vec3<f32>,
    @location(10) normal: vec3<f32>,
    // optimization: combining normal and occlusion tex coords
    @location(11) normal_tex_coords: vec4<f32>,
    //@location(12) occlusion_tex_coords: vec2<f32>,
    // optimization: combining emissive base color tex coords
    @location(12) emissive_base_color_tex_coords: vec4<f32>,
    @location(13) metallic_roughness_tex_coords: vec2<f32>,
    @location(14) joints: vec4<u32>, // reinterpreting u8 as u32, since u8 is not supported by wgsl
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,

    @location(0) tangent: vec3<f32>,
    @location(1) bitangent: vec3<f32>,
    @location(2) world_position: vec4<f32>,
    @location(3) normal: vec3<f32>,
    @location(4) normal_tex_coords: vec2<f32>,
    @location(5) occlusion_tex_coords: vec2<f32>,
    @location(6) emissive_tex_coords: vec2<f32>,
    @location(7) base_color_tex_coords: vec2<f32>,
    @location(8) metallic_roughness_tex_coords: vec2<f32>,
}

const PI: f32 = 3.1415927;
const MAX_REFLECTION_LOD: f32 = 4.0;

@vertex
fn vs_main(
    instance: InstanceInput,
    model: VertexInput,
) -> VertexOutput {
    let transform = mat4x4<f32>(
        instance.m_1,
        instance.m_2,
        instance.m_3,
        instance.m_4,
    );

    let inverse_transpose_rot = mat3x3<f32>(
        instance.itr_1,
        instance.itr_2,
        instance.itr_3,
    );

    var out: VertexOutput;
    out.clip_position = view_proj * transform * vec4<f32>(model.position, 1.0);

    //let N = normalize((transform * vec4f(model.normal, 0.0)).xyz);
    let N = normalize(inverse_transpose_rot * model.normal);
    //let T = normalize((transform * vec4f(model.tangent.xyz, 0.0)).xyz);
    let T = normalize(inverse_transpose_rot * model.tangent.xyz);
    //let B = normalize((transform * vec4f(model.tangent.w * cross(model.normal, model.tangent.xyz), 0.0)).xyz);
    let B = normalize(model.tangent.w * cross(N, T));
    out.normal = N;
    out.tangent = T;
    // http://www.mikktspace.com/
    out.bitangent = B;

    out.world_position = transform * vec4f(model.position, 1.0);

    out.normal_tex_coords = model.normal_tex_coords.xy;
    out.occlusion_tex_coords = model.normal_tex_coords.zw;

    out.emissive_tex_coords = model.emissive_base_color_tex_coords.xy;
    out.base_color_tex_coords = model.emissive_base_color_tex_coords.zw;
    out.metallic_roughness_tex_coords = model.metallic_roughness_tex_coords;

    return out;
}

fn distribution_ggx(N: vec3f, H: vec3f, a: f32) -> f32 {
    let a2 = a*a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH*NdotH;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal_sample = 
        textureSample(
            normal_texture,
            normal_texture_sampler,
            in.normal_tex_coords
        );
    var N = in.normal;
    if (normal_sample.w > 0.5) { // w encodes whether normal mapping should be used
        let n: vec3f = normalize(normal_sample.rgb * 2.0 - 1.0) * vec3f(normal_texture_scale, normal_texture_scale, 1.0);
        let TBN = mat3x3(in.tangent, in.bitangent, in.normal);
        N = normalize(TBN * n);
    }

    let V = normalize(camera_position - in.world_position.xyz);
    let R = reflect(-V, N);

    let surface_color =
        pow(
            textureSample(
                base_color_texture,
                base_color_texture_sampler,
                in.base_color_tex_coords
            ),
            vec4f(2.2)
        ) * base_color_factor;
    let metallic_roughness =
        textureSample(
            metallic_roughness_texture,
            metallic_roughness_texture_sampler,
            in.metallic_roughness_tex_coords
        );
    let surface_metallic = metallic_roughness.b * metallic_factor;
    let surface_roughness = metallic_roughness.g * roughness_factor;
    let surface_emissive_sample = 
        textureSample(
            emissive_texture,
            emissive_texture_sampler,
            in.emissive_tex_coords
        );
    let surface_emissive = surface_emissive_sample.rgb * emissive_factor;
    let ao = textureSample(occlusion_texture, occlusion_texture_sampler, in.occlusion_tex_coords);
    let prefiltered_color = textureSampleLevel(environment_texture, environment_texture_sampler, R, surface_roughness * MAX_REFLECTION_LOD).rgb;

    let F0 = mix(vec3f(0.04), surface_color.xyz, surface_metallic);

    // ---------------- //
    // For each light
    // ---------------- //
    let L = normalize(-light_dir); // reverse light direction (pointing from surface toward light source)
    let H = normalize(V + L);
    let radiance = light_col; // * attenuation (but we assume no attenuation for sunlight)

    let NDF = distribution_ggx(N, H, surface_roughness);
    let G = geometry_smith(N, V, L, surface_roughness);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let omega_0 = max(dot(N, V), 0.0);
    let omega_i = max(dot(N, L), 0.0);
    let num = NDF * G * F;
    let denom = 4.0 * max(omega_0 * omega_i, 0.0001);
    let specular = num / denom;

    let k_d = (vec3f(1.0) - F) * (1.0 - surface_metallic); // diffuse/refracted

    let NdotL = max(dot(N, L), 0.0);
    let Lo = (k_d * surface_color.xyz / PI + specular) * radiance * NdotL;

    // ---------------- //
    // IBL
    // ---------------- //

    let F_env = fresnel_schlick_roughness(max(dot(N, V), 0.0), F0, surface_roughness);

    let k_s2 = F_env;
    var k_d2 = 1.0 - k_s2;
    k_d2 *= 1.0 - surface_metallic;

    let irradiance = 
        textureSample(
            diffuse_irradiance_texture,
            diffuse_irradiance_texture_sampler,
            N
        ).rgb;
    let diffuse = irradiance * surface_color.rgb;

    // in opengl texture origin is at bottom-left so the y coordinate has to be flipped here (as opposed to learnopengl tutorial)
    let brdf = textureSample(brdf_lut, brdf_lut_sampler, vec2(max(dot(N, V), 0.0), 1 - surface_roughness)).rg;
    let specular_env = prefiltered_color * (F_env * brdf.x + brdf.y);
    let ambient = (k_d2 * diffuse + specular_env) * ao.r;

    // ---------------- //

    var col = ambient + Lo + (surface_emissive * surface_emissive_sample.a);

    return vec4f(col, 1.0);
}


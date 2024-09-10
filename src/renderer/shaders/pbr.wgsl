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

@group(3) @binding(0) var diffuse_irradiance_texture: texture_cube<f32>;
@group(3) @binding(1) var diffuse_irradiance_texture_sampler: sampler;

struct InstanceInput {
    @location(0) m_1: vec4<f32>,
    @location(1) m_2: vec4<f32>,
    @location(2) m_3: vec4<f32>,
    @location(3) m_4: vec4<f32>,
}

struct VertexInput {
    @location(4) tangent: vec4<f32>,
    @location(5) weights: vec4<f32>,
    @location(6) position: vec3<f32>,
    @location(7) normal: vec3<f32>,
    // optimization: combining normal and occlusion tex coords
    @location(8) normal_tex_coords: vec4<f32>,
    //@location(12) occlusion_tex_coords: vec2<f32>,
    // optimization: combining emissive base color tex coords
    @location(9) emissive_base_color_tex_coords: vec4<f32>,
    @location(10) metallic_roughness_tex_coords: vec2<f32>,
    @location(11) joints: vec4<u32>, // reinterpreting u8 as u32, since u8 is not supported by wgsl
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

    var out: VertexOutput;
    out.clip_position = view_proj * transform * vec4<f32>(model.position, 1.0);

    out.normal = normalize((transform * vec4f(model.normal, 1.)).xyz);
    //out.normal = model.normal;
    out.tangent = normalize((transform * vec4f(model.tangent.xyz, 1.)).xyz);
    //out.tangent = model.tangent.xyz;
    // http://www.mikktspace.com/
    out.bitangent = normalize(model.tangent.w * cross(out.normal, out.tangent));

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
        let n = normal_sample.xyz * 2.0 - 1.0;
        let TBN = mat3x3(in.tangent, in.bitangent, in.normal);
        N = normalize(TBN * n);
    }

    let V = normalize(camera_position - in.world_position.xyz);

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

    let k_s2 = fresnel_schlick(max(dot(N, V), 0.0), F0);
    var k_d2 = 1.0 - k_s2;
    k_d2 *= 1.0 - surface_metallic;

    let irradiance = 
        textureSample(
            diffuse_irradiance_texture,
            diffuse_irradiance_texture_sampler,
            N
        ).rgb;
    let diffuse = irradiance * surface_color.rgb;
    let ambient = (k_d2 * diffuse) * ao.r;
    var col = ambient + Lo + (surface_emissive * surface_emissive_sample.a);

    col = col / (col + vec3f(1.0));
    col = pow(col, vec3(1.0 / 2.2));

    return vec4f(col, 1.0);
}


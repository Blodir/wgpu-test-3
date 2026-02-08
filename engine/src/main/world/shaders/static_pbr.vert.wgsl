@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;

@group(1) @binding(0) var<uniform> light_dir: vec3<f32>;
@group(1) @binding(1) var<uniform> light_col: vec3<f32>;
@group(1) @binding(2) var environment_texture: texture_cube<f32>;
@group(1) @binding(3) var environment_texture_sampler: sampler;
@group(1) @binding(4) var diffuse_irradiance_texture: texture_cube<f32>;
@group(1) @binding(5) var diffuse_irradiance_texture_sampler: sampler;
@group(1) @binding(6) var brdf_lut: texture_2d<f32>;
@group(1) @binding(7) var brdf_lut_sampler: sampler;

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
    @location(8) position: vec3<f32>,
    @location(9) normal: vec3<f32>,
    // optimization: combining normal and occlusion tex coords
    @location(10) normal_tex_coords: vec4<f32>,
    //@location(12) occlusion_tex_coords: vec2<f32>,
    // optimization: combining emissive base color tex coords
    @location(11) emissive_base_color_tex_coords: vec4<f32>,
    @location(12) metallic_roughness_tex_coords: vec2<f32>,
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

    let position = model.position;
    let normal = normalize(model.normal);
    let tangent = normalize(model.tangent.xyz);

    var out: VertexOutput;
    out.clip_position = view_proj * transform * vec4<f32>(position, 1.0);

    let N = normalize(inverse_transpose_rot * normal);
    let T = normalize(inverse_transpose_rot * tangent);
    let B = normalize(model.tangent.w * cross(N, T));
    out.normal = N;
    out.tangent = T;
    out.bitangent = B;

    out.world_position = transform * vec4f(position, 1.0);

    out.normal_tex_coords = model.normal_tex_coords.xy;
    out.occlusion_tex_coords = model.normal_tex_coords.zw;

    out.emissive_tex_coords = model.emissive_base_color_tex_coords.xy;
    out.base_color_tex_coords = model.emissive_base_color_tex_coords.zw;
    out.metallic_roughness_tex_coords = model.metallic_roughness_tex_coords;

    return out;
}

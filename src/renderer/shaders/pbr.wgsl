@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;

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
    @location(8) normal_tex_coords: vec2<f32>,
    @location(9) occlusion_tex_coords: vec2<f32>,
    @location(10) emissive_tex_coords: vec2<f32>,
    @location(11) base_color_tex_coords: vec2<f32>,
    @location(12) metallic_roughness_tex_coords: vec2<f32>,
    @location(13) joints: vec4<u32>, // reinterpreting u8 as u32, since u8 is not supported by wgsl
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) base_color_tex_coords: vec2<f32>,
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
    var out: VertexOutput;
    out.clip_position = view_proj * transform * vec4<f32>(model.position, 1.0);
    out.normal = model.normal;
    out.base_color_tex_coords = model.base_color_tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var d = (dot(in.normal, light_dir) + 1.) / 2.;
    var base_color = textureSample(base_color_texture, base_color_texture_sampler, in.base_color_tex_coords) * base_color_factor;
    var col = d * base_color.xyz;
    return vec4<f32>(col, 1.);
}


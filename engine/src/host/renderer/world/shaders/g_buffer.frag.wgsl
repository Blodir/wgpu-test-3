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
@group(2) @binding(15) var<uniform> alpha_mask_enabled: u32;
@group(2) @binding(16) var<uniform> alpha_cutoff: f32;
@group(2) @binding(17) var<uniform> alpha_blend_enabled: u32;

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

struct GBufferOutput {
    @location(0) albedo_ao: vec4<f32>,
    @location(1) normal_roughness: vec4<f32>,
    @location(2) emissive_metallic: vec4<f32>,
    @location(3) world_position: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
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

    let surface_color =
        textureSample(
            base_color_texture,
            base_color_texture_sampler,
            in.base_color_tex_coords
        ) * base_color_factor;
    if (alpha_blend_enabled == 1u) {
        discard;
    }
    if (alpha_mask_enabled == 1u && surface_color.a < alpha_cutoff) {
        discard;
    }
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

    var out: GBufferOutput;
    out.albedo_ao = vec4f(surface_color.rgb, ao.r);
    out.normal_roughness = vec4f(normalize(N), surface_roughness);
    out.emissive_metallic = vec4f(surface_emissive, surface_metallic);
    out.world_position = vec4f(in.world_position.xyz, 1.0);
    return out;
}

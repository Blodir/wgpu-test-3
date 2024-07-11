@group(0) @binding(0)
var<uniform> view_proj: mat4x4<f32>;
@group(1) @binding(0)
var<uniform> view_invert_transpose: mat4x4<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = view_proj * vec4<f32>(model.position, 1.0);

    let view_invert_transpose_3x3 = mat3x3<f32>(
        view_invert_transpose[0].xyz,
        view_invert_transpose[1].xyz,
        view_invert_transpose[2].xyz
    );
    out.normal = normalize(view_invert_transpose_3x3 * model.normal);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var lightDir = normalize(vec3<f32>(-1., 1., 1.));
    //var lightPos = vec3<f32>(0., -5000., -5000.);
    var color = dot(in.normal, lightDir);
    //var color = dot(in.normal, normalize(in.clip_position.xyz - lightPos));
    //return vec4<f32>((in.normal + 1.) / 2., 1.0);
    return vec4<f32>(color, color, color, 1.0);
    //return vec4<f32>(1., 0., 0., 1.);
}


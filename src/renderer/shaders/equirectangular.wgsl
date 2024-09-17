@group(0) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(0) @binding(1) var equirectangular_texture_sampler: sampler;

@group(1) @binding(0) var<uniform> face_rot: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec3<f32>,
}

const PI: f32 = 3.1415927;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var out: VertexOutput;
    var pos = array(
        vec2(1.0, 1.0),
        vec2(1.0, -1.0),
        vec2(-1.0, -1.0),
        vec2(-1.0, 1.0)
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0, 1);
    out.position = (vec4<f32>(pos[vertex_index], 1, 1) * face_rot).xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let rd = normalize(in.position.xyz);

    // angle from positive y
    let y = acos(rd.y) / PI;

    // angle with positive x as meridian
    var t = atan2(rd.z, rd.x);
    if (t < 0) {
        t += 2 * PI;
    }
    let x = t / (2 * PI);

    let col =
        textureSampleLevel(
            equirectangular_texture,
            equirectangular_texture_sampler,
            vec2f(x, y),
            0.0
        );

    return col;
    //return pow(col, vec4(1.0/2.2));
}


@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var out: VertexOutput;
    var pos = array(
        vec2(-1.0, -1.0),
        vec2(3.0, -1.0),
        vec2(-1.0, 3.0),
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0, 1);
    out.uv = (pos[vertex_index] + vec2f(1, 1)) / 2.0;
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(texture, texture_sampler, in.uv);
    return vec4f(sample.rgb, 1.0);
}


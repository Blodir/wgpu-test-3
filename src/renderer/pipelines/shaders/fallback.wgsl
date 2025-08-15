struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) position: vec4<f32>
}

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
    out.position = vec4<f32>(pos[vertex_index], 0, 1);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv: vec2<f32> = (in.position.xy + 1.0) / 2.;
    return vec4<f32>(uv, 0.0, 1.0);
}


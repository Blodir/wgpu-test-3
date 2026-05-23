@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
@group(0) @binding(5) var<uniform> prev_view_proj: mat4x4<f32>;

struct BoneMat34 {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
}

@group(3) @binding(0) var<storage, read> curr_bones: array<BoneMat34>;
@group(3) @binding(1) var<storage, read> prev_bones: array<BoneMat34>;

struct InstanceData {
    m_1: vec4<f32>,
    m_2: vec4<f32>,
    m_3: vec4<f32>,
    m_4: vec4<f32>,
    itr_1: vec4<f32>,
    itr_2: vec4<f32>,
    itr_3: vec4<f32>,
    offsets: vec4<u32>,
}

@group(3) @binding(2) var<storage, read> curr_instances: array<InstanceData>;
@group(3) @binding(3) var<storage, read> prev_instances: array<InstanceData>;

struct VertexInput {
    @location(8) tangent: vec4<f32>,
    @location(9) weights: vec4<f32>,
    @location(10) position: vec3<f32>,
    @location(11) normal: vec3<f32>,
    @location(12) normal_tex_coords: vec4<f32>,
    @location(13) emissive_base_color_tex_coords: vec4<f32>,
    @location(14) metallic_roughness_tex_coords: vec2<f32>,
    @location(15) joints: vec4<u32>,
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
    @location(9) curr_clip: vec4<f32>,
    @location(10) prev_clip: vec4<f32>,
}

fn apply_bone_to_position(mat: BoneMat34, position: vec4<f32>) -> vec3<f32> {
    return vec3f(
        dot(mat.r0, position),
        dot(mat.r1, position),
        dot(mat.r2, position),
    );
}

fn apply_bone_to_direction(mat: BoneMat34, direction: vec3<f32>) -> vec3<f32> {
    return vec3f(
        dot(mat.r0.xyz, direction),
        dot(mat.r1.xyz, direction),
        dot(mat.r2.xyz, direction),
    );
}

fn skin_position_curr(
    palette_offset: u32,
    joints: vec4<u32>,
    weights: vec4<f32>,
    position: vec3<f32>,
) -> vec3<f32> {
    let pos = vec4f(position, 1.0);
    var skinned = vec3f(0.0);

    skinned += weights.x * apply_bone_to_position(curr_bones[palette_offset + joints.x], pos);
    skinned += weights.y * apply_bone_to_position(curr_bones[palette_offset + joints.y], pos);
    skinned += weights.z * apply_bone_to_position(curr_bones[palette_offset + joints.z], pos);
    skinned += weights.w * apply_bone_to_position(curr_bones[palette_offset + joints.w], pos);

    return skinned;
}

fn skin_position_prev(
    palette_offset: u32,
    joints: vec4<u32>,
    weights: vec4<f32>,
    position: vec3<f32>,
) -> vec3<f32> {
    let pos = vec4f(position, 1.0);
    var skinned = vec3f(0.0);

    skinned += weights.x * apply_bone_to_position(prev_bones[palette_offset + joints.x], pos);
    skinned += weights.y * apply_bone_to_position(prev_bones[palette_offset + joints.y], pos);
    skinned += weights.z * apply_bone_to_position(prev_bones[palette_offset + joints.z], pos);
    skinned += weights.w * apply_bone_to_position(prev_bones[palette_offset + joints.w], pos);

    return skinned;
}

fn skin_direction_curr(
    palette_offset: u32,
    joints: vec4<u32>,
    weights: vec4<f32>,
    direction: vec3<f32>,
) -> vec3<f32> {
    var skinned = vec3f(0.0);

    skinned += weights.x * apply_bone_to_direction(curr_bones[palette_offset + joints.x], direction);
    skinned += weights.y * apply_bone_to_direction(curr_bones[palette_offset + joints.y], direction);
    skinned += weights.z * apply_bone_to_direction(curr_bones[palette_offset + joints.z], direction);
    skinned += weights.w * apply_bone_to_direction(curr_bones[palette_offset + joints.w], direction);

    return skinned;
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
    model: VertexInput,
) -> VertexOutput {
    let instance = curr_instances[instance_index];
    let curr_transform = mat4x4<f32>(
        instance.m_1,
        instance.m_2,
        instance.m_3,
        instance.m_4,
    );

    let inverse_transpose_rot = mat3x3<f32>(
        instance.itr_1.xyz,
        instance.itr_2.xyz,
        instance.itr_3.xyz,
    );

    let palette_offset = instance.offsets.x;
    let curr_pos = skin_position_curr(palette_offset, model.joints, model.weights, model.position);
    let curr_normal = normalize(skin_direction_curr(
        palette_offset,
        model.joints,
        model.weights,
        model.normal,
    ));
    let curr_tangent = normalize(skin_direction_curr(
        palette_offset,
        model.joints,
        model.weights,
        model.tangent.xyz,
    ));

    let has_prev_pose = instance.offsets.y != 0xffffffffu;
    var prev_pos = curr_pos;
    var prev_transform = curr_transform;
    if (has_prev_pose) {
        let prev_instance = prev_instances[instance.offsets.y];
        prev_transform = mat4x4<f32>(
            prev_instance.m_1,
            prev_instance.m_2,
            prev_instance.m_3,
            prev_instance.m_4,
        );
        prev_pos = skin_position_prev(
            prev_instance.offsets.x,
            model.joints,
            model.weights,
            model.position,
        );
    }

    let curr_clip = view_proj * curr_transform * vec4<f32>(curr_pos, 1.0);
    var prev_clip = curr_clip;
    if (has_prev_pose) {
        prev_clip = prev_view_proj * prev_transform * vec4<f32>(prev_pos, 1.0);
    }

    var out: VertexOutput;
    out.clip_position = curr_clip;

    let N = normalize(inverse_transpose_rot * curr_normal);
    let T = normalize(inverse_transpose_rot * curr_tangent);
    let B = normalize(model.tangent.w * cross(N, T));
    out.normal = N;
    out.tangent = T;
    out.bitangent = B;

    out.world_position = curr_transform * vec4f(curr_pos, 1.0);

    out.normal_tex_coords = model.normal_tex_coords.xy;
    out.occlusion_tex_coords = model.normal_tex_coords.zw;

    out.emissive_tex_coords = model.emissive_base_color_tex_coords.xy;
    out.base_color_tex_coords = model.emissive_base_color_tex_coords.zw;
    out.metallic_roughness_tex_coords = model.metallic_roughness_tex_coords;
    out.curr_clip = curr_clip;
    out.prev_clip = prev_clip;

    return out;
}

@group(0) @binding(0) var<uniform> sun_shadow_light_view_proj: mat4x4<f32>;

struct BoneMat34 {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
};

@group(1) @binding(0) var<storage, read> bones: array<BoneMat34>;

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

@group(1) @binding(1) var<storage, read> instances: array<InstanceData>;

struct VertexInput {
    @location(9) weights: vec4<f32>,
    @location(10) position: vec3<f32>,
    @location(15) joints: vec4<u32>,
}

fn apply_bone_to_position(mat: BoneMat34, position: vec4<f32>) -> vec3<f32> {
    return vec3f(
        dot(mat.r0, position),
        dot(mat.r1, position),
        dot(mat.r2, position),
    );
}

fn skin_position(
    palette_offset: u32,
    joints: vec4<u32>,
    weights: vec4<f32>,
    position: vec3<f32>,
) -> vec3<f32> {
    let pos = vec4f(position, 1.0);
    var skinned = vec3f(0.0);

    let b0 = bones[palette_offset + joints.x];
    skinned += weights.x * apply_bone_to_position(b0, pos);

    let b1 = bones[palette_offset + joints.y];
    skinned += weights.y * apply_bone_to_position(b1, pos);

    let b2 = bones[palette_offset + joints.z];
    skinned += weights.z * apply_bone_to_position(b2, pos);

    let b3 = bones[palette_offset + joints.w];
    skinned += weights.w * apply_bone_to_position(b3, pos);

    return skinned;
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
    model: VertexInput,
) -> @builtin(position) vec4<f32> {
    let instance = instances[instance_index];
    let transform = mat4x4<f32>(
        instance.m_1,
        instance.m_2,
        instance.m_3,
        instance.m_4,
    );
    let skinned_position = skin_position(
        instance.offsets.x,
        model.joints,
        model.weights,
        model.position,
    );
    let world_position = transform * vec4<f32>(skinned_position, 1.0);
    return sun_shadow_light_view_proj * world_position;
}

use gltf::animation::Interpolation;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    PrimitiveGroup(u32),
    SkeletonJoint(u32),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AnimationClip {
    pub duration: f32,
    pub tracks: Vec<Track>,
    pub primitive_groups: Vec<Vec<u32>>,
    pub binary_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Track {
    pub target: Target,

    /// If Some, T/R/S share the same time array.
    pub shared_times: Option<BinRef>,

    pub translation: Option<Sampler3>,
    pub rotation: Option<SamplerQuat>,
    pub scale: Option<Sampler3>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BinRef {
    pub offset: u32,
    pub count: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sampler3 {
    pub times: Option<BinRef>, // None -> use Track.shared_times
    pub values: BinRef,
    pub interpolation: Interpolation,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SamplerQuat {
    pub times: Option<BinRef>,
    pub values: BinRef,
    pub interpolation: Interpolation,
}

// Runtime animation format

use glam::{Quat, Vec3};

use crate::main::assets::io::asset_formats::animationfile::{Interpolation, Target};

pub struct AnimationClip {
    pub duration: f32,
    pub tracks: Vec<Track>,
    pub primitive_groups: Vec<Vec<u32>>,
}

pub struct Track {
    pub target: Target,
    pub shared_times: Option<Box<[f32]>>, // if all TRS share same time array
    pub translation: Option<Channel<Vec3>>,
    pub rotation:    Option<Channel<Quat>>,
    pub scale:       Option<Channel<Vec3>>,
}

pub struct Channel<T> {
    pub times: Option<Box<[f32]>>,  // None = use Track.shared_times
    pub values: Box<[T]>,
    pub interpolation: Interpolation,
}

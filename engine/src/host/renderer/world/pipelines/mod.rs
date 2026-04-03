pub mod post_processing;
pub mod skinned_pbr;
pub mod skybox;
pub mod static_pbr;

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub enum MeshPipelineKind {
    StaticPbr,
    SkinnedPbr,
}

pub mod post_processing;
pub mod skinned_pbr;
pub mod static_pbr;
pub mod skybox;

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub enum MeshPipelineKind {
    StaticPbr, SkinnedPbr
}

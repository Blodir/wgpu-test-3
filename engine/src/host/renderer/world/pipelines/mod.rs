pub mod deferred_lighting;
pub mod g_buffer;
pub mod gtao;
pub mod post_processing;
pub mod skinned_pbr;
pub mod skybox;
pub mod static_pbr;

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub enum MeshPipelineKind {
    StaticPbr,
    SkinnedPbr,
}

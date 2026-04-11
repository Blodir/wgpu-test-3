pub mod deferred_lighting;
pub mod g_buffer;
pub mod gtao;
pub mod history;
pub mod motion_vectors;
pub mod post_processing;
pub mod skinned_pbr;
pub mod skybox;
pub mod static_pbr;
pub mod sun_shadow;

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub enum MeshPipelineKind {
    StaticPbr,
    SkinnedPbr,
}

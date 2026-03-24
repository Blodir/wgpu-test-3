use std::{cell::RefCell, rc::Rc};

use crate::game::{
    animator::AnimationGraph,
    assets::registry::ResourceRegistry,
    scene_tree::{Scene, SceneNodeId},
    sim::InputEvent,
};

pub type BuildUiFn = fn(ctx: &egui::Context, frame_idx: u32);

pub trait GameTrait: Send + Sync {
    fn init(
        &mut self,
        resource_registry: &Rc<RefCell<ResourceRegistry>>,
    ) -> (Scene, Vec<AnimationGraph>);
    fn update(
        &mut self,
        scene: &mut Scene,
        resource_registry: &Rc<RefCell<ResourceRegistry>>,
        animation_graphs: &Vec<AnimationGraph>,
        node: SceneNodeId,
        dt: f32,
    );
    fn consume_input(&mut self, scene: &mut Scene, event: InputEvent);
    fn build_ui(_ctx: &egui::Context, _frame_idx: u32) {}
}

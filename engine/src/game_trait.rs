use std::{cell::RefCell, rc::Rc};

use crate::game::{
    animator::AnimationGraph,
    assets::registry::ResourceRegistry,
    scene_tree::{Scene, SceneNodeId},
};

pub enum InputEvent<C> {
    DeviceEvent(winit::event::DeviceEvent),
    WindowEvent(winit::event::WindowEvent),
    AspectChange(f32),
    Ui(C),
    Exit,
}

pub type BuildUiFn<S, C> =
    fn(ctx: &egui::Context, frame_idx: u32, snapshot: Option<&S>, emit: &mut dyn FnMut(C));

pub trait SimTrait {
    type UiSnapshot;
    type UiCommand;

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
    fn consume_input(&mut self, scene: &mut Scene, event: InputEvent<Self::UiCommand>);
    fn build_ui_snapshot(&mut self, scene: &Scene, tick: u64) -> Self::UiSnapshot;
}

pub trait UiTrait {
    type UiSnapshot;
    type UiCommand;

    fn build_ui(
        _ctx: &egui::Context,
        _frame_idx: u32,
        _snapshot: Option<&<Self as UiTrait>::UiSnapshot>,
        _emit: &mut dyn FnMut(<Self as UiTrait>::UiCommand),
    ) {
    }
}

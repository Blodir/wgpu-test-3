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

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderDebugInfo {
    pub frame_index: u32,
    pub fps: f32,
    pub frame_time_ms: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SimDebugInfo {
    pub fps: f32,
    pub frame_time_ms: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugInfo {
    pub render: RenderDebugInfo,
    pub sim: SimDebugInfo,
}

pub type BuildUiFn<S, C> =
    fn(ctx: &egui::Context, snapshot: Option<&S>, debug_info: &DebugInfo, emit: &mut dyn FnMut(C));

pub trait SimTrait {
    type VarSnapshot;
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
    fn build_var_snapshot(&mut self, scene: &Scene, tick: u64) -> Self::VarSnapshot;
}

pub trait UiTrait {
    type VarSnapshot;
    type UiCommand;

    fn build_ui(
        _ctx: &egui::Context,
        _snapshot: Option<&<Self as UiTrait>::VarSnapshot>,
        _debug_info: &DebugInfo,
        _emit: &mut dyn FnMut(<Self as UiTrait>::UiCommand),
    ) {
    }
}

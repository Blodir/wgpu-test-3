use std::{cell::RefCell, io};

use engine::{
    game::{
        animator::{self, AnimationGraph, Animator},
        assets::registry::RegistryExt as _,
        camera::Camera,
        scene_tree::{
            AnimatedModel, Environment, Node, RenderDataType, Scene, SceneNodeId, StaticModel, Sun,
        },
    },
    game_trait::{InputEvent, SimTrait, UiTrait},
    run,
};
use generational_arena::Arena;
use glam::{Mat4, Quat, Vec3};
use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

struct Game {
    shift_is_pressed: bool,
    mouse_btn_is_pressed: bool,
}

struct UiSnapshot {
    global_time_sec: f32,
    camera_distance: f32,
    shift_is_pressed: bool,
}

enum UiCommand {
    SetCameraDistance(f32),
}

impl Game {
    fn new() -> Self {
        Self {
            shift_is_pressed: false,
            mouse_btn_is_pressed: false,
        }
    }
}
impl SimTrait for Game {
    type UiSnapshot = UiSnapshot;
    type UiCommand = UiCommand;

    fn init(
        &mut self,
        resource_registry: &std::rc::Rc<
            std::cell::RefCell<engine::game::assets::registry::ResourceRegistry>,
        >,
    ) -> (
        engine::game::scene_tree::Scene,
        Vec<engine::game::animator::AnimationGraph>,
    ) {
        let mut nodes = Arena::new();

        let animation_graph = AnimationGraph {
            states: vec![
                animator::State {
                    clip_idx: 0,
                    time_wrap: animator::TimeWrapMode::Repeat,
                    boundary_mode: animator::BoundaryMode::Closed,
                    speed: 1.0,
                },
                animator::State {
                    clip_idx: 1,
                    time_wrap: animator::TimeWrapMode::Repeat,
                    boundary_mode: animator::BoundaryMode::Closed,
                    speed: 1.0,
                },
                animator::State {
                    clip_idx: 2,
                    time_wrap: animator::TimeWrapMode::Repeat,
                    boundary_mode: animator::BoundaryMode::Closed,
                    speed: 1.0,
                },
            ],
            transitions: vec![
                animator::Transition {
                    blend_time: 0.5,
                    to: 1,
                }, // look -> walk
                animator::Transition {
                    blend_time: 0.5,
                    to: 0,
                }, // walk -> look
                animator::Transition {
                    blend_time: 0.5,
                    to: 2,
                }, // walk -> run
                animator::Transition {
                    blend_time: 0.5,
                    to: 1,
                }, // run -> walk
            ],
        };
        let animation_graphs = vec![animation_graph];

        //let model_handle = resource_registry.request_model("assets/local/Lantern/Lantern.json");
        let model_handle = resource_registry.request_model("assets/local/Sponza/Sponza.json");

        let mut children = vec![];
        let grid_size = 1;
        for i in 0..grid_size {
            for j in 0..grid_size {
                let spacing = 200.0;
                let x = i as f32 * spacing - ((grid_size as f32 * spacing) / 2.0);
                let z = j as f32 * spacing - ((grid_size as f32 * spacing) / 2.0);
                //let transform = Mat4::from_translation(Vec3::new(x, 0.0, z));

                let transform = Mat4::from_scale_rotation_translation(
                    Vec3::new(10.0, 10.0, 10.0),
                    Quat::IDENTITY,
                    Vec3::new(0.0, 0.0, 0.0),
                );

                //let render_data = RenderDataType::AnimatedModel(AnimatedModel { model: model_handle.clone(), animator: Animator::new(0, 0), last_visible_frame: RefCell::new(u32::MAX) });

                let render_data = RenderDataType::Model(StaticModel {
                    handle: model_handle.clone(),
                    last_visible_frame: RefCell::new(u32::MAX),
                });

                let child = nodes.insert(Node {
                    parent: None,
                    children: vec![],
                    transform,
                    transform_last_mut: 0,
                    render_data,
                });
                children.push(SceneNodeId(child));
            }
        }

        let root_handle = nodes.insert(Node {
            parent: None,
            children,
            transform: Mat4::IDENTITY,
            transform_last_mut: 0,
            render_data: RenderDataType::None,
        });

        let environment = Environment {
            sun: Sun::default(),
            prefiltered: resource_registry.request_texture(
                "assets/kloofendal_overcast_puresky_8k.prefiltered.dds",
                true,
            ),
            di: resource_registry
                .request_texture("assets/kloofendal_overcast_puresky_8k.di.dds", true),
            brdf: resource_registry.request_texture("assets/brdf_lut.png", false),
        };

        let scene = Scene {
            root: SceneNodeId(root_handle),
            nodes,
            environment,
            camera: Camera::default(),
            global_time_sec: 0.0,
        };

        (scene, animation_graphs)
    }

    fn update(
        &mut self,
        scene: &mut engine::game::scene_tree::Scene,
        resource_registry: &std::rc::Rc<
            std::cell::RefCell<engine::game::assets::registry::ResourceRegistry>,
        >,
        animation_graphs: &Vec<engine::game::animator::AnimationGraph>,
        node: engine::game::scene_tree::SceneNodeId,
        dt: f32,
    ) {
        let node = scene.nodes.get_mut(node.into()).unwrap();
        match &mut node.render_data {
            RenderDataType::None => (),
            RenderDataType::Model(_model_handle) => (),
            RenderDataType::AnimatedModel(animated_model) => {
                // TODO remove this after testing
                // automatically transition for fun

                let cycle_duration = 16.0;
                let phase = scene.global_time_sec % cycle_duration;
                let a = cycle_duration / 4.0;
                if (phase - 0.0).abs() < dt {
                    // look -> walk
                    if let animator::AnimatorState::State(_state) =
                        animated_model.animator.get_current_state()
                    {
                        match animated_model.animator.transition(0) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                } else if (phase - a).abs() < dt {
                    // walk -> run
                    if let animator::AnimatorState::State(_state) =
                        animated_model.animator.get_current_state()
                    {
                        match animated_model.animator.transition(2) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                } else if (phase - (2.0 * a)).abs() < dt {
                    // run -> walk
                    if let animator::AnimatorState::State(_state) =
                        animated_model.animator.get_current_state()
                    {
                        match animated_model.animator.transition(3) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                } else if (phase - (3.0 * a)).abs() < dt {
                    // walk -> look
                    if let animator::AnimatorState::State(_state) =
                        animated_model.animator.get_current_state()
                    {
                        match animated_model.animator.transition(1) {
                            Ok(_) => (),
                            Err(_) => println!("Incorrect transition"),
                        }
                    }
                }

                animated_model.animator.update(animation_graphs, dt);
            }
        }
    }

    fn consume_input(&mut self, scene: &mut Scene, event: InputEvent<Self::UiCommand>) {
        match event {
            InputEvent::Exit => return (),
            InputEvent::AspectChange(aspect) => scene.camera.aspect = aspect,
            InputEvent::Ui(command) => match command {
                UiCommand::SetCameraDistance(distance) => {
                    scene.camera.eye.z = distance.max(0.0);
                }
            },
            InputEvent::DeviceEvent(event) => match event {
                DeviceEvent::MouseMotion { delta: (x, y) } => {
                    if !self.mouse_btn_is_pressed {
                        return;
                    }
                    let camera = &mut scene.camera;
                    let sensitivity = 5f32;
                    camera.rot_x = camera.rot_x - (x as f32 / sensitivity);
                    camera.rot_y = camera.rot_y - (y as f32 / sensitivity);
                }
                _ => (),
            },
            InputEvent::WindowEvent(event) => match event {
                WindowEvent::MouseWheel { delta, .. } => {
                    let camera = &mut scene.camera;
                    match delta {
                        MouseScrollDelta::LineDelta(_x, y) => {
                            let scroll_speed = 10f32;
                            camera.eye.z = (camera.eye.z
                                + ((if self.shift_is_pressed {
                                    10f32 * scroll_speed
                                } else {
                                    scroll_speed
                                }) * -y as f32))
                                .max(0f32);
                        }
                        MouseScrollDelta::PixelDelta(_pos) => (),
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    match button {
                        winit::event::MouseButton::Left => match state {
                            ElementState::Pressed => {
                                self.mouse_btn_is_pressed = true;
                            }
                            ElementState::Released => {
                                self.mouse_btn_is_pressed = false;
                            }
                        },
                        _ => (),
                    };
                }
                WindowEvent::KeyboardInput { event, .. } => match event {
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::ShiftLeft),
                        state: ElementState::Pressed,
                        ..
                    } => {
                        self.shift_is_pressed = true;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::ShiftLeft),
                        state: ElementState::Released,
                        ..
                    } => {
                        self.shift_is_pressed = false;
                    }
                    _ => (),
                },
                _ => (),
            },
        }
    }

    fn build_ui_snapshot(&mut self, scene: &Scene, _tick: u64) -> Self::UiSnapshot {
        UiSnapshot {
            global_time_sec: scene.global_time_sec,
            camera_distance: scene.camera.eye.z,
            shift_is_pressed: self.shift_is_pressed,
        }
    }
}

impl UiTrait for Game {
    type UiSnapshot = UiSnapshot;
    type UiCommand = UiCommand;

    fn build_ui(
        ctx: &egui::Context,
        frame_idx: u32,
        snapshot: Option<&Self::UiSnapshot>,
        emit: &mut dyn FnMut(Self::UiCommand),
    ) {
        egui::Window::new("Renderer UI").show(ctx, |ui| {
            ui.label(format!("frame index: {}", frame_idx));
            if let Some(snapshot) = snapshot {
                ui.label(format!("sim t: {:.2}", snapshot.global_time_sec));
                ui.label(format!("shift pressed: {}", snapshot.shift_is_pressed));
                let mut camera_distance = snapshot.camera_distance;
                if ui
                    .add(egui::Slider::new(&mut camera_distance, 0.0..=300.0).text("Camera Z"))
                    .changed()
                {
                    emit(UiCommand::SetCameraDistance(camera_distance));
                }
            } else {
                ui.label("waiting for sim ui snapshot...");
            }
        });
    }
}

fn main() -> io::Result<()> {
    run(Game::new());
    Ok(())
}

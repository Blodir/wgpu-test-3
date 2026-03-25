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
    game_trait::{DebugInfo, InputEvent, SimTrait, UiTrait},
    run,
};
use generational_arena::Arena;
use glam::{Mat3, Mat4, Quat, Vec3};
use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

struct Game {
    shift_is_pressed: bool,
    alt_is_pressed: bool,
    mouse_btn_is_pressed: bool,
    move_forward_pressed: bool,
    move_back_pressed: bool,
    move_left_pressed: bool,
    move_right_pressed: bool,
    orbit_target: Vec3,
    orbit_up: Vec3,
    orbit_distance: f32,
    orbit_yaw_deg: f32,
    orbit_pitch_deg: f32,
}

struct VarSnapshot;

enum UiCommand {
    SetCameraDistance(f32),
}

impl Game {
    fn new() -> Self {
        Self {
            shift_is_pressed: false,
            alt_is_pressed: false,
            mouse_btn_is_pressed: false,
            move_forward_pressed: false,
            move_back_pressed: false,
            move_left_pressed: false,
            move_right_pressed: false,
            orbit_target: Vec3::ZERO,
            orbit_up: Vec3::Y,
            orbit_distance: 100.0,
            orbit_yaw_deg: 0.0,
            orbit_pitch_deg: 0.0,
        }
    }

    fn look_at_rotation(eye: Vec3, target: Vec3, world_up: Vec3) -> Quat {
        let forward = (target - eye).normalize();
        let up = (world_up - forward * world_up.dot(forward)).normalize();
        let right = forward.cross(up);

        // Camera looks down -Z
        Quat::from_mat3(&Mat3::from_cols(right, up, -forward))
    }

    fn apply_orbit_camera(&self, scene: &mut Scene) {
        let orbit_rot = Quat::from_rotation_y(self.orbit_yaw_deg.to_radians())
            * Quat::from_rotation_x(self.orbit_pitch_deg.to_radians());
        let position = self.orbit_target + (orbit_rot * Vec3::new(0.0, 0.0, self.orbit_distance));
        let rotation = Self::look_at_rotation(position, self.orbit_target, self.orbit_up);
        scene.camera.position = position;
        scene.camera.rotation = rotation;
    }

    fn pan_orbit_target(&mut self, scene: &mut Scene, dx: f32, dy: f32) {
        let forward = (self.orbit_target - scene.camera.position).normalize_or_zero();
        if forward.length_squared() == 0.0 {
            return;
        }
        let right = forward.cross(self.orbit_up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();

        let pan_speed = (self.orbit_distance * 0.0025).max(0.001);
        self.orbit_target += (-dx * pan_speed) * right + (dy * pan_speed) * up;
        self.apply_orbit_camera(scene);
    }

    fn move_orbit_target_wasd(&mut self, scene: &mut Scene, dt: f32) {
        if !self.mouse_btn_is_pressed {
            return;
        }

        let mut forward_axis = 0.0f32;
        if self.move_forward_pressed {
            forward_axis += 1.0;
        }
        if self.move_back_pressed {
            forward_axis -= 1.0;
        }

        let mut strafe_axis = 0.0f32;
        if self.move_right_pressed {
            strafe_axis += 1.0;
        }
        if self.move_left_pressed {
            strafe_axis -= 1.0;
        }

        if forward_axis == 0.0 && strafe_axis == 0.0 {
            return;
        }

        let forward = (scene.camera.rotation * Vec3::NEG_Z).normalize_or_zero();
        let right = (scene.camera.rotation * Vec3::X).normalize_or_zero();
        if forward.length_squared() == 0.0 || right.length_squared() == 0.0 {
            return;
        }
        let move_dir = (forward * forward_axis + right * strafe_axis).normalize_or_zero();
        if move_dir.length_squared() == 0.0 {
            return;
        }

        let move_speed = (self.orbit_distance * 1.5).max(2.0);
        self.orbit_target += move_dir * move_speed * dt;
        self.apply_orbit_camera(scene);
    }
}
impl SimTrait for Game {
    type VarSnapshot = VarSnapshot;
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

        let mut scene = Scene {
            root: SceneNodeId(root_handle),
            nodes,
            environment,
            camera: Camera::default(),
            global_time_sec: 0.0,
        };
        self.apply_orbit_camera(&mut scene);

        (scene, animation_graphs)
    }

    fn fixed_update(
        &mut self,
        scene: &mut engine::game::scene_tree::Scene,
        resource_registry: &std::rc::Rc<
            std::cell::RefCell<engine::game::assets::registry::ResourceRegistry>,
        >,
        animation_graphs: &Vec<engine::game::animator::AnimationGraph>,
        node: engine::game::scene_tree::SceneNodeId,
        dt: f32,
    ) {
        let children = scene
            .nodes
            .get(node.into())
            .map(|n| n.children.clone())
            .unwrap_or_default();
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

        for child in children {
            self.fixed_update(scene, resource_registry, animation_graphs, child, dt);
        }
    }

    fn variable_update(&mut self, scene: &mut Scene, dt: f32) {
        self.move_orbit_target_wasd(scene, dt);
    }

    fn consume_input(&mut self, scene: &mut Scene, event: InputEvent<Self::UiCommand>) {
        match event {
            InputEvent::Exit => return (),
            InputEvent::AspectChange(aspect) => scene.camera.aspect = aspect,
            InputEvent::Ui(command) => match command {
                UiCommand::SetCameraDistance(distance) => {
                    self.orbit_distance = distance.max(0.0);
                    self.apply_orbit_camera(scene);
                }
            },
            InputEvent::DeviceEvent(event) => match event {
                DeviceEvent::MouseMotion { delta: (x, y) } => {
                    if !self.mouse_btn_is_pressed {
                        return;
                    }
                    if self.shift_is_pressed {
                        self.pan_orbit_target(scene, x as f32, y as f32);
                    } else {
                        let sensitivity = 5f32;
                        self.orbit_yaw_deg -= x as f32 / sensitivity;
                        self.orbit_pitch_deg -= y as f32 / sensitivity;
                        self.apply_orbit_camera(scene);
                    }
                }
                _ => (),
            },
            InputEvent::WindowEvent(event) => match event {
                WindowEvent::MouseWheel { delta, .. } => match delta {
                    MouseScrollDelta::LineDelta(_x, y) => {
                        let scroll_speed = 10f32;
                        self.orbit_distance = (self.orbit_distance
                            + ((if self.alt_is_pressed {
                                10f32 * scroll_speed
                            } else {
                                scroll_speed
                            }) * -y as f32))
                            .max(0.0);
                        self.apply_orbit_camera(scene);
                    }
                    MouseScrollDelta::PixelDelta(_pos) => (),
                },
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
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::AltLeft),
                        state: ElementState::Pressed,
                        ..
                    }
                    | KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::AltRight),
                        state: ElementState::Pressed,
                        ..
                    } => {
                        self.alt_is_pressed = true;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::AltLeft),
                        state: ElementState::Released,
                        ..
                    }
                    | KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::AltRight),
                        state: ElementState::Released,
                        ..
                    } => {
                        self.alt_is_pressed = false;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyW),
                        state: ElementState::Pressed,
                        ..
                    } => {
                        self.move_forward_pressed = true;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyW),
                        state: ElementState::Released,
                        ..
                    } => {
                        self.move_forward_pressed = false;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyS),
                        state: ElementState::Pressed,
                        ..
                    } => {
                        self.move_back_pressed = true;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyS),
                        state: ElementState::Released,
                        ..
                    } => {
                        self.move_back_pressed = false;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyA),
                        state: ElementState::Pressed,
                        ..
                    } => {
                        self.move_left_pressed = true;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyA),
                        state: ElementState::Released,
                        ..
                    } => {
                        self.move_left_pressed = false;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyD),
                        state: ElementState::Pressed,
                        ..
                    } => {
                        self.move_right_pressed = true;
                    }
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyD),
                        state: ElementState::Released,
                        ..
                    } => {
                        self.move_right_pressed = false;
                    }
                    _ => (),
                },
                _ => (),
            },
        }
    }

    fn build_var_snapshot(&mut self, _scene: &Scene, _tick: u64) -> Self::VarSnapshot {
        VarSnapshot
    }
}

impl UiTrait for Game {
    type VarSnapshot = VarSnapshot;
    type UiCommand = UiCommand;

    fn build_ui(
        ctx: &egui::Context,
        _snapshot: Option<&Self::VarSnapshot>,
        debug_info: &DebugInfo,
        _emit: &mut dyn FnMut(Self::UiCommand),
    ) {
        let line1 = format!("render fps: {:.1}", debug_info.render.fps);
        let line2 = format!("render frame: {:.2} ms", debug_info.render.frame_time_ms);
        let line3 = format!("sim fps: {:.1}", debug_info.sim.fps);
        let line4 = format!("sim frame: {:.2} ms", debug_info.sim.frame_time_ms);
        let text_color = egui::Color32::WHITE;
        let font = egui::FontId::proportional(22.0);
        let padding = egui::vec2(10.0, 8.0);
        let line_gap = 4.0;
        let origin = egui::pos2(12.0, 12.0);
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("renderer_stats_overlay"),
        ));

        let galley1 = painter.layout_no_wrap(line1, font.clone(), text_color);
        let galley2 = painter.layout_no_wrap(line2, font.clone(), text_color);
        let galley3 = painter.layout_no_wrap(line3, font.clone(), text_color);
        let galley4 = painter.layout_no_wrap(line4, font, text_color);
        let line1_h = galley1.size().y;
        let line2_h = galley2.size().y;
        let line3_h = galley3.size().y;
        let line4_h = galley4.size().y;
        let width = galley1
            .size()
            .x
            .max(galley2.size().x)
            .max(galley3.size().x)
            .max(galley4.size().x)
            + (padding.x * 2.0);
        let height = line1_h + line2_h + line3_h + line4_h + (line_gap * 3.0) + (padding.y * 2.0);
        let rect = egui::Rect::from_min_size(origin, egui::vec2(width, height));

        painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(160));
        painter.galley(rect.min + padding, galley1, text_color);
        painter.galley(
            rect.min + padding + egui::vec2(0.0, line1_h + line_gap),
            galley2,
            text_color,
        );
        painter.galley(
            rect.min + padding + egui::vec2(0.0, line1_h + line2_h + (line_gap * 2.0)),
            galley3,
            text_color,
        );
        painter.galley(
            rect.min + padding + egui::vec2(0.0, line1_h + line2_h + line3_h + (line_gap * 3.0)),
            galley4,
            text_color,
        );
    }
}

fn main() -> io::Result<()> {
    run(Game::new);
    Ok(())
}

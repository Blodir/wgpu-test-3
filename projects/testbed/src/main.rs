use std::{cell::RefCell, io};

use engine::{
    game::{
        animator::{self, AnimationGraph, Animator},
        assets::registry::{ModelHandle, RegistryExt as _, TextureHandle},
        camera::Camera,
        scene_tree::{
            AnimatedModel, Environment, Node, PointLight, RenderDataType, Scene, SceneNodeId,
            StaticModel, Sun,
        },
    },
    game_trait::{InputEvent, SimTrait, UiTrait},
    main::renderer::DebugInfo,
    run,
};
use generational_arena::Arena;
use glam::{Mat3, Mat4, Quat, Vec3};
use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

const ENV_MAP_CHOICES: [(&str, &str, &str); 3] = [
    (
        "Kloofendal Overcast",
        "assets/kloofendal_overcast_puresky_8k.prefiltered.dds",
        "assets/kloofendal_overcast_puresky_8k.di.dds",
    ),
    (
        "Steinbach Field",
        "assets/steinbach_field_4k.prefiltered.dds",
        "assets/steinbach_field_4k.di.dds",
    ),
    (
        "indoor_pool_4k",
        "assets/indoor_pool_4k.prefiltered.dds",
        "assets/indoor_pool_4k.di.dds",
    ),
];

const MESH_CHOICES: [(&str, &str, f32, bool); 4] = [
    ("Fox", "assets/local/Fox/Fox.json", 1.0, true),
    ("Sponza", "assets/local/Sponza/Sponza.json", 10.0, false),
    ("Lantern", "assets/local/Lantern/Lantern.json", 10.0, false),
    (
        "MetalRoughSpheres",
        "assets/local/MetalRoughSpheres/MetalRoughSpheres.json",
        10.0,
        false,
    ),
];

const DEFAULT_MESH_IDX: usize = 1;
const DEFAULT_GRID_SIZE: u32 = 1;
const DEFAULT_GRID_SPACING: f32 = 200.0;

struct EnvironmentMapOption {
    prefiltered: TextureHandle,
    di: TextureHandle,
}

struct MeshOption {
    model: ModelHandle,
    scale: f32,
    animated: bool,
}

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
    sun_tint: [f32; 3],
    sun_intensity: f32,
    environment_map_intensity: f32,
    sun_altitude_deg: f32,
    sun_direction_deg: f32,
    environment_map_options: Vec<EnvironmentMapOption>,
    selected_environment_map_idx: usize,
    mesh_options: Vec<MeshOption>,
    selected_mesh_idx: usize,
    grid_size: u32,
    grid_spacing: f32,
}

struct VarSnapshot {
    sun_tint: [f32; 3],
    sun_intensity: f32,
    environment_map_intensity: f32,
    sun_altitude_deg: f32,
    sun_direction_deg: f32,
    selected_environment_map_idx: usize,
    selected_mesh_idx: usize,
    grid_size: u32,
    grid_spacing: f32,
}

enum UiCommand {
    SetSunSettings {
        tint: [f32; 3],
        intensity: f32,
        environment_map_intensity: f32,
        altitude_deg: f32,
        direction_deg: f32,
        environment_map_idx: usize,
    },
    SetSceneSettings {
        mesh_idx: usize,
        grid_size: u32,
        grid_spacing: f32,
    },
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
            sun_tint: [1.0, 1.0, 1.0],
            sun_intensity: 10.0,
            environment_map_intensity: 1.0,
            sun_altitude_deg: -35.0,
            sun_direction_deg: 45.0,
            environment_map_options: vec![],
            selected_environment_map_idx: 0,
            mesh_options: vec![],
            selected_mesh_idx: DEFAULT_MESH_IDX,
            grid_size: DEFAULT_GRID_SIZE,
            grid_spacing: DEFAULT_GRID_SPACING,
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

    fn split_tint_intensity(color: [f32; 3]) -> ([f32; 3], f32) {
        let intensity = color[0].max(color[1]).max(color[2]);
        if intensity > 0.0 {
            (
                [
                    color[0] / intensity,
                    color[1] / intensity,
                    color[2] / intensity,
                ],
                intensity,
            )
        } else {
            ([1.0, 1.0, 1.0], 0.0)
        }
    }

    fn direction_to_angles(direction: [f32; 3]) -> (f32, f32) {
        let dir = Vec3::from(direction).normalize_or_zero();
        if dir.length_squared() == 0.0 {
            return (0.0, 0.0);
        }
        // Direction: start at +X and rotate around +Y toward +Z.
        let direction_deg = dir.z.atan2(dir.x).to_degrees().rem_euclid(360.0);
        // Altitude: start at ground-plane +X and rotate around +Z.
        // Undo direction first, then read angle in the XY plane.
        let dir_rad = direction_deg.to_radians();
        let x_after_direction = (dir.x * dir_rad.cos()) + (dir.z * dir_rad.sin());
        let altitude_deg = dir
            .y
            .atan2(x_after_direction)
            .to_degrees()
            .rem_euclid(360.0);
        (altitude_deg, direction_deg)
    }

    fn angles_to_direction(altitude_deg: f32, direction_deg: f32) -> [f32; 3] {
        let alt = altitude_deg.to_radians();
        let az = direction_deg.to_radians();
        let horizontal = alt.cos();
        // Equivalent to: Ry(direction) * Rz(altitude) * +X
        Vec3::new(horizontal * az.cos(), alt.sin(), horizontal * az.sin()).to_array()
    }

    fn apply_sun_settings(&self, scene: &mut Scene) {
        scene.environment.sun.color = [
            self.sun_tint[0] * self.sun_intensity,
            self.sun_tint[1] * self.sun_intensity,
            self.sun_tint[2] * self.sun_intensity,
        ];
        scene.environment.environment_map_intensity = self.environment_map_intensity;
        scene.environment.sun.direction =
            Self::angles_to_direction(self.sun_altitude_deg, self.sun_direction_deg);
    }

    fn request_environment_map_options(
        resource_registry: &std::rc::Rc<
            std::cell::RefCell<engine::game::assets::registry::ResourceRegistry>,
        >,
    ) -> Vec<EnvironmentMapOption> {
        ENV_MAP_CHOICES
            .iter()
            .map(|(_label, prefiltered, di)| EnvironmentMapOption {
                prefiltered: resource_registry.request_texture(prefiltered, true),
                di: resource_registry.request_texture(di, true),
            })
            .collect()
    }

    fn apply_environment_map_selection(&self, scene: &mut Scene) {
        if let Some(selected) = self
            .environment_map_options
            .get(self.selected_environment_map_idx)
        {
            scene.environment.prefiltered = selected.prefiltered.clone();
            scene.environment.di = selected.di.clone();
        }
    }

    fn request_mesh_options(
        resource_registry: &std::rc::Rc<
            std::cell::RefCell<engine::game::assets::registry::ResourceRegistry>,
        >,
    ) -> Vec<MeshOption> {
        MESH_CHOICES
            .iter()
            .map(|(_label, path, scale, animated)| MeshOption {
                model: resource_registry.request_model(path),
                scale: *scale,
                animated: *animated,
            })
            .collect()
    }

    fn build_animation_graphs() -> Vec<AnimationGraph> {
        vec![AnimationGraph {
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
        }]
    }

    fn rebuild_scene_nodes(&self, scene: &mut Scene) {
        let Some(mesh) = self.mesh_options.get(self.selected_mesh_idx) else {
            return;
        };
        let mut nodes = Arena::new();
        let mut children = Vec::with_capacity((self.grid_size * self.grid_size) as usize);
        let half = (self.grid_size as f32 - 1.0) * 0.5;
        for i in 0..self.grid_size {
            for j in 0..self.grid_size {
                let x = (i as f32 - half) * self.grid_spacing;
                let z = (j as f32 - half) * self.grid_spacing;
                let transform = Mat4::from_scale_rotation_translation(
                    Vec3::splat(mesh.scale),
                    Quat::IDENTITY,
                    Vec3::new(x, 0.0, z),
                );
                let render_data = if mesh.animated {
                    RenderDataType::AnimatedModel(AnimatedModel {
                        model: mesh.model.clone(),
                        animator: Animator::new(0, 0),
                        last_visible_frame: RefCell::new(u32::MAX),
                    })
                } else {
                    RenderDataType::Model(StaticModel {
                        handle: mesh.model.clone(),
                        last_visible_frame: RefCell::new(u32::MAX),
                    })
                };

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

        // TEMP: point light for validating point-light rendering in testbed.
        let test_light = nodes.insert(Node {
            parent: None,
            children: vec![],
            transform: Mat4::from_translation(Vec3::new(30.0, 40.0, 30.0)),
            transform_last_mut: 0,
            render_data: RenderDataType::PointLight(PointLight {
                color: [1.0, 0.92, 0.75],
                intensity: 30_000.0,
                range: 220.0,
            }),
        });
        children.push(SceneNodeId(test_light));

        let root_handle = nodes.insert(Node {
            parent: None,
            children: children.clone(),
            transform: Mat4::IDENTITY,
            transform_last_mut: 0,
            render_data: RenderDataType::None,
        });
        let root = SceneNodeId(root_handle);
        for child in &children {
            if let Some(node) = nodes.get_mut(child.0) {
                node.parent = Some(root);
            }
        }

        scene.nodes = nodes;
        scene.root = root;
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
        let animation_graphs = Self::build_animation_graphs();

        self.mesh_options = Self::request_mesh_options(resource_registry);
        self.selected_mesh_idx = DEFAULT_MESH_IDX.min(self.mesh_options.len().saturating_sub(1));
        self.grid_size = DEFAULT_GRID_SIZE;
        self.grid_spacing = DEFAULT_GRID_SPACING;

        self.environment_map_options = Self::request_environment_map_options(resource_registry);
        self.selected_environment_map_idx = 0;

        let environment = Environment {
            sun: Sun::default(),
            environment_map_intensity: 1.0,
            prefiltered: self.environment_map_options[self.selected_environment_map_idx]
                .prefiltered
                .clone(),
            di: self.environment_map_options[self.selected_environment_map_idx]
                .di
                .clone(),
            brdf: resource_registry.request_texture("assets/brdf_lut.png", false),
        };

        let mut nodes = Arena::new();
        let root_handle = nodes.insert(Node {
            parent: None,
            children: vec![],
            transform: Mat4::IDENTITY,
            transform_last_mut: 0,
            render_data: RenderDataType::None,
        });
        let mut scene = Scene {
            root: SceneNodeId(root_handle),
            nodes,
            environment,
            camera: Camera::default(),
            global_time_sec: 0.0,
        };
        self.rebuild_scene_nodes(&mut scene);

        let (sun_tint, sun_intensity) = Self::split_tint_intensity(scene.environment.sun.color);
        let (sun_altitude_deg, sun_direction_deg) =
            Self::direction_to_angles(scene.environment.sun.direction);
        self.sun_tint = sun_tint;
        self.sun_intensity = sun_intensity;
        self.environment_map_intensity = scene.environment.environment_map_intensity;
        self.sun_altitude_deg = sun_altitude_deg;
        self.sun_direction_deg = sun_direction_deg;
        self.apply_sun_settings(&mut scene);
        self.apply_environment_map_selection(&mut scene);
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
            RenderDataType::PointLight(_point_light) => (),
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
                UiCommand::SetSunSettings {
                    tint,
                    intensity,
                    environment_map_intensity,
                    altitude_deg,
                    direction_deg,
                    environment_map_idx,
                } => {
                    let intensity = intensity.max(0.0);
                    self.sun_tint = tint;
                    self.sun_intensity = intensity;
                    self.environment_map_intensity = environment_map_intensity.max(0.0);
                    self.sun_altitude_deg = altitude_deg.rem_euclid(360.0);
                    self.sun_direction_deg = direction_deg.rem_euclid(360.0);
                    self.selected_environment_map_idx = environment_map_idx
                        .min(self.environment_map_options.len().saturating_sub(1));
                    self.apply_sun_settings(scene);
                    self.apply_environment_map_selection(scene);
                }
                UiCommand::SetSceneSettings {
                    mesh_idx,
                    grid_size,
                    grid_spacing,
                } => {
                    let clamped_mesh_idx = mesh_idx.min(self.mesh_options.len().saturating_sub(1));
                    let clamped_grid_size = grid_size.clamp(1, 100);
                    let clamped_grid_spacing = grid_spacing.max(1.0);
                    let scene_changed = clamped_mesh_idx != self.selected_mesh_idx
                        || clamped_grid_size != self.grid_size
                        || (clamped_grid_spacing - self.grid_spacing).abs() > f32::EPSILON;

                    self.selected_mesh_idx = clamped_mesh_idx;
                    self.grid_size = clamped_grid_size;
                    self.grid_spacing = clamped_grid_spacing;

                    if scene_changed {
                        self.rebuild_scene_nodes(scene);
                        self.apply_orbit_camera(scene);
                    }
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
        VarSnapshot {
            sun_tint: self.sun_tint,
            sun_intensity: self.sun_intensity,
            environment_map_intensity: self.environment_map_intensity,
            sun_altitude_deg: self.sun_altitude_deg,
            sun_direction_deg: self.sun_direction_deg,
            selected_environment_map_idx: self.selected_environment_map_idx,
            selected_mesh_idx: self.selected_mesh_idx,
            grid_size: self.grid_size,
            grid_spacing: self.grid_spacing,
        }
    }
}

impl UiTrait for Game {
    type VarSnapshot = VarSnapshot;
    type UiCommand = UiCommand;

    fn build_ui(
        ctx: &egui::Context,
        snapshot: Option<&Self::VarSnapshot>,
        debug_info: &DebugInfo,
        emit: &mut dyn FnMut(Self::UiCommand),
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

        if let Some(snapshot) = snapshot {
            let mut tint = snapshot.sun_tint;
            let mut intensity = snapshot.sun_intensity;
            let mut environment_map_intensity = snapshot.environment_map_intensity;
            let mut altitude_deg = snapshot.sun_altitude_deg;
            let mut direction_deg = snapshot.sun_direction_deg;
            let mut environment_map_idx = snapshot.selected_environment_map_idx;
            let mut sun_changed = false;

            egui::Window::new("Sun")
                .default_pos(egui::pos2(12.0, rect.max.y + 10.0))
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Color");
                    sun_changed |= ui.color_edit_button_rgb(&mut tint).changed();
                    let selected_label = ENV_MAP_CHOICES
                        .get(environment_map_idx)
                        .map(|v| v.0)
                        .unwrap_or("Unknown");
                    egui::ComboBox::from_label("Environment Map")
                        .selected_text(selected_label)
                        .show_ui(ui, |ui| {
                            for (idx, (label, _, _)) in ENV_MAP_CHOICES.iter().enumerate() {
                                sun_changed |= ui
                                    .selectable_value(&mut environment_map_idx, idx, *label)
                                    .changed();
                            }
                        });
                    sun_changed |= ui
                        .add(
                            egui::Slider::new(&mut intensity, 0.0..=100.0)
                                .text("Intensity")
                                .clamping(egui::SliderClamping::Always),
                        )
                        .changed();
                    sun_changed |= ui
                        .add(
                            egui::Slider::new(&mut environment_map_intensity, 0.0..=100.0)
                                .text("Environment Intensity")
                                .clamping(egui::SliderClamping::Always),
                        )
                        .changed();
                    sun_changed |= ui
                        .add(
                            egui::Slider::new(&mut altitude_deg, 0.0..=360.0)
                                .text("Altitude (deg)")
                                .clamping(egui::SliderClamping::Always),
                        )
                        .changed();
                    sun_changed |= ui
                        .add(
                            egui::Slider::new(&mut direction_deg, 0.0..=360.0)
                                .text("Direction (deg)")
                                .clamping(egui::SliderClamping::Always),
                        )
                        .changed();
                });

            if sun_changed {
                emit(UiCommand::SetSunSettings {
                    tint,
                    intensity,
                    environment_map_intensity,
                    altitude_deg,
                    direction_deg,
                    environment_map_idx,
                });
            }

            let mut mesh_idx = snapshot.selected_mesh_idx;
            let mut grid_size = snapshot.grid_size;
            let mut grid_spacing = snapshot.grid_spacing;
            let mut scene_changed = false;

            egui::Window::new("Scene")
                .default_pos(egui::pos2(260.0, rect.max.y + 10.0))
                .resizable(false)
                .show(ctx, |ui| {
                    let selected_label =
                        MESH_CHOICES.get(mesh_idx).map(|v| v.0).unwrap_or("Unknown");
                    egui::ComboBox::from_label("Mesh")
                        .selected_text(selected_label)
                        .show_ui(ui, |ui| {
                            for (idx, (label, _, _, _)) in MESH_CHOICES.iter().enumerate() {
                                scene_changed |=
                                    ui.selectable_value(&mut mesh_idx, idx, *label).changed();
                            }
                        });
                    scene_changed |= ui
                        .add(
                            egui::Slider::new(&mut grid_size, 1..=100)
                                .text("Grid Size")
                                .clamping(egui::SliderClamping::Always),
                        )
                        .changed();
                    scene_changed |= ui
                        .add(
                            egui::Slider::new(&mut grid_spacing, 1.0..=500.0)
                                .text("Spacing")
                                .clamping(egui::SliderClamping::Always),
                        )
                        .changed();
                });

            if scene_changed {
                emit(UiCommand::SetSceneSettings {
                    mesh_idx,
                    grid_size,
                    grid_spacing,
                });
            }
        }
    }
}

fn main() -> io::Result<()> {
    run(Game::new);
    Ok(())
}

use std::{
    cell::RefCell, rc::Rc, sync::Arc, thread, time::{Duration, Instant}
};

use crossbeam_queue::{ArrayQueue, SegQueue};
use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    job_system::worker_pool::Task, render_snapshot::{RenderSnapshot, SnapshotHandoff}, resource_system::{game_resources::{CreateGameResourceRequest, CreateGameResourceResponse, GameResources}, registry::{RegistryExt, ResourceRegistry, ResourceRequest, ResourceResult}, resource_manager::ResourceManager}
};

use super::scene_tree::{build_test_scene, RenderDataType};

#[derive(Debug)]
pub enum InputEvent {
    DeviceEvent(winit::event::DeviceEvent),
    WindowEvent(winit::event::WindowEvent),
    AspectChange(f32),
    Exit,
}

const TICK: Duration = Duration::from_millis(100);
const SPIN: Duration = Duration::from_micros(200);

pub fn spawn_sim(
    inputs: Arc<SegQueue<InputEvent>>,
    snap_handoff: Arc<SnapshotHandoff>,
    reg_req_tx: crossbeam::channel::Sender<ResourceRequest>,
    reg_res_rx: crossbeam::channel::Receiver<ResourceResult>,
    game_req_rx: crossbeam::channel::Receiver<CreateGameResourceRequest>,
    game_res_tx: crossbeam::channel::Sender<CreateGameResourceResponse>,
    job_task_tx: crossbeam::channel::Sender<Task>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let resource_registry = Rc::new(RefCell::new(ResourceRegistry::new(reg_req_tx, reg_res_rx)));
        let mut game_resources = GameResources::new(game_req_rx, game_res_tx, &resource_registry);
        let (mut scene, animation_graphs) = build_test_scene(&resource_registry);
        let mut next = Instant::now() + TICK;
        let mut prev_tick = Instant::now();
        let mut shift_is_pressed = false;
        let mut mouse_btn_is_pressed = false;
        let mut frame_index = 0u32;
        let sim_start_time = Instant::now();
        loop {
            let now = Instant::now();
            let dt = (now - prev_tick).as_secs_f32();
            prev_tick = now;

            scene.global_time_sec = (now - sim_start_time).as_secs_f32();

            game_resources.process_requests(&resource_registry);
            resource_registry.process_responses();

            'a: while let Some(event) = inputs.pop() {
                match event {
                    InputEvent::Exit => return (),
                    InputEvent::AspectChange(aspect) => scene.camera.aspect = aspect,
                    InputEvent::DeviceEvent(event) => match event {
                        DeviceEvent::MouseMotion { delta: (x, y) } => {
                            if !mouse_btn_is_pressed {
                                continue 'a;
                            }
                            let camera = &mut scene.camera;
                            let sensitivity = 5f32;
                            camera.rot_x = camera.rot_x - (x as f32 / sensitivity);
                            camera.rot_y = camera.rot_y - (y as f32 / sensitivity);
                        }
                        _ => (),
                    },
                    InputEvent::WindowEvent(event) => match event {
                        WindowEvent::MouseWheel {
                            device_id,
                            delta,
                            phase,
                        } => {
                            let camera = &mut scene.camera;
                            match delta {
                                MouseScrollDelta::LineDelta(x, y) => {
                                    let scroll_speed = 10f32;
                                    camera.eye.z = (camera.eye.z
                                        + ((if shift_is_pressed { 10f32 * scroll_speed } else { scroll_speed })
                                            * -y as f32))
                                        .max(0f32);
                                }
                                MouseScrollDelta::PixelDelta(pos) => (),
                            }
                        }
                        WindowEvent::MouseInput {
                            device_id,
                            state,
                            button,
                        } => {
                            match button {
                                winit::event::MouseButton::Left => match state {
                                    ElementState::Pressed => {
                                        mouse_btn_is_pressed = true;
                                    }
                                    ElementState::Released => {
                                        mouse_btn_is_pressed = false;
                                    }
                                },
                                _ => (),
                            };
                        }
                        WindowEvent::KeyboardInput {
                            device_id,
                            event,
                            is_synthetic,
                        } => match event {
                            KeyEvent {
                                physical_key: PhysicalKey::Code(KeyCode::ShiftLeft),
                                state: ElementState::Pressed,
                                ..
                            } => {
                                shift_is_pressed = true;
                            }
                            KeyEvent {
                                physical_key: PhysicalKey::Code(KeyCode::ShiftLeft),
                                state: ElementState::Released,
                                ..
                            } => {
                                shift_is_pressed = false;
                            }
                            _ => (),
                        },
                        _ => (),
                    },
                }
            }

            scene.update(&resource_registry, &animation_graphs, scene.root, dt);

            let snap = RenderSnapshot::build(&mut scene, &resource_registry, &animation_graphs, &game_resources, frame_index);

            // schedule animation jobs
            for (node_id, _) in &snap.mesh_draw_snapshot.skinned_instances {
                match &mut scene.nodes.get_mut((*node_id).into()).unwrap().render_data {
                    RenderDataType::Model(static_model) => (),
                    RenderDataType::AnimatedModel(animated_model) => {
                        let job = animated_model.animator.build_job(dt, &animation_graphs, *node_id, &animated_model.model, &game_resources, &resource_registry);
                        if job_task_tx.send(Task::Pose(*node_id, job)).is_err() {
                            todo!();
                        }
                    },
                    RenderDataType::None => (),
                }
            }

            snap_handoff.publish(snap);

            next += TICK;

            // sleep most of the remaining time, then spin the last bit
            if let Some(remain) = next.checked_duration_since(Instant::now()) {
                if remain > SPIN { thread::sleep(remain - SPIN); }
                while Instant::now() < next { std::hint::spin_loop(); }
            } else {
                // if we fell behind, resync the schedule
                next = Instant::now() + TICK;
            }

            frame_index = frame_index.wrapping_add(1);
        }
    })
}

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crossbeam_queue::{ArrayQueue, SegQueue};
use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    renderer::render_snapshot::{RenderSnapshot, SnapshotHandoff},
    scene_tree::Scene,
};

#[derive(Debug)]
pub enum InputEvent {
    DeviceEvent(winit::event::DeviceEvent),
    WindowEvent(winit::event::WindowEvent),
    Exit,
}

const TICK: Duration = Duration::from_nanos(50_000_000); // 20hz ish
const SPIN: Duration = Duration::from_micros(200);

pub fn spawn_sim(
    inputs: Arc<SegQueue<InputEvent>>,
    snap_handoff: Arc<SnapshotHandoff>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut scene = Scene::default();
        let mut next = Instant::now() + TICK;
        let mut shift_is_pressed = false;
        let mut mouse_btn_is_pressed = false;
        loop {
            'a: while let Some(event) = inputs.pop() {
                match event {
                    InputEvent::Exit => return (),
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
                                    camera.eye.z = (camera.eye.z
                                        + ((if shift_is_pressed { 10f32 } else { 1f32 })
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
            let snap = RenderSnapshot::build(&scene);
            snap_handoff.publish(snap);

            next += TICK;

            // sleep most of the remaining time, then spin the last bit
            let now = Instant::now();
            if next > now {
                let remain = next - now;
                if remain > SPIN {
                    thread::sleep(remain - SPIN);
                }
                while Instant::now() < next {
                    std::hint::spin_loop();
                }
            }
        }
    })
}

use std::{cell::RefCell, rc::Rc, sync::Arc};

use crate::{job_system::worker_pool::{AnimPoseTask, BlendPoseTask, SinglePoseTask, Task}, render_snapshot::AnimationSnapshot, renderer::pose_storage::POSE_STORAGE_BUFFER_SIZE, resource_system::{file_formats::skeletonfile::Skeleton, game_resources::{self, GameResources}, registry::{GameState, ModelHandle, ModelId, RenderState, ResourceRegistry}}};

use super::scene_tree::SceneNodeId;

/// What happens when animation time leaves [0, duration)
#[derive(Clone, Copy)]
pub enum TimeWrapMode {
    Clamp, Repeat, PingPong
}

// TODO: this does nothing atm
/// What to do to with animation values at the wrap seam
#[derive(Clone, Copy)]
pub enum BoundaryMode {
    /// last != first
    Open,
    /// last == first
    Closed,
    /// interpolate last -> first
    Interpolate,
}

pub struct State {
    pub clip_idx: u8,
    pub time_wrap: TimeWrapMode,
    /// This probably doesn't belong to the animation state machine, however, currently the
    /// animation files don't contain the boundary mode so it's included here in the meantime.
    pub boundary_mode: BoundaryMode,
    pub speed: f32,
}

pub struct Transition {
    pub blend_time: f32,
    /// state idx
    pub to: u8,
}

pub struct AnimationGraph {
    /// states[state_idx] => State
    pub states: Vec<State>,
    /// transitions[state_idx] => Transition
    pub transitions: Vec<Transition>,
}

pub struct AnimatorTransitionState {
    /// state idx
    pub from: u8,
    /// transition idx
    pub transition: u8,
    /// anim ticks since the transition to the previous state started
    pub from_start_instance_time: u64,
    /// anim ticks since this transition started
    pub to_start_instance_time: u64,
}

pub struct AnimatorStateState {
    pub state_idx: u8,
    /// anim ticks since the transition into this state started
    pub start_instance_time: u64,
}

pub enum AnimatorState {
    /// state idx
    State(AnimatorStateState),
    Transition(AnimatorTransitionState)
}

pub enum AnimatorError {
    AttemptedTransitionWhilePreviousTransitionStillPlaying
}

const ANIM_TICKS_PER_SEC: u64 = 1000;

pub struct Animator {
    animation_graph: usize,
    current_state: AnimatorState,
    /// main animation timeline in game ticks
    time: u64,
    last_scheduled_time: u64,
}
impl Animator {
    pub fn new(animation_graph: usize, start_state: u8) -> Self {
        Self {
            animation_graph,
            current_state: AnimatorState::State(AnimatorStateState { state_idx: start_state, start_instance_time: 0 }),
            time: 0,
            last_scheduled_time: 0,
        }
        // TODO maybe should schedule one task immediately?
    }

    pub fn get_current_state(&self) -> &AnimatorState {
        &self.current_state
    }

    pub fn transition(&mut self, transition_idx: u8) -> Result<(), AnimatorError> {
        let prev_state = match &self.current_state {
            AnimatorState::State(idx) => Ok(idx),
            _ => Err(AnimatorError::AttemptedTransitionWhilePreviousTransitionStillPlaying)
        }?;
        self.current_state = AnimatorState::Transition(
            AnimatorTransitionState {
                from: prev_state.state_idx,
                transition: transition_idx,
                from_start_instance_time: prev_state.start_instance_time,
                to_start_instance_time: self.time,
            }
        );
        Ok(())
    }

    pub fn update(&mut self, animation_graphs: &Vec<AnimationGraph>, dt: f32) {
        let maybe_updated_state = match &self.current_state {
            AnimatorState::State(state_idx) => None,
            AnimatorState::Transition(AnimatorTransitionState { from, transition, from_start_instance_time, to_start_instance_time }) => {
                let ags = &animation_graphs[self.animation_graph];
                let tr = &ags.transitions[*transition as usize];
                if ((self.time - *to_start_instance_time) / ANIM_TICKS_PER_SEC) as f32 > tr.blend_time {
                    Some(AnimatorState::State(AnimatorStateState { state_idx: tr.to, start_instance_time: *to_start_instance_time }))
                } else { None }
            },
        };
        if let Some(state) = maybe_updated_state {
            self.current_state = state;
        }

        let delta_ticks =
            (dt * ANIM_TICKS_PER_SEC as f32).round() as u64;

        self.time += delta_ticks;
    }

    pub fn build_job(&mut self, dt: f32, animation_graphs: &Vec<AnimationGraph>, node_id: SceneNodeId, model_handle: &ModelHandle, game_resources: &GameResources, resource_registry: &Rc<RefCell<ResourceRegistry>>) -> Vec<AnimPoseTask> {
        let mut job = vec![];

        let model_game_idx = match resource_registry.borrow().get(model_handle).game_state {
            GameState::Absent => return job,
            GameState::Loading => return job,
            GameState::Ready(index) => index,
        };

        let model = game_resources.models.get(model_game_idx).unwrap();

        let skeleton_game_idx = match resource_registry.borrow().get(&model.skeleton).game_state {
            GameState::Ready(index) => index,
            _ => return job,
        };
        let skeleton = game_resources.skeletons.get(skeleton_game_idx).unwrap();

        // make sure that the last pose covers the next snapshot interval
        let delta_ticks =
            (dt * ANIM_TICKS_PER_SEC as f32).round() as u64;
        while self.last_scheduled_time < self.time {
            // at worst there may be one pose ahead and one behind the snapshot interval
            let min_sample_length = delta_ticks / (POSE_STORAGE_BUFFER_SIZE as u64 - 2);
            // TODO logic to determine goal sample rate
            let delta = min_sample_length;
            let next_instance_time = self.last_scheduled_time + delta;
            let task = match &self.current_state {
                AnimatorState::State(animator_state_state) => AnimPoseTask::Single(
                    {
                        let state = &animation_graphs[self.animation_graph].states[animator_state_state.state_idx as usize];
                        let clip_handle = &model.animation_clips[state.clip_idx as usize];
                        let clip_game_idx = match resource_registry.borrow().get(clip_handle).game_state {
                            GameState::Ready(index) => index,
                            _ => return job,
                        };
                        let anim_clip = game_resources.animation_clips.get(clip_game_idx).unwrap();
                        let anim_game_idx = match resource_registry.borrow().get(&anim_clip.animation).game_state {
                            GameState::Ready(index) => index,
                            _ => return job,
                        };
                        let anim = game_resources.animations.get(anim_game_idx).unwrap();
                        SinglePoseTask {
                            instance_time: next_instance_time,
                            node_id,
                            skeleton: skeleton.clone(),
                            clip: anim.clone(),
                            time_wrap: state.time_wrap,
                            boundary_mode: state.boundary_mode,
                            local_time: ((next_instance_time.saturating_sub(animator_state_state.start_instance_time)) as f32 / ANIM_TICKS_PER_SEC as f32) * state.speed,
                        }
                    }
                ),
                AnimatorState::Transition(animator_transition_state) => AnimPoseTask::Blend(
                    {
                        let from_state = &animation_graphs[self.animation_graph].states[animator_transition_state.from as usize];
                        let transition = &animation_graphs[self.animation_graph].transitions[animator_transition_state.transition as usize];
                        let to_state = &animation_graphs[self.animation_graph].states[transition.to as usize];

                        let from_handle = &model.animation_clips[from_state.clip_idx as usize];
                        let to_handle = &model.animation_clips[to_state.clip_idx as usize];
                        let from_game_idx = match resource_registry.borrow().get(from_handle).game_state {
                            GameState::Ready(index) => index,
                            _ => return job,
                        };
                        let to_game_idx = match resource_registry.borrow().get(to_handle).game_state {
                            GameState::Ready(index) => index,
                            _ => return job,
                        };

                        let from_clip = game_resources.animation_clips.get(from_game_idx).unwrap();
                        let from_anim_game_idx = match resource_registry.borrow().get(&from_clip.animation).game_state {
                            GameState::Ready(index) => index,
                            _ => return job,
                        };
                        let from_anim = game_resources.animations.get(from_anim_game_idx).unwrap();

                        let to_clip = game_resources.animation_clips.get(to_game_idx).unwrap();
                        let to_anim_game_idx = match resource_registry.borrow().get(&to_clip.animation).game_state {
                            GameState::Ready(index) => index,
                            _ => return job,
                        };
                        let to_anim = game_resources.animations.get(to_anim_game_idx).unwrap();

                        let from_time = ((next_instance_time.saturating_sub(animator_transition_state.from_start_instance_time)) as f32 / ANIM_TICKS_PER_SEC as f32) * from_state.speed;
                        let to_time = ((next_instance_time.saturating_sub(animator_transition_state.to_start_instance_time)) as f32 / ANIM_TICKS_PER_SEC as f32) * to_state.speed;

                        BlendPoseTask {
                            node_id,
                            instance_time: next_instance_time,
                            skeleton: skeleton.clone(),
                            from_clip: from_anim.clone(),
                            to_clip: to_anim.clone(),
                            blend_time: transition.blend_time,
                            from_time,
                            to_time,
                            from_time_wrap: from_state.time_wrap,
                            to_time_wrap: to_state.time_wrap,
                        }
                    }
                ),
            };
            job.push(task);
            self.last_scheduled_time = next_instance_time;
        }

        job
    }

    pub fn build_snapshot(&self/*, animation_graphs: &Vec<AnimationGraph>, model_handle: &ModelHandle, resource_registry: &Rc<RefCell<ResourceRegistry>>, game_resources: &GameResources */) -> AnimationSnapshot {
        AnimationSnapshot(self.time)
        /*
        let resource_registry = resource_registry.borrow();
        let animation_graph = &animation_graphs[self.animation_graph];
        if let GameState::Ready(model_game_idx) = resource_registry.get(model_handle).game_state {
            let anim_clip_handles = &game_resources.models.get(model_game_idx).unwrap().animation_clips;
            match &self.current_state {
                AnimatorState::State(animator_state_state) => {
                    let state = &animation_graph.states[animator_state_state.state_idx as usize];
                    if let RenderState::Ready(id) = resource_registry.get(&anim_clip_handles[state.clip_idx as usize]).render_state {
                        Some(
                            AnimationSnapshot::AnimationStateSnapshot(
                                AnimationStateSnapshot {
                                    clip_id: AnimationClipRenderId(id),
                                    animation_time: animator_state_state.animation_time,
                                    time_wrap: state.time_wrap,
                                    boundary_mode: state.boundary_mode
                                }
                            )
                        )
                    } else {
                        None
                    }
                },
                AnimatorState::Transition(animator_transition_state) => {
                    let from_state = &animation_graph.states[animator_transition_state.from as usize];
                    let transition = &animation_graph.transitions[animator_transition_state.transition as usize];
                    let to_state = &animation_graph.states[transition.to as usize];
                    if let (
                        RenderState::Ready(from_id),
                        RenderState::Ready(to_id),
                    ) = (
                        &resource_registry.get(&anim_clip_handles[from_state.clip_idx as usize]).render_state,
                        &resource_registry.get(&anim_clip_handles[to_state.clip_idx as usize]).render_state,
                    ) {
                        Some(
                            AnimationSnapshot::AnimationTransitionSnapshot(
                                AnimationTransitionSnapshot {
                                    from_clip_id: AnimationClipRenderId(*from_id),
                                    to_clip_id: AnimationClipRenderId(*to_id),
                                    blend_time: transition.blend_time,
                                    from_time: animator_transition_state.from_time,
                                    to_time: animator_transition_state.to_time,
                                    from_time_wrap: from_state.time_wrap,
                                    to_time_wrap: to_state.time_wrap,
                                }
                            )
                        )
                    } else {
                        None
                    }
                },
            }
        } else {
            None
        }
         */
    }
}

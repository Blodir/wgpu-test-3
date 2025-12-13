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
    /// time in seconds since the transition to the previous state started
    pub from_time: f32,
    /// time in seconds since this transition started
    pub to_time: f32,
}

pub struct AnimatorStateState {
    pub state_idx: u8,
    /// time in seconds since the transition into this state started
    pub animation_time: f32,
}

pub enum AnimatorState {
    /// state idx
    State(AnimatorStateState),
    Transition(AnimatorTransitionState)
}

pub enum AnimatorError {
    AttemptedTransitionWhilePreviousTransitionStillPlaying
}

pub struct AnimationStateSnapshot {
    pub clip_idx: u8,
    pub time_wrap: TimeWrapMode,
    pub boundary_mode: BoundaryMode,
    /// time in seconds since the transition into this state started
    pub animation_time: f32,
}

pub struct AnimationTransitionSnapshot {
    pub from_clip_idx: u8,
    pub to_clip_idx: u8,
    pub blend_time: f32,
    /// time in seconds since the transition to the previous state started
    pub from_time: f32,
    /// time in seconds since this transition started
    pub to_time: f32,
    pub from_time_wrap: TimeWrapMode,
    pub to_time_wrap: TimeWrapMode,
}

pub enum AnimationSnapshot {
    AnimationStateSnapshot(AnimationStateSnapshot),
    AnimationTransitionSnapshot(AnimationTransitionSnapshot),
}

pub struct Animator {
    animation_graph: usize,
    current_state: AnimatorState,
}
impl Animator {
    pub fn new(animation_graph: usize, start_state: u8) -> Self {
        Self {
            animation_graph,
            current_state: AnimatorState::State(AnimatorStateState { state_idx: start_state, animation_time: 0.0 }),
        }
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
                from_time: prev_state.animation_time,
                to_time: 0.0,
            }
        );
        Ok(())
    }

    pub fn update(&mut self, animation_graphs: &Vec<AnimationGraph>, dt: f32) {
        let maybe_updated_state = match &self.current_state {
            AnimatorState::State(state_idx) => None,
            AnimatorState::Transition(AnimatorTransitionState { from, transition, from_time, to_time }) => {
                let ags = &animation_graphs[self.animation_graph];
                let tr = &ags.transitions[*transition as usize];
                if *to_time > tr.blend_time {
                    Some(AnimatorState::State(AnimatorStateState { state_idx: tr.to, animation_time: *to_time }))
                } else { None }
            },
        };
        if let Some(state) = maybe_updated_state {
            self.current_state = state;
        }
        match &mut self.current_state {
            AnimatorState::State(animator_state_state) => {
                let state = &animation_graphs[self.animation_graph].states[animator_state_state.state_idx as usize];
                animator_state_state.animation_time += dt * state.speed;
            },
            AnimatorState::Transition(animator_transition_state) => {
                let ags = &animation_graphs[self.animation_graph];
                let state_1 = &ags.states[animator_transition_state.from as usize];
                let transition = &ags.transitions[animator_transition_state.transition as usize];
                let state_2 = &ags.states[transition.to as usize];
                animator_transition_state.from_time += dt * state_1.speed;
                animator_transition_state.to_time += dt * state_2.speed;
            },
        }
    }

    pub fn build_snapshot(&self, animation_graphs: &Vec<AnimationGraph>) -> AnimationSnapshot {
        let animation_graph = &animation_graphs[self.animation_graph];
        match &self.current_state {
            AnimatorState::State(animator_state_state) => {
                let state = &animation_graph.states[animator_state_state.state_idx as usize];
                AnimationSnapshot::AnimationStateSnapshot(
                    AnimationStateSnapshot {
                        clip_idx: state.clip_idx,
                        animation_time: animator_state_state.animation_time,
                        time_wrap: state.time_wrap,
                        boundary_mode: state.boundary_mode
                    }
                )
            },
            AnimatorState::Transition(animator_transition_state) => {
                let from_state = &animation_graph.states[animator_transition_state.from as usize];
                let transition = &animation_graph.transitions[animator_transition_state.transition as usize];
                let to_state = &animation_graph.states[transition.to as usize];
                AnimationSnapshot::AnimationTransitionSnapshot(
                    AnimationTransitionSnapshot {
                        from_clip_idx: from_state.clip_idx,
                        to_clip_idx: to_state.clip_idx,
                        blend_time: transition.blend_time,
                        from_time: animator_transition_state.from_time,
                        to_time: animator_transition_state.to_time,
                        from_time_wrap: from_state.time_wrap,
                        to_time_wrap: to_state.time_wrap,
                    }
                )
            },
        }
    }
}

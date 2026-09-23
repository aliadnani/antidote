use crossbeam::channel::Sender;
use std::time::Duration;
use tracing::{error, info};

use crate::{
    audio::AudioCommand,
    input::{Input, InputEvent},
    state::State,
};

const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub struct Coordinator<I: Input> {
    input: I,
    state: State,
    audio_command_sender: Sender<AudioCommand>,
}

impl<I: Input> Coordinator<I> {
    pub fn new(input: I, state: State, audio_command_sender: Sender<AudioCommand>) -> Self {
        Coordinator {
            input,
            state,
            audio_command_sender,
        }
    }

    pub fn run(mut self) {
        loop {
            self.tick();
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    pub fn tick(&mut self) {
        match self.input.poll_events() {
            Ok(events) => events.into_iter().for_each(|event| self.handle_event(event)),
            Err(error) => error!(?error, "Failed to poll input events"),
        }
    }

    fn handle_event(&mut self, event: InputEvent) {
        match event {
            InputEvent::FootSwitchRightTap => self.swap_model(CycleDirection::Forward),
            InputEvent::FootSwitchRightHold => self.swap_model(CycleDirection::Backward),
        }
    }

    fn swap_model(&mut self, direction: CycleDirection) {
        let model = match direction {
            CycleDirection::Forward => self.state.next_model(),
            CycleDirection::Backward => self.state.previous_model(),
        };

        let Some(model) = model else {
            error!("No NAM models available to swap to");
            return;
        };

        let model_path = model.as_str().to_string();
        info!(model_path = %model_path, "Hot swapping NAM A2 model");

        if self.audio_command_sender.send(AudioCommand::HotSwapModel(model_path)).is_err() {
            error!("Failed to send HotSwapModel command - audio command channel is closed");
        }
    }
}

enum CycleDirection {
    Forward,
    Backward,
}

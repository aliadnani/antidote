use crossbeam::channel::Sender;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::{
    audio::AudioCommand,
    input::{Input, InputEvent},
    nam_ffi,
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
        debug!(?event, "Coordinator received input event");

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

        // Loading the model is not instant - hence we do it on a separate thread
        // Maybe can refactor it to use a dedicated thread pool later but for now is ok
        let command_sender = self.audio_command_sender.clone();
        std::thread::spawn(move || match nam_ffi::load_nam_a2_model_path(&model_path) {
            Ok(dsp) => {
                info!(model_path = %model_path, "Loaded NAM A2 model");
                if command_sender.send(AudioCommand::InstallModel(dsp)).is_err() {
                    error!("Failed to send InstallModel command - audio command channel is closed");
                }
            }
            Err(error) => {
                error!(model_path = %model_path, ?error, "Failed to load NAM A2 model");
            }
        });
    }
}

enum CycleDirection {
    Forward,
    Backward,
}

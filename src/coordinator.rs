use crossbeam::channel::Sender;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::{
    audio::AudioCommand,
    display::Display,
    input::{Input, InputEvent},
    nam_ffi,
    state::State,
};

const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub struct Coordinator<I: Input, D: Display> {
    input: I,
    state: State,
    audio_command_sender: Sender<AudioCommand>,
    display: D,
}

impl<I: Input, D: Display> Coordinator<I, D> {
    pub fn new(
        input: I,
        state: State,
        audio_command_sender: Sender<AudioCommand>,
        display: D,
    ) -> Self {
        Coordinator {
            input,
            state,
            audio_command_sender,
            display,
        }
    }

    pub fn run(mut self) {
        self.render_current_model();
        loop {
            self.tick();
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    pub fn tick(&mut self) {
        match self.input.poll_events() {
            Ok(events) => events
                .into_iter()
                .for_each(|event| self.handle_event(event)),
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
        self.render_current_model();

        // Loading the model is not instant - hence we do it on a separate thread
        // Maybe can refactor it to use a dedicated thread pool later but for now is ok
        let command_sender = self.audio_command_sender.clone();
        std::thread::spawn(move || match nam_ffi::load_nam_a2_model_path(&model_path) {
            Ok(dsp) => {
                info!(model_path = %model_path, "Loaded NAM A2 model");
                if command_sender
                    .send(AudioCommand::InstallModel(dsp))
                    .is_err()
                {
                    error!("Failed to send InstallModel command - audio command channel is closed");
                }
            }
            Err(error) => {
                error!(model_path = %model_path, ?error, "Failed to load NAM A2 model");
            }
        });
    }

    fn render_current_model(&mut self) {
        let Some(model) = self.state.current_model() else {
            return;
        };
        let Some(profile_index) = self.state.current_model_index() else {
            return;
        };
        let model_path = model.as_str();
        let profile_name = std::path::Path::new(model_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(model_path);
        self.display.render(profile_name, profile_index);
    }
}

enum CycleDirection {
    Forward,
    Backward,
}

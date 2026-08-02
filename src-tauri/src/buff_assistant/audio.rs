use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use rodio::{DeviceSinkBuilder, Source, source::SineWave};

#[derive(Clone, Copy)]
pub enum AudioCue {
    Triggered,
    PrewarnThree,
    PrewarnTwo,
    PrewarnOne,
}

#[derive(Clone)]
pub struct AudioEngine {
    sender: Sender<(AudioCue, f32)>,
}

impl AudioEngine {
    pub fn start() -> (Self, Option<String>) {
        let (sender, receiver) = mpsc::channel::<(AudioCue, f32)>();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let stream = match DeviceSinkBuilder::open_default_sink() {
                Ok(mut stream) => {
                    stream.log_on_drop(false);
                    let _ = ready_sender.send(Ok(()));
                    stream
                }
                Err(error) => {
                    let _ = ready_sender.send(Err(format!("声音设备初始化失败：{error}")));
                    return;
                }
            };
            while let Ok((cue, volume)) = receiver.recv() {
                let (frequency, duration) = match cue {
                    AudioCue::Triggered => (820.0, 180),
                    AudioCue::PrewarnThree | AudioCue::PrewarnTwo | AudioCue::PrewarnOne => {
                        (800.0, 170)
                    }
                };
                let wave = SineWave::new(frequency)
                    .take_duration(Duration::from_millis(duration))
                    .amplify(volume.clamp(0.0, 1.0));
                stream.mixer().add(wave);
            }
        });

        let warning = ready_receiver
            .recv_timeout(Duration::from_secs(2))
            .ok()
            .and_then(Result::err);
        (Self { sender }, warning)
    }

    pub fn play(&self, cue: AudioCue, volume: f32) {
        let _ = self.sender.send((cue, volume));
    }
}

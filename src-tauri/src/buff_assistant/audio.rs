use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use rodio::{Decoder, DeviceSinkBuilder, Player, Source, buffer::SamplesBuffer, source::SineWave};

use super::model::BuffSoundCue;

const MAX_SOUND_DURATION: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub enum ResolvedSoundSource {
    Sine,
    Wav(PathBuf),
}

struct AudioRequest {
    cue: BuffSoundCue,
    source: ResolvedSoundSource,
    volume: f32,
}

#[derive(Clone)]
pub struct AudioEngine {
    sender: Sender<AudioRequest>,
}

impl AudioEngine {
    pub fn start() -> (Self, Option<String>) {
        let (sender, receiver) = mpsc::channel::<AudioRequest>();
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
            let mut player: Option<Player> = None;
            let mut cache = HashMap::<PathBuf, SamplesBuffer>::new();
            while let Ok(request) = receiver.recv() {
                if let Some(previous) = player.take() {
                    previous.stop();
                }
                let next = Player::connect_new(stream.mixer());
                next.set_volume(request.volume.clamp(0.0, 1.0));
                match request.source {
                    ResolvedSoundSource::Sine => next.append(sine_wave(request.cue)),
                    ResolvedSoundSource::Wav(path) => match cached_wav(&path, &mut cache) {
                        Ok(sound) => next.append(sound),
                        Err(_) => next.append(sine_wave(request.cue)),
                    },
                }
                player = Some(next);
            }
        });

        let warning = ready_receiver
            .recv_timeout(Duration::from_secs(2))
            .ok()
            .and_then(Result::err);
        (Self { sender }, warning)
    }

    pub fn play(&self, cue: BuffSoundCue, source: ResolvedSoundSource, volume: f32) {
        let _ = self.sender.send(AudioRequest {
            cue,
            source,
            volume,
        });
    }
}

pub fn validate_wav_file(path: &Path) -> Result<(), String> {
    decode_wav(path).map(|_| ())
}

fn cached_wav(
    path: &Path,
    cache: &mut HashMap<PathBuf, SamplesBuffer>,
) -> Result<SamplesBuffer, String> {
    if let Some(sound) = cache.get(path) {
        return Ok(sound.clone());
    }
    let sound = decode_wav(path)?;
    cache.insert(path.to_path_buf(), sound.clone());
    Ok(sound)
}

fn decode_wav(path: &Path) -> Result<SamplesBuffer, String> {
    let file = File::open(path).map_err(|error| format!("读取 WAV 文件失败：{error}"))?;
    let decoder = Decoder::try_from(file).map_err(|error| format!("WAV 解码失败：{error}"))?;
    let duration = decoder
        .total_duration()
        .ok_or_else(|| "无法确定 WAV 时长".to_string())?;
    if duration.is_zero() {
        return Err("WAV 文件没有可播放内容".into());
    }
    if duration > MAX_SOUND_DURATION {
        return Err("WAV 文件不能超过 10 秒".into());
    }
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    Ok(SamplesBuffer::new(
        channels,
        sample_rate,
        decoder.collect::<Vec<_>>(),
    ))
}

fn sine_wave(cue: BuffSoundCue) -> impl Source + Send + 'static {
    let (frequency, duration) = match cue {
        BuffSoundCue::Triggered => (820.0, 180),
        BuffSoundCue::PrewarnThree | BuffSoundCue::PrewarnTwo | BuffSoundCue::PrewarnOne => {
            (800.0, 170)
        }
    };
    SineWave::new(frequency).take_duration(Duration::from_millis(duration))
}

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError, Sender},
    thread,
    time::{Duration, Instant},
};

use rodio::{
    Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source, buffer::SamplesBuffer,
    source::SineWave,
};
use tauri::{AppHandle, Emitter};

use super::model::BuffSoundCue;

const MAX_SOUND_DURATION: Duration = Duration::from_secs(10);
const PLAYBACK_POLL_INTERVAL: Duration = Duration::from_millis(20);
const AUDIO_IDLE_TIMEOUT: Duration = Duration::from_secs(2);

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

enum AudioCommand {
    Preload(Vec<PathBuf>),
    Play(AudioRequest),
}

#[derive(Clone)]
pub struct AudioEngine {
    sender: Sender<AudioCommand>,
}

impl AudioEngine {
    pub fn start(app: AppHandle) -> Self {
        let (sender, receiver) = mpsc::channel::<AudioCommand>();
        thread::spawn(move || {
            let mut stream = None::<MixerDeviceSink>;
            let mut players = Vec::<Player>::new();
            let mut idle_since = None;
            let mut cache = HashMap::<PathBuf, SamplesBuffer>::new();

            loop {
                let command = if stream.is_some() {
                    match receiver.recv_timeout(PLAYBACK_POLL_INTERVAL) {
                        Ok(command) => Some(command),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match receiver.recv() {
                        Ok(command) => Some(command),
                        Err(_) => break,
                    }
                };

                match command {
                    None => {}
                    Some(AudioCommand::Preload(paths)) => preload_wavs(paths, &mut cache),
                    Some(AudioCommand::Play(request)) => {
                        players.retain(|player| !player.empty());
                        if stream.is_none() {
                            match DeviceSinkBuilder::open_default_sink() {
                                Ok(mut opened) => {
                                    opened.log_on_drop(false);
                                    stream = Some(opened);
                                }
                                Err(error) => {
                                    let _ = app.emit(
                                        "buff-assistant-execution-log",
                                        format!("提示音播放失败：无法打开声音设备：{error}"),
                                    );
                                    continue;
                                }
                            }
                        }

                        let next = Player::connect_new(
                            stream.as_ref().expect("audio stream was opened").mixer(),
                        );
                        next.set_volume(request.volume.clamp(0.0, 1.0));
                        match request.source {
                            ResolvedSoundSource::Sine => next.append(sine_wave(request.cue)),
                            ResolvedSoundSource::Wav(path) => match cached_wav(&path, &mut cache) {
                                Ok(sound) => next.append(sound),
                                Err(_) => next.append(sine_wave(request.cue)),
                            },
                        }
                        players.push(next);
                        idle_since = None;
                    }
                }

                if stream.is_some() {
                    players.retain(|player| !player.empty());
                    if should_release_audio_session(
                        !players.is_empty(),
                        Instant::now(),
                        &mut idle_since,
                    ) {
                        players.clear();
                        stream = None;
                        idle_since = None;
                    }
                }
            }
        });

        Self { sender }
    }

    pub fn play(&self, cue: BuffSoundCue, source: ResolvedSoundSource, volume: f32) {
        let _ = self.sender.send(AudioCommand::Play(AudioRequest {
            cue,
            source,
            volume,
        }));
    }

    pub fn preload(&self, paths: Vec<PathBuf>) {
        let _ = self.sender.send(AudioCommand::Preload(paths));
    }
}

fn should_release_audio_session(
    has_active_players: bool,
    now: Instant,
    idle_since: &mut Option<Instant>,
) -> bool {
    if has_active_players {
        *idle_since = None;
        return false;
    }

    match idle_since {
        Some(started_at) => now.duration_since(*started_at) >= AUDIO_IDLE_TIMEOUT,
        None => {
            *idle_since = Some(now);
            false
        }
    }
}

fn preload_wavs(paths: Vec<PathBuf>, cache: &mut HashMap<PathBuf, SamplesBuffer>) {
    for path in paths.into_iter().collect::<HashSet<_>>() {
        let _ = cached_wav(&path, cache);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preload_and_playback_lookup_share_the_same_cache() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/buff-sounds/template-1/triggered.wav");
        let mut cache = HashMap::new();

        preload_wavs(vec![path.clone(), path.clone()], &mut cache);

        assert_eq!(cache.len(), 1);
        assert!(cached_wav(&path, &mut cache).is_ok());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn preload_ignores_invalid_paths_without_poisoning_the_cache() {
        let mut cache = HashMap::new();

        preload_wavs(vec![PathBuf::from("missing-sound.wav")], &mut cache);

        assert!(cache.is_empty());
    }

    #[test]
    fn active_playback_keeps_the_audio_session_open() {
        let now = Instant::now();
        let mut idle_since = Some(now - AUDIO_IDLE_TIMEOUT);

        assert!(!should_release_audio_session(true, now, &mut idle_since));
        assert!(idle_since.is_none());
    }

    #[test]
    fn finished_playback_starts_idle_timer_without_releasing_immediately() {
        let now = Instant::now();
        let mut idle_since = None;

        assert!(!should_release_audio_session(false, now, &mut idle_since));
        assert_eq!(idle_since, Some(now));
    }

    #[test]
    fn audio_session_stays_open_before_idle_timeout() {
        let now = Instant::now();
        let mut idle_since = Some(now - AUDIO_IDLE_TIMEOUT + Duration::from_millis(1));

        assert!(!should_release_audio_session(false, now, &mut idle_since));
    }

    #[test]
    fn audio_session_is_released_at_idle_timeout() {
        let now = Instant::now();
        let mut idle_since = Some(now - AUDIO_IDLE_TIMEOUT);

        assert!(should_release_audio_session(false, now, &mut idle_since));
    }

    #[test]
    fn resumed_playback_cancels_idle_release() {
        let now = Instant::now();
        let mut idle_since = Some(now - Duration::from_secs(1));

        assert!(!should_release_audio_session(true, now, &mut idle_since));
        assert!(idle_since.is_none());
    }
}

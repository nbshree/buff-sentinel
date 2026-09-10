use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::{Path, PathBuf},
    str::FromStr,
    sync::mpsc::{self, RecvTimeoutError, Sender, SyncSender},
    thread,
    time::{Duration, Instant},
};

use rodio::{
    Decoder, Device, DeviceSinkBuilder, DeviceTrait, MixerDeviceSink, Player, Source,
    buffer::SamplesBuffer,
    cpal::{DeviceId, traits::HostTrait},
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
    output_device_id: Option<String>,
    allow_fallback: bool,
}

enum AudioCommand {
    Preload(Vec<PathBuf>),
    Play(AudioRequest),
    Test(AudioRequest, SyncSender<Result<(), String>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

struct ResolvedOutputDevice {
    device: Device,
    id: String,
    warning: Option<String>,
    requested_id: Option<String>,
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
            let mut active_device_id = None::<String>;
            let mut last_route_warning = None::<String>;

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
                        if let Err(error) = play_request(
                            request,
                            &mut stream,
                            &mut active_device_id,
                            &mut players,
                            &mut idle_since,
                            &mut cache,
                            &app,
                            &mut last_route_warning,
                        ) {
                            let _ = app.emit(
                                "buff-assistant-execution-log",
                                format!("提示音播放失败：{error}"),
                            );
                        }
                    }
                    Some(AudioCommand::Test(request, response)) => {
                        let result = play_request(
                            request,
                            &mut stream,
                            &mut active_device_id,
                            &mut players,
                            &mut idle_since,
                            &mut cache,
                            &app,
                            &mut last_route_warning,
                        );
                        let _ = response.send(result);
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
                        active_device_id = None;
                        idle_since = None;
                    }
                }
            }
        });

        Self { sender }
    }

    pub fn play(
        &self,
        cue: BuffSoundCue,
        source: ResolvedSoundSource,
        volume: f32,
        output_device_id: Option<String>,
    ) {
        let _ = self.sender.send(AudioCommand::Play(AudioRequest {
            cue,
            source,
            volume,
            output_device_id,
            allow_fallback: true,
        }));
    }

    pub fn test_output(&self, output_device_id: Option<String>) -> Result<(), String> {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.sender
            .send(AudioCommand::Test(
                AudioRequest {
                    cue: BuffSoundCue::Triggered,
                    source: ResolvedSoundSource::Sine,
                    volume: 0.45,
                    output_device_id,
                    allow_fallback: false,
                },
                response_sender,
            ))
            .map_err(|_| "声音播放线程不可用".to_string())?;
        response_receiver
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "等待声音设备响应超时".to_string())?
    }

    pub fn preload(&self, paths: Vec<PathBuf>) {
        let _ = self.sender.send(AudioCommand::Preload(paths));
    }
}

pub fn list_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    let host = rodio::cpal::default_host();
    let default_id = host
        .default_output_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());
    let devices = host
        .output_devices()
        .map_err(|error| format!("无法读取声音输出设备：{error}"))?;
    let mut result = devices
        .filter_map(|device| {
            let id = device.id().ok()?.to_string();
            let name = device
                .description()
                .map(|description| description.name().to_string())
                .unwrap_or_else(|_| "未知输出设备".into());
            Some(AudioOutputDevice {
                is_default: default_id.as_deref() == Some(id.as_str()),
                id,
                name,
            })
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn play_request(
    request: AudioRequest,
    stream: &mut Option<MixerDeviceSink>,
    active_device_id: &mut Option<String>,
    players: &mut Vec<Player>,
    idle_since: &mut Option<Instant>,
    cache: &mut HashMap<PathBuf, SamplesBuffer>,
    app: &AppHandle,
    last_route_warning: &mut Option<String>,
) -> Result<(), String> {
    players.retain(|player| !player.empty());
    let mut resolved =
        resolve_output_device(request.output_device_id.as_deref(), request.allow_fallback)?;
    if resolved.warning.is_some() {
        emit_route_warning(app, resolved.warning.as_deref(), last_route_warning);
    }

    if active_device_id.as_deref() != Some(resolved.id.as_str()) {
        players.clear();
        *stream = None;
        let opened = open_device_sink(resolved.device);
        let (mut opened, warning) = match opened {
            Ok(opened) => (opened, resolved.warning.take()),
            Err(error) if resolved.requested_id.is_some() && request.allow_fallback => {
                let fallback = resolve_output_device(None, true)?;
                let fallback_id = fallback.id.clone();
                let opened = open_device_sink(fallback.device).map_err(|fallback_error| {
                    format!(
                        "无法打开指定声音设备：{error}；系统默认设备也无法打开：{fallback_error}"
                    )
                })?;
                resolved.id = fallback_id;
                (
                    opened,
                    Some("已选择的播报输出设备无法打开，临时改用系统默认设备".into()),
                )
            }
            Err(error) => return Err(format!("无法打开声音设备：{error}")),
        };
        emit_route_warning(app, warning.as_deref(), last_route_warning);
        opened.log_on_drop(false);
        *stream = Some(opened);
        *active_device_id = Some(resolved.id);
    } else if resolved.warning.is_none() {
        emit_route_warning(app, None, last_route_warning);
    }

    let next = Player::connect_new(stream.as_ref().expect("audio stream was opened").mixer());
    next.set_volume(request.volume.clamp(0.0, 1.0));
    match request.source {
        ResolvedSoundSource::Sine => next.append(sine_wave(request.cue)),
        ResolvedSoundSource::Wav(path) => match cached_wav(&path, cache) {
            Ok(sound) => next.append(sound),
            Err(_) => next.append(sine_wave(request.cue)),
        },
    }
    players.push(next);
    *idle_since = None;
    Ok(())
}

fn resolve_output_device(
    requested_id: Option<&str>,
    allow_fallback: bool,
) -> Result<ResolvedOutputDevice, String> {
    let host = rodio::cpal::default_host();
    if let Some(requested_id) = requested_id {
        let selected = DeviceId::from_str(requested_id)
            .ok()
            .and_then(|device_id| host.device_by_id(&device_id));
        if let Some(device) = selected {
            return Ok(ResolvedOutputDevice {
                id: requested_id.to_string(),
                device,
                warning: None,
                requested_id: Some(requested_id.to_string()),
            });
        }

        if !allow_fallback {
            return Err("已选择的播报输出设备当前不可用，请刷新设备列表后重试".into());
        }

        let fallback = host
            .default_output_device()
            .ok_or_else(|| "指定的输出设备不可用，且系统没有默认输出设备".to_string())?;
        let fallback_id = fallback
            .id()
            .map_err(|error| format!("无法读取系统默认输出设备标识：{error}"))?
            .to_string();
        return Ok(ResolvedOutputDevice {
            device: fallback,
            id: fallback_id,
            warning: Some("已选择的播报输出设备不可用，临时改用系统默认设备".into()),
            requested_id: Some(requested_id.to_string()),
        });
    }

    let device = host
        .default_output_device()
        .ok_or_else(|| "系统没有可用的默认输出设备".to_string())?;
    let id = device
        .id()
        .map_err(|error| format!("无法读取系统默认输出设备标识：{error}"))?
        .to_string();
    Ok(ResolvedOutputDevice {
        device,
        id,
        warning: None,
        requested_id: None,
    })
}

fn open_device_sink(device: Device) -> Result<MixerDeviceSink, String> {
    DeviceSinkBuilder::from_device(device)
        .and_then(|builder| builder.open_sink_or_fallback())
        .map_err(|error| error.to_string())
}

fn emit_route_warning(
    app: &AppHandle,
    warning: Option<&str>,
    last_route_warning: &mut Option<String>,
) {
    match warning {
        Some(warning) if last_route_warning.as_deref() != Some(warning) => {
            let _ = app.emit("buff-assistant-execution-log", warning.to_string());
            *last_route_warning = Some(warning.to_string());
        }
        None => *last_route_warning = None,
        Some(_) => {}
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

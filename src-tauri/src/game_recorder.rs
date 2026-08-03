use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    input,
    model::{
        EMERGENCY_STOP_HOTKEY, LoopMode, create_id, normalize_hotkey, now_millis, truncate_chars,
    },
    raw_input::{self, RawInputEvent, RawInputKind, RawMouseButton},
    shortcuts,
    state::AppState,
};

const STORAGE_DIRECTORY: &str = "game-recordings";
const INDEX_FILE_NAME: &str = "index.json";
const SCHEMA_VERSION: u32 = 1;
const COUNTDOWN_SECONDS: u32 = 3;
const MAX_RECORDING_DURATION_MS: u64 = 10 * 60 * 1000;
const MAX_RECORDED_EVENTS: usize = 50_000;
const MAX_STORED_EVENTS: usize = MAX_RECORDED_EVENTS * 2 + 3;
const MAX_RECORDING_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MOVE_BUCKET_MS: u64 = 16;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameRecorderActivity {
    #[default]
    Idle,
    RecordingCountdown,
    Recording,
    PlaybackCountdown,
    Playing,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRecorderHotkeys {
    pub record_start: String,
    pub stop: String,
    pub playback_start: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePlaybackSettings {
    pub speed: f64,
    pub loop_mode: LoopMode,
    pub loop_count: u32,
    pub loop_interval_seconds: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTarget {
    pub process_name: String,
    pub window_title: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRecordingSummary {
    pub id: String,
    pub name: String,
    pub duration_ms: u64,
    pub event_count: usize,
    pub keyboard_event_count: usize,
    pub mouse_event_count: usize,
    pub target: GameTarget,
    pub created_at: i64,
    pub updated_at: i64,
    pub playback: GamePlaybackSettings,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRecorderState {
    pub recordings: Vec<GameRecordingSummary>,
    pub active_recording_id: Option<String>,
    pub hotkeys: GameRecorderHotkeys,
    pub activity: GameRecorderActivity,
    pub countdown_remaining: u32,
    pub completed_loops: u32,
    pub target_mismatch: bool,
    pub hotkey_errors: Vec<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GameRecordedEvent {
    MouseMove {
        at_ms: u64,
        dx: i32,
        dy: i32,
    },
    MouseButton {
        at_ms: u64,
        button: RawMouseButton,
        pressed: bool,
    },
    MouseWheel {
        at_ms: u64,
        delta: i16,
    },
    Key {
        at_ms: u64,
        scan_code: u16,
        extended: bool,
        pressed: bool,
    },
}

impl GameRecordedEvent {
    fn at_ms(&self) -> u64 {
        match self {
            Self::MouseMove { at_ms, .. }
            | Self::MouseButton { at_ms, .. }
            | Self::MouseWheel { at_ms, .. }
            | Self::Key { at_ms, .. } => *at_ms,
        }
    }

    fn is_keyboard(&self) -> bool {
        matches!(self, Self::Key { .. })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameRecordingFile {
    schema_version: u32,
    id: String,
    name: String,
    target: GameTarget,
    created_at: i64,
    updated_at: i64,
    #[serde(default)]
    duration_ms: u64,
    events: Vec<GameRecordedEvent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameRecorderIndex {
    schema_version: u32,
    active_recording_id: Option<String>,
    hotkeys: GameRecorderHotkeys,
    recordings: Vec<GameRecordingSummary>,
}

impl Default for GameRecorderIndex {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            active_recording_id: None,
            hotkeys: default_hotkeys(),
            recordings: Vec::new(),
        }
    }
}

struct PendingMove {
    at_ms: u64,
    dx: i32,
    dy: i32,
}

struct RecordingSession {
    started_at: Instant,
    target: GameTarget,
    events: Vec<GameRecordedEvent>,
    pending_move: Option<PendingMove>,
    pressed_keys: HashSet<(u16, bool)>,
    pressed_buttons: HashSet<RawMouseButton>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecordingStopReason {
    Duration,
    Capacity,
}

impl RecordingSession {
    fn new(target: GameTarget) -> Self {
        Self {
            started_at: Instant::now(),
            target,
            events: Vec::new(),
            pending_move: None,
            pressed_keys: HashSet::new(),
            pressed_buttons: HashSet::new(),
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    fn event_count(&self) -> usize {
        self.events.len() + usize::from(self.pending_move.is_some())
    }

    fn record(&mut self, event: RawInputEvent) -> Option<RecordingStopReason> {
        let elapsed = event.captured_at.checked_duration_since(self.started_at)?;
        self.record_at(event.kind, elapsed.as_millis() as u64)
    }

    fn record_at(&mut self, event: RawInputKind, at_ms: u64) -> Option<RecordingStopReason> {
        if at_ms >= MAX_RECORDING_DURATION_MS {
            return Some(RecordingStopReason::Duration);
        }
        if self.event_count() >= MAX_RECORDED_EVENTS {
            return Some(RecordingStopReason::Capacity);
        }
        match event {
            RawInputKind::MouseMove { dx, dy } => {
                if dx == 0 && dy == 0 {
                    return None;
                }
                match &mut self.pending_move {
                    Some(pending) if at_ms.saturating_sub(pending.at_ms) < MOVE_BUCKET_MS => {
                        pending.dx = pending.dx.saturating_add(dx);
                        pending.dy = pending.dy.saturating_add(dy);
                    }
                    Some(_) => {
                        self.flush_move();
                        if self.events.len() >= MAX_RECORDED_EVENTS {
                            return Some(RecordingStopReason::Capacity);
                        }
                        self.pending_move = Some(PendingMove { at_ms, dx, dy });
                    }
                    None => self.pending_move = Some(PendingMove { at_ms, dx, dy }),
                }
            }
            RawInputKind::MouseButton { button, pressed } => {
                self.flush_move();
                if self.events.len() >= MAX_RECORDED_EVENTS {
                    return Some(RecordingStopReason::Capacity);
                }
                let changed = if pressed {
                    self.pressed_buttons.insert(button)
                } else {
                    self.pressed_buttons.remove(&button)
                };
                if changed {
                    self.events.push(GameRecordedEvent::MouseButton {
                        at_ms,
                        button,
                        pressed,
                    });
                }
            }
            RawInputKind::MouseWheel { delta } => {
                self.flush_move();
                if self.events.len() >= MAX_RECORDED_EVENTS {
                    return Some(RecordingStopReason::Capacity);
                }
                if delta != 0 {
                    self.events
                        .push(GameRecordedEvent::MouseWheel { at_ms, delta });
                }
            }
            RawInputKind::Key {
                scan_code,
                extended,
                pressed,
            } => {
                self.flush_move();
                if self.events.len() >= MAX_RECORDED_EVENTS {
                    return Some(RecordingStopReason::Capacity);
                }
                let identity = (scan_code, extended);
                let changed = if pressed {
                    self.pressed_keys.insert(identity)
                } else {
                    self.pressed_keys.remove(&identity)
                };
                if changed {
                    self.events.push(GameRecordedEvent::Key {
                        at_ms,
                        scan_code,
                        extended,
                        pressed,
                    });
                }
            }
        }
        (self.event_count() >= MAX_RECORDED_EVENTS).then_some(RecordingStopReason::Capacity)
    }

    fn flush_move(&mut self) {
        if let Some(move_event) = self.pending_move.take()
            && (move_event.dx != 0 || move_event.dy != 0)
            && self.events.len() < MAX_RECORDED_EVENTS
        {
            self.events.push(GameRecordedEvent::MouseMove {
                at_ms: move_event.at_ms,
                dx: move_event.dx,
                dy: move_event.dy,
            });
        }
    }

    fn finish(
        mut self,
        stop_hotkey: Option<&str>,
        ended_at: Option<Instant>,
    ) -> (Vec<GameRecordedEvent>, u64, GameTarget) {
        let duration_ms = ended_at
            .and_then(|ended_at| ended_at.checked_duration_since(self.started_at))
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_else(|| self.elapsed_ms())
            .min(MAX_RECORDING_DURATION_MS);
        self.flush_move();
        if let Some(hotkey) = stop_hotkey {
            trim_hotkey_tail(&mut self.events, hotkey, duration_ms);
        }
        let (pressed_keys, pressed_buttons) = pressed_state(&self.events);
        // Safety releases are allowed past the 50,000 captured-event ceiling. Dropping them would
        // turn a capacity stop into a recording that holds a key or mouse button forever.
        for (scan_code, extended) in pressed_keys {
            self.events.push(GameRecordedEvent::Key {
                at_ms: duration_ms,
                scan_code,
                extended,
                pressed: false,
            });
        }
        for button in pressed_buttons {
            self.events.push(GameRecordedEvent::MouseButton {
                at_ms: duration_ms,
                button,
                pressed: false,
            });
        }
        (self.events, duration_ms, self.target)
    }
}

pub struct GameRecorderRuntime {
    state: GameRecorderState,
    index: GameRecorderIndex,
    storage_dir: PathBuf,
    token: u64,
    listener_ready: bool,
    pending_mismatch_id: Option<String>,
    recording: Option<RecordingSession>,
    playback_keys: HashSet<(u16, bool)>,
    playback_buttons: HashSet<RawMouseButton>,
    pending_release: bool,
}

pub struct GameRecorder {
    inner: Mutex<GameRecorderRuntime>,
}

struct TemporaryActivityClaim {
    app: AppHandle,
}

impl Drop for TemporaryActivityClaim {
    fn drop(&mut self) {
        release_activity(&self.app);
    }
}

impl GameRecorder {
    pub fn load(storage_dir: PathBuf) -> (Self, Vec<String>) {
        let (mut index, notices) = load_index(&storage_dir);
        sanitize_index(&mut index, &storage_dir);
        let state = state_from_index(&index);
        (
            Self {
                inner: Mutex::new(GameRecorderRuntime {
                    state,
                    index,
                    storage_dir,
                    token: 0,
                    listener_ready: false,
                    pending_mismatch_id: None,
                    recording: None,
                    playback_keys: HashSet::new(),
                    playback_buttons: HashSet::new(),
                    pending_release: false,
                }),
            },
            notices,
        )
    }

    pub fn lock(&self) -> MutexGuard<'_, GameRecorderRuntime> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn snapshot(&self) -> GameRecorderState {
        self.lock().state.clone()
    }

    pub fn replace_hotkey_errors(&self, app: &AppHandle, errors: Vec<String>) {
        let snapshot = {
            let mut inner = self.lock();
            inner.state.hotkey_errors = errors;
            inner.state.clone()
        };
        emit_state(app, &snapshot);
    }
}

pub fn storage_directory(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join(STORAGE_DIRECTORY))
        .map_err(|error| format!("无法确定游戏录制目录：{error}"))
}

pub fn start_raw_input_listener(app: &AppHandle) -> Result<(), String> {
    let app_handle = app.clone();
    let result = raw_input::start_listener(move |event| handle_raw_input(&app_handle, event));
    let state = app.state::<GameRecorder>();
    let snapshot = {
        let mut inner = state.lock();
        inner.listener_ready = result.is_ok();
        if let Err(error) = &result {
            inner.state.last_error = Some(error.clone());
        }
        inner.state.clone()
    };
    emit_state(app, &snapshot);
    result
}

#[tauri::command]
pub fn get_game_recorder_state(state: State<'_, GameRecorder>) -> GameRecorderState {
    state.snapshot()
}

#[tauri::command]
pub fn start_game_recording(app: AppHandle) -> Result<GameRecorderState, String> {
    start_game_recording_internal(&app)
}

#[tauri::command]
pub fn stop_game_activity(app: AppHandle) -> GameRecorderState {
    stop_game_activity_with_source(&app, None)
}

#[tauri::command]
pub fn start_game_playback(
    app: AppHandle,
    allow_target_mismatch: bool,
) -> Result<GameRecorderState, String> {
    start_game_playback_internal(&app, allow_target_mismatch)
}

#[tauri::command]
pub fn select_game_recording(
    app: AppHandle,
    state: State<'_, GameRecorder>,
    id: String,
) -> Result<GameRecorderState, String> {
    ensure_global_idle(&app)?;
    let snapshot = {
        let mut inner = state.lock();
        ensure_idle(&inner)?;
        if !inner
            .index
            .recordings
            .iter()
            .any(|recording| recording.id == id)
        {
            return Err("找不到要选择的游戏录制".into());
        }
        let previous_id = inner.index.active_recording_id.clone();
        let previous_pending_mismatch = inner.pending_mismatch_id.clone();
        let previous_target_mismatch = inner.state.target_mismatch;
        inner.index.active_recording_id = Some(id.clone());
        inner.state.active_recording_id = Some(id);
        inner.pending_mismatch_id = None;
        inner.state.target_mismatch = false;
        if let Err(error) = persist_index(&inner) {
            inner.index.active_recording_id = previous_id.clone();
            inner.state.active_recording_id = previous_id;
            inner.pending_mismatch_id = previous_pending_mismatch;
            inner.state.target_mismatch = previous_target_mismatch;
            return Err(error);
        }
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn rename_game_recording(
    app: AppHandle,
    state: State<'_, GameRecorder>,
    id: String,
    name: String,
) -> Result<GameRecorderState, String> {
    ensure_global_idle(&app)?;
    let name = sanitize_recording_name(&name, "游戏录制");
    let snapshot = {
        let mut inner = state.lock();
        ensure_idle(&inner)?;
        if !inner
            .index
            .recordings
            .iter()
            .any(|recording| recording.id == id)
        {
            return Err("找不到要重命名的游戏录制".into());
        }
        let path = recording_path(&inner.storage_dir, &id)?;
        let mut recording = load_recording(&path, &id)?;
        let previous_recording = recording.clone();
        let previous_index = inner.index.clone();
        let now = now_millis();
        recording.name = name.clone();
        recording.updated_at = now;
        save_recording(&path, &recording)?;
        let summary = inner
            .index
            .recordings
            .iter_mut()
            .find(|recording| recording.id == id)
            .ok_or_else(|| "找不到要重命名的游戏录制".to_string())?;
        summary.name = name;
        summary.updated_at = now;
        if let Err(error) = persist_index(&inner) {
            inner.index = previous_index;
            let rollback_error = save_recording(&path, &previous_recording).err();
            return Err(match rollback_error {
                Some(rollback_error) => {
                    format!("{error}；同时恢复原录制名称失败：{rollback_error}")
                }
                None => error,
            });
        }
        sync_public_state(&mut inner);
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn delete_game_recording(
    app: AppHandle,
    state: State<'_, GameRecorder>,
    id: String,
) -> Result<GameRecorderState, String> {
    ensure_global_idle(&app)?;
    let snapshot = {
        let mut inner = state.lock();
        ensure_idle(&inner)?;
        let path = recording_path(&inner.storage_dir, &id)?;
        let previous_index = inner.index.clone();
        let previous_pending_mismatch = inner.pending_mismatch_id.clone();
        let previous_target_mismatch = inner.state.target_mismatch;
        let original_len = inner.index.recordings.len();
        inner
            .index
            .recordings
            .retain(|recording| recording.id != id);
        if inner.index.recordings.len() == original_len {
            return Err("找不到要删除的游戏录制".into());
        }
        if inner.index.active_recording_id.as_deref() == Some(&id) {
            inner.index.active_recording_id = inner
                .index
                .recordings
                .first()
                .map(|recording| recording.id.clone());
        }
        if inner.pending_mismatch_id.as_deref() == Some(&id) {
            inner.pending_mismatch_id = None;
            inner.state.target_mismatch = false;
        }
        if let Err(error) = persist_index(&inner) {
            inner.index = previous_index;
            inner.pending_mismatch_id = previous_pending_mismatch;
            inner.state.target_mismatch = previous_target_mismatch;
            return Err(error);
        }
        sync_public_state(&mut inner);
        inner.state.last_error = match fs::remove_file(&path) {
            Ok(()) => None,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => Some(format!("录制已从索引删除，但清理录制文件失败：{error}")),
        };
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn update_game_recorder_hotkeys(
    app: AppHandle,
    state: State<'_, GameRecorder>,
    hotkeys: GameRecorderHotkeys,
) -> GameRecorderState {
    let _activity_claim = match claim_temporary_activity(&app) {
        Ok(claim) => claim,
        Err(error) => {
            let snapshot = {
                let mut inner = state.lock();
                inner.state.last_error = Some(error);
                inner.state.clone()
            };
            emit_state(&app, &snapshot);
            return snapshot;
        }
    };
    let hotkeys = GameRecorderHotkeys {
        record_start: normalize_hotkey(&hotkeys.record_start),
        stop: normalize_hotkey(&hotkeys.stop),
        playback_start: normalize_hotkey(&hotkeys.playback_start),
    };
    let errors = validate_game_hotkeys(&hotkeys);
    if !errors.is_empty() {
        let snapshot = {
            let mut inner = state.lock();
            inner.state.hotkey_errors = errors;
            inner.state.last_error = Some("配置未保存：游戏录制热键存在冲突".into());
            inner.state.clone()
        };
        emit_state(&app, &snapshot);
        return snapshot;
    }

    let (snapshot, should_register) = {
        let mut inner = state.lock();
        if ensure_idle(&inner).is_err() {
            inner.state.last_error = Some("录制或回放中不能修改热键".into());
            return inner.state.clone();
        }
        let previous_hotkeys = inner.index.hotkeys.clone();
        inner.index.hotkeys = hotkeys.clone();
        inner.state.hotkeys = hotkeys;
        inner.state.hotkey_errors.clear();
        inner.state.last_error = None;
        let should_register = if let Err(error) = persist_index(&inner) {
            inner.index.hotkeys = previous_hotkeys.clone();
            inner.state.hotkeys = previous_hotkeys;
            inner.state.last_error = Some(error);
            false
        } else {
            true
        };
        (inner.state.clone(), should_register)
    };
    emit_state(&app, &snapshot);
    if should_register {
        shortcuts::register_shortcuts(&app);
        app.state::<GameRecorder>().snapshot()
    } else {
        snapshot
    }
}

#[tauri::command]
pub fn update_game_playback_settings(
    app: AppHandle,
    state: State<'_, GameRecorder>,
    id: String,
    settings: GamePlaybackSettings,
) -> Result<GameRecorderState, String> {
    ensure_global_idle(&app)?;
    let settings = sanitize_playback(settings);
    let snapshot = {
        let mut inner = state.lock();
        ensure_idle(&inner)?;
        let previous_index = inner.index.clone();
        let summary = inner
            .index
            .recordings
            .iter_mut()
            .find(|recording| recording.id == id)
            .ok_or_else(|| "找不到要更新的游戏录制".to_string())?;
        summary.playback = settings;
        summary.updated_at = now_millis();
        if let Err(error) = persist_index(&inner) {
            inner.index = previous_index;
            return Err(error);
        }
        sync_public_state(&mut inner);
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

pub(crate) fn start_game_recording_internal(app: &AppHandle) -> Result<GameRecorderState, String> {
    retry_pending_release_before_start(app)?;
    if let Err(error) = claim_activity(app) {
        set_last_error(app, error.clone());
        app.state::<AppState>().log(app, &error);
        return Err(error);
    }
    let state = app.state::<GameRecorder>();
    let result = {
        let mut inner = state.lock();
        if !inner.listener_ready || !raw_input::is_listener_alive() {
            inner.listener_ready = false;
            Err(inner
                .state
                .last_error
                .clone()
                .unwrap_or_else(|| "Raw Input 监听器未运行，无法开始游戏录制".into()))
        } else if inner.state.activity != GameRecorderActivity::Idle {
            Err("已有游戏录制或回放任务正在进行".into())
        } else {
            inner.token = inner.token.wrapping_add(1);
            inner.recording = None;
            inner.state.activity = GameRecorderActivity::RecordingCountdown;
            inner.state.countdown_remaining = COUNTDOWN_SECONDS;
            inner.state.completed_loops = 0;
            inner.state.target_mismatch = false;
            inner.state.last_error = None;
            Ok((inner.token, inner.state.clone()))
        }
    };
    let (token, snapshot) = match result {
        Ok(value) => value,
        Err(error) => {
            release_activity(app);
            set_last_error(app, error.clone());
            return Err(error);
        }
    };
    emit_state(app, &snapshot);
    app.state::<AppState>().log(app, "3 秒后开始游戏操作录制");
    let app_handle = app.clone();
    thread::spawn(move || recording_countdown(app_handle, token));
    Ok(snapshot)
}

pub(crate) fn start_game_playback_internal(
    app: &AppHandle,
    allow_target_mismatch: bool,
) -> Result<GameRecorderState, String> {
    let result = start_game_playback_impl(app, allow_target_mismatch);
    if let Err(error) = &result {
        set_last_error(app, error.clone());
        app.state::<AppState>().log(app, error);
    }
    result
}

fn start_game_playback_impl(
    app: &AppHandle,
    allow_target_mismatch: bool,
) -> Result<GameRecorderState, String> {
    retry_pending_release_before_start(app)?;
    let (recording, playback) = {
        let state = app.state::<GameRecorder>();
        let inner = state.lock();
        ensure_idle(&inner)?;
        let id = inner
            .index
            .active_recording_id
            .as_deref()
            .ok_or_else(|| "请先选择一条游戏录制".to_string())?;
        if allow_target_mismatch && inner.pending_mismatch_id.as_deref() != Some(id) {
            return Err("当前没有等待确认的目标不匹配回放".into());
        }
        let summary = inner
            .index
            .recordings
            .iter()
            .find(|recording| recording.id == id)
            .ok_or_else(|| "当前选择的游戏录制不存在".to_string())?;
        (
            load_recording(&recording_path(&inner.storage_dir, id)?, id)?,
            summary.playback.clone(),
        )
    };
    if recording.events.is_empty() {
        return Err("当前游戏录制没有可回放的操作".into());
    }
    claim_activity(app)?;
    let state = app.state::<GameRecorder>();
    let result = {
        let mut inner = state.lock();
        if inner.state.activity != GameRecorderActivity::Idle {
            Err("已有游戏录制或回放任务正在进行".to_string())
        } else if inner.index.active_recording_id.as_deref() != Some(recording.id.as_str()) {
            Err("准备回放时选择的录制已发生变化，请重试".to_string())
        } else {
            inner.token = inner.token.wrapping_add(1);
            inner.state.activity = GameRecorderActivity::PlaybackCountdown;
            inner.state.countdown_remaining = COUNTDOWN_SECONDS;
            inner.state.completed_loops = 0;
            inner.state.target_mismatch = false;
            inner.pending_mismatch_id = None;
            inner.state.last_error = None;
            Ok((inner.token, inner.state.clone()))
        }
    };
    let (token, snapshot) = match result {
        Ok(value) => value,
        Err(error) => {
            release_activity(app);
            return Err(error);
        }
    };
    emit_state(app, &snapshot);
    app.state::<AppState>().log(app, "3 秒后开始游戏操作回放");
    let app_handle = app.clone();
    thread::spawn(move || {
        playback_countdown(
            app_handle,
            token,
            recording,
            playback,
            allow_target_mismatch,
        )
    });
    Ok(snapshot)
}

pub(crate) fn stop_game_activity_internal(app: &AppHandle) -> GameRecorderState {
    stop_game_activity_with_source(app, None)
}

pub(crate) fn stop_game_activity_from_hotkey(
    app: &AppHandle,
    accelerator: &str,
) -> GameRecorderState {
    stop_game_activity_with_source(app, Some(accelerator.to_string()))
}

pub(crate) fn hotkeys(app: &AppHandle) -> GameRecorderHotkeys {
    app.state::<GameRecorder>().lock().state.hotkeys.clone()
}

fn recording_countdown(app: AppHandle, token: u64) {
    if !countdown(&app, token, GameRecorderActivity::RecordingCountdown) {
        return;
    }
    let target = match foreground_target() {
        Ok(target) => target,
        Err(error) => {
            abort_pending_activity(
                &app,
                token,
                GameRecorderActivity::RecordingCountdown,
                format!("无法开始游戏录制：{error}"),
            );
            return;
        }
    };
    if is_current_process_target(&target) {
        abort_pending_activity(
            &app,
            token,
            GameRecorderActivity::RecordingCountdown,
            "无法开始游戏录制：倒计时结束时前台仍是本应用，请在倒计时内切回游戏".into(),
        );
        return;
    }
    if !raw_input::is_listener_alive() {
        abort_pending_activity(
            &app,
            token,
            GameRecorderActivity::RecordingCountdown,
            "无法开始游戏录制：Raw Input 监听器已停止".into(),
        );
        return;
    }
    let snapshot = {
        let state = app.state::<GameRecorder>();
        let mut inner = state.lock();
        if inner.token != token || inner.state.activity != GameRecorderActivity::RecordingCountdown
        {
            return;
        }
        raw_input::begin_capture();
        inner.recording = Some(RecordingSession::new(target.clone()));
        inner.state.activity = GameRecorderActivity::Recording;
        inner.state.countdown_remaining = 0;
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    app.state::<AppState>().log(
        &app,
        format!("开始录制游戏操作：{}", display_target(&target)),
    );
    monitor_recording_limit(app, token);
}

fn monitor_recording_limit(app: AppHandle, token: u64) {
    loop {
        thread::sleep(Duration::from_millis(100));
        let should_finish = {
            let state = app.state::<GameRecorder>();
            let inner = state.lock();
            if inner.token != token || inner.state.activity != GameRecorderActivity::Recording {
                return;
            }
            if !raw_input::is_listener_alive() {
                drop(inner);
                abort_recording(&app, token, "Raw Input 监听器已停止，本次录制已丢弃");
                return;
            }
            if raw_input::has_dropped_events() {
                drop(inner);
                abort_recording(
                    &app,
                    token,
                    "输入速度过快，录制事件队列溢出；本次不完整录制已丢弃",
                );
                return;
            }
            inner
                .recording
                .as_ref()
                .is_some_and(|session| session.elapsed_ms() >= MAX_RECORDING_DURATION_MS)
        };
        if should_finish {
            finish_recording_after_drain(&app, token, None, Some("录制已达到 10 分钟上限"));
            return;
        }
    }
}

fn playback_countdown(
    app: AppHandle,
    token: u64,
    recording: GameRecordingFile,
    playback: GamePlaybackSettings,
    allow_target_mismatch: bool,
) {
    if !countdown(&app, token, GameRecorderActivity::PlaybackCountdown) {
        return;
    }
    let current_target = match foreground_target() {
        Ok(target) => target,
        Err(error) => {
            abort_pending_activity(
                &app,
                token,
                GameRecorderActivity::PlaybackCountdown,
                format!("无法开始游戏回放：{error}"),
            );
            return;
        }
    };
    if is_current_process_target(&current_target) {
        abort_pending_activity(
            &app,
            token,
            GameRecorderActivity::PlaybackCountdown,
            "无法开始游戏回放：倒计时结束时前台仍是本应用，请在倒计时内切回游戏".into(),
        );
        return;
    }
    if !allow_target_mismatch && !target_matches(&recording.target, &current_target) {
        let snapshot = {
            let state = app.state::<GameRecorder>();
            let mut inner = state.lock();
            if inner.token != token
                || inner.state.activity != GameRecorderActivity::PlaybackCountdown
            {
                return;
            }
            inner.token = inner.token.wrapping_add(1);
            inner.state.activity = GameRecorderActivity::Idle;
            inner.state.countdown_remaining = 0;
            inner.state.target_mismatch = true;
            inner.pending_mismatch_id = Some(recording.id.clone());
            inner.state.clone()
        };
        release_activity(&app);
        emit_state(&app, &snapshot);
        crate::desktop::show_main_window(&app);
        app.state::<AppState>().log(
            &app,
            format!(
                "已取消回放：当前前台程序 {} 与录制目标 {} 不匹配",
                display_target(&current_target),
                display_target(&recording.target)
            ),
        );
        return;
    }

    let snapshot = {
        let state = app.state::<GameRecorder>();
        let mut inner = state.lock();
        if inner.token != token || inner.state.activity != GameRecorderActivity::PlaybackCountdown {
            return;
        }
        inner.state.activity = GameRecorderActivity::Playing;
        inner.state.countdown_remaining = 0;
        inner.state.target_mismatch = false;
        inner.pending_mismatch_id = None;
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    run_playback(
        app,
        token,
        recording.events,
        recording.duration_ms,
        playback,
    );
}

fn run_playback(
    app: AppHandle,
    token: u64,
    events: Vec<GameRecordedEvent>,
    duration_ms: u64,
    playback: GamePlaybackSettings,
) {
    loop {
        let loop_started = Instant::now();
        for event in &events {
            let target = scaled_event_delay(event.at_ms(), playback.speed);
            if !wait_until(&app, token, loop_started, target) {
                return;
            }
            if let Err(error) = play_event(&app, token, event) {
                finish_playback(&app, token, Some(format!("游戏操作回放失败：{error}")));
                return;
            }
        }
        if !wait_until(
            &app,
            token,
            loop_started,
            scaled_event_delay(duration_ms, playback.speed),
        ) {
            return;
        }
        if !release_playback_pressed(&app, token) {
            finish_playback(
                &app,
                token,
                Some("游戏操作回放停止：部分按键或鼠标按钮释放失败".into()),
            );
            return;
        }

        let completed = {
            let state = app.state::<GameRecorder>();
            let mut inner = state.lock();
            if inner.token != token || inner.state.activity != GameRecorderActivity::Playing {
                return;
            }
            inner.state.completed_loops = inner.state.completed_loops.saturating_add(1);
            let completed = inner.state.completed_loops;
            let snapshot = inner.state.clone();
            drop(inner);
            emit_state(&app, &snapshot);
            completed
        };
        if playback.loop_mode == LoopMode::Count && completed >= playback.loop_count.max(1) {
            finish_playback(&app, token, None);
            return;
        }
        if !wait_game_duration(
            &app,
            token,
            Duration::from_secs_f64(playback.loop_interval_seconds),
            GameRecorderActivity::Playing,
        ) {
            return;
        }
    }
}

fn release_playback_pressed(app: &AppHandle, token: u64) -> bool {
    let state = app.state::<GameRecorder>();
    let mut inner = state.lock();
    if inner.token != token || inner.state.activity != GameRecorderActivity::Playing {
        return false;
    }
    release_tracked_inputs(&mut inner)
}

fn release_tracked_inputs(inner: &mut GameRecorderRuntime) -> bool {
    let keys = inner.playback_keys.drain().collect::<Vec<_>>();
    let buttons = inner.playback_buttons.drain().collect::<Vec<_>>();
    let released = input::release_game_inputs(&keys, &buttons);
    if !released {
        inner.playback_keys.extend(keys);
        inner.playback_buttons.extend(buttons);
    }
    released
}

fn retry_pending_release_before_start(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<GameRecorder>();
    let outcome = {
        let mut inner = state.lock();
        if !inner.pending_release {
            return Ok(());
        }
        let released = release_tracked_inputs(&mut inner);
        if released {
            inner.pending_release = false;
            inner.token = inner.token.wrapping_add(1);
            inner.state.activity = GameRecorderActivity::Idle;
            inner.state.last_error = None;
            Ok(inner.state.clone())
        } else {
            let error = "仍有按键或鼠标按钮未能释放，请再次使用停止或紧急停止热键";
            inner.state.activity = GameRecorderActivity::Playing;
            inner.state.last_error = Some(error.into());
            Err((error.to_string(), inner.state.clone()))
        }
    };
    match outcome {
        Ok(snapshot) => {
            release_activity(app);
            emit_state(app, &snapshot);
            Ok(())
        }
        Err((error, snapshot)) => {
            app.state::<AppState>().lock().game_activity = true;
            emit_state(app, &snapshot);
            Err(error)
        }
    }
}

fn play_event(app: &AppHandle, token: u64, event: &GameRecordedEvent) -> Result<(), String> {
    // Keep the activity check, SendInput call, and pressed-state update in one critical section.
    // stop_game_activity takes the same lock, so once stop returns no playback thread can inject
    // one final movement/key in the gap between a stale token check and SendInput.
    let state = app.state::<GameRecorder>();
    let mut inner = state.lock();
    if inner.token != token || inner.state.activity != GameRecorderActivity::Playing {
        return Err("回放已停止".into());
    }
    match event {
        GameRecordedEvent::MouseMove { dx, dy, .. } => input::game_mouse_move(*dx, *dy),
        GameRecordedEvent::MouseWheel { delta, .. } => input::game_mouse_wheel(*delta),
        GameRecordedEvent::MouseButton {
            button, pressed, ..
        } => {
            input::game_mouse_button(*button, *pressed)?;
            if *pressed {
                inner.playback_buttons.insert(*button);
            } else {
                inner.playback_buttons.remove(button);
            }
            Ok(())
        }
        GameRecordedEvent::Key {
            scan_code,
            extended,
            pressed,
            ..
        } => {
            input::game_key(*scan_code, *extended, *pressed)?;
            let identity = (*scan_code, *extended);
            if *pressed {
                inner.playback_keys.insert(identity);
            } else {
                inner.playback_keys.remove(&identity);
            }
            Ok(())
        }
    }
}

fn finish_playback(app: &AppHandle, token: u64, error: Option<String>) {
    let (snapshot, released, final_message) = {
        let state = app.state::<GameRecorder>();
        let mut inner = state.lock();
        if inner.token != token || inner.state.activity != GameRecorderActivity::Playing {
            return;
        }
        let released = release_tracked_inputs(&mut inner);
        inner.token = inner.token.wrapping_add(1);
        inner.pending_release = !released;
        inner.state.activity = if released {
            GameRecorderActivity::Idle
        } else {
            GameRecorderActivity::Playing
        };
        inner.state.countdown_remaining = 0;
        let final_error = if released {
            error.clone()
        } else {
            Some(
                error
                    .as_deref()
                    .map(|error| format!("{error}；部分输入释放失败"))
                    .unwrap_or_else(|| "部分按键或鼠标按钮释放失败".into()),
            )
        };
        inner.state.last_error = final_error.clone();
        let final_message = final_error.unwrap_or_else(|| "游戏操作回放完成".into());
        (inner.state.clone(), released, final_message)
    };
    if released {
        release_activity(app);
    } else {
        app.state::<AppState>().lock().game_activity = true;
    }
    emit_state(app, &snapshot);
    app.state::<AppState>().log(app, final_message);
}

fn stop_game_activity_with_source(
    app: &AppHandle,
    stop_hotkey: Option<String>,
) -> GameRecorderState {
    let state = app.state::<GameRecorder>();
    let (activity, token, idle_result) = {
        let mut inner = state.lock();
        let activity = inner.state.activity;
        let token = inner.token;
        let idle_result = if activity == GameRecorderActivity::Idle {
            let owned_activity = inner.pending_release;
            let released = release_tracked_inputs(&mut inner);
            let changed = inner.state.target_mismatch || !released;
            inner.state.target_mismatch = false;
            inner.pending_mismatch_id = None;
            inner.pending_release = !released;
            if released && owned_activity {
                inner.state.last_error = None;
            } else if !released {
                inner.state.last_error = Some("部分按键或鼠标按钮释放失败，请再次紧急停止".into());
                inner.state.activity = GameRecorderActivity::Playing;
            }
            Some((inner.state.clone(), changed, released, owned_activity))
        } else {
            None
        };
        (activity, token, idle_result)
    };
    if let Some((snapshot, changed, released, owned_activity)) = idle_result {
        let _ = raw_input::end_capture();
        if released && owned_activity {
            release_activity(app);
        } else {
            if !released {
                app.state::<AppState>().lock().game_activity = true;
            }
        }
        if changed {
            emit_state(app, &snapshot);
        }
        return snapshot;
    }
    if activity == GameRecorderActivity::Recording {
        return finish_recording_after_drain(app, token, stop_hotkey, Some("已停止游戏操作录制"));
    }
    let _ = raw_input::end_capture();

    let (snapshot, released, was_pending_release) = {
        let mut inner = state.lock();
        let was_pending_release = inner.pending_release;
        let released = release_tracked_inputs(&mut inner);
        inner.token = inner.token.wrapping_add(1);
        inner.recording = None;
        inner.pending_release = !released;
        inner.state.activity = if released {
            GameRecorderActivity::Idle
        } else {
            GameRecorderActivity::Playing
        };
        inner.state.countdown_remaining = 0;
        inner.state.target_mismatch = false;
        if released {
            inner.state.last_error = None;
        } else {
            inner.state.last_error = Some("部分按键或鼠标按钮释放失败，请再次紧急停止".into());
        }
        (inner.state.clone(), released, was_pending_release)
    };
    if released {
        release_activity(app);
    } else {
        app.state::<AppState>().lock().game_activity = true;
    }
    emit_state(app, &snapshot);
    app.state::<AppState>().log(
        app,
        if released {
            if was_pending_release {
                "残留的按键和鼠标按钮已释放"
            } else {
                "已停止游戏录制或回放任务"
            }
        } else {
            "游戏任务已停止，但仍有输入未释放；请再次紧急停止"
        },
    );
    snapshot
}

fn finish_recording(
    app: &AppHandle,
    token: u64,
    stop_hotkey: Option<String>,
    message: Option<&str>,
    ended_at: Option<Instant>,
) -> GameRecorderState {
    let ended_at = ended_at.unwrap_or_else(|| raw_input::end_capture().captured_at);
    let state = app.state::<GameRecorder>();
    let (session, storage_dir) = {
        let mut inner = state.lock();
        if inner.token != token || inner.state.activity != GameRecorderActivity::Recording {
            return inner.state.clone();
        }
        let Some(session) = inner.recording.take() else {
            return inner.state.clone();
        };
        inner.token = inner.token.wrapping_add(1);
        inner.state.activity = GameRecorderActivity::Idle;
        inner.state.countdown_remaining = 0;
        (session, inner.storage_dir.clone())
    };
    let (events, duration_ms, target) = session.finish(stop_hotkey.as_deref(), Some(ended_at));
    let now = now_millis();
    let id = create_id();
    let name = Local::now()
        .format("游戏录制 %Y-%m-%d %H-%M-%S")
        .to_string();
    let recording = GameRecordingFile {
        schema_version: SCHEMA_VERSION,
        id: id.clone(),
        name: name.clone(),
        target: target.clone(),
        created_at: now,
        updated_at: now,
        duration_ms,
        events,
    };
    let recording_file_path = recording_path(&storage_dir, &id);
    let save_result = match &recording_file_path {
        Ok(path) => save_recording(path, &recording),
        Err(error) => Err(error.clone()),
    };
    let snapshot = {
        let mut inner = state.lock();
        match save_result {
            Ok(()) => {
                let previous_index = inner.index.clone();
                let summary = summary_from_recording(&recording, default_playback());
                inner.index.recordings.insert(0, summary);
                inner.index.active_recording_id = Some(id.clone());
                inner.state.last_error = None;
                sync_public_state(&mut inner);
                if let Err(error) = persist_index(&inner) {
                    inner.index = previous_index;
                    sync_public_state(&mut inner);
                    let cleanup_error = recording_file_path
                        .as_ref()
                        .ok()
                        .and_then(|path| fs::remove_file(path).err());
                    inner.state.last_error = Some(match cleanup_error {
                        Some(cleanup_error) => {
                            format!("{error}；同时清理未写入索引的录制文件失败：{cleanup_error}")
                        }
                        None => error,
                    });
                }
            }
            Err(error) => inner.state.last_error = Some(error),
        }
        inner.state.clone()
    };
    release_activity(app);
    emit_state(app, &snapshot);
    app.state::<AppState>()
        .log(app, message.unwrap_or("游戏操作录制已保存"));
    snapshot
}

fn finish_recording_after_drain(
    app: &AppHandle,
    token: u64,
    stop_hotkey: Option<String>,
    message: Option<&str>,
) -> GameRecorderState {
    let cutoff = raw_input::end_capture();
    if !raw_input::wait_until_drained(&cutoff, Duration::from_millis(250)) {
        return abort_recording(
            app,
            token,
            "停止录制时输入队列未能及时排空；为避免保存不完整轨迹，本次录制已丢弃",
        );
    }
    if raw_input::has_dropped_events() {
        return abort_recording(
            app,
            token,
            "输入速度过快，录制事件队列溢出；本次不完整录制已丢弃",
        );
    }
    finish_recording(app, token, stop_hotkey, message, Some(cutoff.captured_at))
}

fn abort_recording(app: &AppHandle, token: u64, error: &str) -> GameRecorderState {
    let _ = raw_input::end_capture();
    let snapshot = {
        let state = app.state::<GameRecorder>();
        let mut inner = state.lock();
        if inner.token != token || inner.state.activity != GameRecorderActivity::Recording {
            return inner.state.clone();
        }
        inner.token = inner.token.wrapping_add(1);
        inner.recording = None;
        inner.state.activity = GameRecorderActivity::Idle;
        inner.state.countdown_remaining = 0;
        inner.state.last_error = Some(error.into());
        inner.state.clone()
    };
    release_activity(app);
    emit_state(app, &snapshot);
    app.state::<AppState>().log(app, error);
    snapshot
}

fn abort_pending_activity(
    app: &AppHandle,
    token: u64,
    activity: GameRecorderActivity,
    error: String,
) {
    let _ = raw_input::end_capture();
    let snapshot = {
        let state = app.state::<GameRecorder>();
        let mut inner = state.lock();
        if inner.token != token || inner.state.activity != activity {
            return;
        }
        inner.token = inner.token.wrapping_add(1);
        inner.state.activity = GameRecorderActivity::Idle;
        inner.state.countdown_remaining = 0;
        inner.state.last_error = Some(error.clone());
        inner.state.clone()
    };
    release_activity(app);
    emit_state(app, &snapshot);
    app.state::<AppState>().log(app, error);
}

fn handle_raw_input(app: &AppHandle, event: RawInputEvent) {
    if raw_input::has_dropped_events() {
        let token = app.state::<GameRecorder>().lock().token;
        abort_recording(
            app,
            token,
            "输入速度过快，录制事件队列溢出；本次不完整录制已丢弃",
        );
        return;
    }
    let stop_reason = {
        let state = app.state::<GameRecorder>();
        let mut inner = state.lock();
        if inner.state.activity != GameRecorderActivity::Recording {
            return;
        }
        inner
            .recording
            .as_mut()
            .and_then(|session| session.record(event))
    };
    if let Some(stop_reason) = stop_reason {
        let token = app.state::<GameRecorder>().lock().token;
        let cutoff = raw_input::end_capture();
        if raw_input::has_dropped_events() {
            abort_recording(
                app,
                token,
                "输入速度过快，录制事件队列溢出；本次不完整录制已丢弃",
            );
            return;
        }
        finish_recording(
            app,
            token,
            None,
            Some(match stop_reason {
                RecordingStopReason::Duration => "录制已达到 10 分钟上限",
                RecordingStopReason::Capacity => "录制已达到 50,000 个事件上限",
            }),
            Some(cutoff.captured_at),
        );
    }
}

fn countdown(app: &AppHandle, token: u64, activity: GameRecorderActivity) -> bool {
    for remaining in (1..=COUNTDOWN_SECONDS).rev() {
        let snapshot = {
            let state = app.state::<GameRecorder>();
            let mut inner = state.lock();
            if inner.token != token || inner.state.activity != activity {
                return false;
            }
            inner.state.countdown_remaining = remaining;
            inner.state.clone()
        };
        emit_state(app, &snapshot);
        if !wait_game_duration(app, token, Duration::from_secs(1), activity) {
            return false;
        }
    }
    true
}

fn wait_until(app: &AppHandle, token: u64, started: Instant, target: Duration) -> bool {
    loop {
        if !is_current(app, token, GameRecorderActivity::Playing) {
            return false;
        }
        let elapsed = started.elapsed();
        if elapsed >= target {
            return true;
        }
        thread::sleep((target - elapsed).min(Duration::from_millis(5)));
    }
}

fn wait_game_duration(
    app: &AppHandle,
    token: u64,
    duration: Duration,
    activity: GameRecorderActivity,
) -> bool {
    let started = Instant::now();
    loop {
        if !is_current(app, token, activity) {
            return false;
        }
        let elapsed = started.elapsed();
        if elapsed >= duration {
            return true;
        }
        thread::sleep((duration - elapsed).min(Duration::from_millis(20)));
    }
}

fn is_current(app: &AppHandle, token: u64, activity: GameRecorderActivity) -> bool {
    let state = app.state::<GameRecorder>();
    let inner = state.lock();
    inner.token == token && inner.state.activity == activity
}

fn scaled_event_delay(at_ms: u64, speed: f64) -> Duration {
    Duration::from_secs_f64(at_ms as f64 / 1000.0 / sanitize_speed(speed))
}

fn claim_activity(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut inner = state.lock();
    if inner.state.is_running || inner.state.is_recording || inner.game_activity {
        Err("已有宏录制、宏执行或游戏录制任务正在进行".into())
    } else {
        inner.game_activity = true;
        inner.is_capturing_key = false;
        Ok(())
    }
}

fn claim_temporary_activity(app: &AppHandle) -> Result<TemporaryActivityClaim, String> {
    claim_activity(app)?;
    Ok(TemporaryActivityClaim { app: app.clone() })
}

fn release_activity(app: &AppHandle) {
    app.state::<AppState>().lock().game_activity = false;
}

fn ensure_idle(inner: &GameRecorderRuntime) -> Result<(), String> {
    if inner.state.activity == GameRecorderActivity::Idle {
        Ok(())
    } else {
        Err("录制或回放进行中，当前操作不可用".into())
    }
}

fn ensure_global_idle(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let inner = state.lock();
    if inner.state.is_running || inner.state.is_recording || inner.game_activity {
        Err("宏录制、宏执行或游戏任务进行中，当前配置操作不可用".into())
    } else {
        Ok(())
    }
}

fn emit_state(app: &AppHandle, state: &GameRecorderState) {
    let _ = app.emit("game-recorder-state", state);
}

fn set_last_error(app: &AppHandle, error: String) {
    let snapshot = {
        let state = app.state::<GameRecorder>();
        let mut inner = state.lock();
        inner.state.last_error = Some(error);
        inner.state.clone()
    };
    emit_state(app, &snapshot);
}

fn state_from_index(index: &GameRecorderIndex) -> GameRecorderState {
    GameRecorderState {
        recordings: index.recordings.clone(),
        active_recording_id: index.active_recording_id.clone(),
        hotkeys: index.hotkeys.clone(),
        activity: GameRecorderActivity::Idle,
        countdown_remaining: 0,
        completed_loops: 0,
        target_mismatch: false,
        hotkey_errors: Vec::new(),
        last_error: None,
    }
}

fn sync_public_state(inner: &mut GameRecorderRuntime) {
    inner.state.recordings = inner.index.recordings.clone();
    inner.state.active_recording_id = inner.index.active_recording_id.clone();
    inner.state.hotkeys = inner.index.hotkeys.clone();
}

fn default_hotkeys() -> GameRecorderHotkeys {
    GameRecorderHotkeys {
        record_start: "CommandOrControl+Alt+R".into(),
        stop: "CommandOrControl+Alt+S".into(),
        playback_start: "CommandOrControl+Alt+L".into(),
    }
}

fn default_playback() -> GamePlaybackSettings {
    GamePlaybackSettings {
        speed: 1.0,
        loop_mode: LoopMode::Count,
        loop_count: 1,
        loop_interval_seconds: 1.0,
    }
}

fn sanitize_playback(mut settings: GamePlaybackSettings) -> GamePlaybackSettings {
    settings.speed = sanitize_speed(settings.speed);
    settings.loop_count = settings.loop_count.clamp(1, 9999);
    settings.loop_interval_seconds = if settings.loop_interval_seconds.is_finite() {
        settings.loop_interval_seconds.clamp(0.0, 3600.0)
    } else {
        1.0
    };
    settings
}

fn sanitize_speed(speed: f64) -> f64 {
    if !speed.is_finite() {
        return 1.0;
    }
    [0.5, 1.0, 1.5, 2.0]
        .into_iter()
        .min_by(|left, right| (speed - left).abs().total_cmp(&(speed - right).abs()))
        .unwrap_or(1.0)
}

fn validate_game_hotkeys(game_hotkeys: &GameRecorderHotkeys) -> Vec<String> {
    let entries = [
        ("开始游戏录制", game_hotkeys.record_start.as_str()),
        ("停止游戏任务", game_hotkeys.stop.as_str()),
        ("开始游戏回放", game_hotkeys.playback_start.as_str()),
    ];
    let mut errors = Vec::new();
    let mut seen = Vec::<(String, &str)>::new();
    for (label, accelerator) in entries {
        if accelerator.is_empty() {
            errors.push(format!("热键不能为空：{label}"));
            continue;
        }
        let normalized = canonical_hotkey(accelerator);
        if normalized == canonical_hotkey(EMERGENCY_STOP_HOTKEY) {
            errors.push(format!("热键 {accelerator} 已保留为紧急停止"));
        } else if let Some((_, previous)) = seen.iter().find(|(key, _)| key == &normalized) {
            errors.push(format!(
                "热键重复：{previous} 与 {label} 都使用 {accelerator}"
            ));
        } else {
            seen.push((normalized, label));
        }
    }
    if hotkey_scan_codes(&game_hotkeys.stop).is_none() {
        errors.push("停止游戏任务热键包含当前录制器无法安全裁剪的按键".into());
    }
    errors
}

fn canonical_hotkey(accelerator: &str) -> String {
    let mut parts = accelerator
        .split('+')
        .map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "commandorcontrol" | "ctrl" => "control".to_string(),
            "escape" => "esc".to_string(),
            "return" => "enter".to_string(),
            "spacebar" => "space".to_string(),
            value => value.to_string(),
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts.sort();
    parts.join("+")
}

fn trim_hotkey_tail(events: &mut Vec<GameRecordedEvent>, hotkey: &str, duration_ms: u64) {
    let Some(identities) = hotkey_scan_codes(hotkey) else {
        return;
    };
    let cutoff = duration_ms.saturating_sub(750);

    if let Some(mut remove_indices) = completed_hotkey_tail(events, &identities, cutoff) {
        remove_indices.sort_unstable();
        remove_indices.dedup();
        for index in remove_indices.into_iter().rev() {
            events.remove(index);
        }
    }

    if let Some(mut remove_indices) = incomplete_main_hotkey_tail(events, &identities, cutoff) {
        remove_indices.sort_unstable();
        remove_indices.dedup();
        for index in remove_indices.into_iter().rev() {
            events.remove(index);
        }
    }

    if let Some(remove_indices) = modifier_only_hotkey_tail(events, &identities, cutoff) {
        for index in remove_indices.into_iter().rev() {
            events.remove(index);
        }
    }

    // The global shortcut callback can run before Raw Input receives the main key or key-up
    // messages. Remove only chord keys that remain unmatched at the end of the capture.
    let (pressed_keys, _) = pressed_state(events);
    let main_anchor = pressed_keys
        .intersection(&identities.main)
        .filter_map(|identity| {
            events.iter().rposition(|event| {
                matches!(
                    event,
                    GameRecordedEvent::Key {
                        scan_code,
                        extended,
                        pressed: true,
                        ..
                    } if (*scan_code, *extended) == *identity
                )
            })
        })
        .max();
    let mut remove_indices = pressed_keys
        .intersection(&identities.all)
        .filter_map(|identity| {
            let down_index = events.iter().rposition(|event| {
                matches!(
                    event,
                    GameRecordedEvent::Key {
                        at_ms,
                        scan_code,
                        extended,
                        pressed: true,
                    } if *at_ms >= cutoff && (*scan_code, *extended) == *identity
                )
            })?;
            if identities.main.contains(identity)
                || modifier_down_belongs_to_tail(events, &identities, down_index, main_anchor)
            {
                Some(down_index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    remove_indices.sort_unstable();
    for index in remove_indices.into_iter().rev() {
        events.remove(index);
    }
}

fn modifier_down_belongs_to_tail(
    events: &[GameRecordedEvent],
    identities: &HotkeyScanCodes,
    down_index: usize,
    main_anchor: Option<usize>,
) -> bool {
    let end = main_anchor
        .filter(|anchor| *anchor > down_index)
        .unwrap_or(events.len());
    !events
        .iter()
        .take(end)
        .skip(down_index + 1)
        .any(|event| match event {
            GameRecordedEvent::Key {
                scan_code,
                extended,
                pressed: true,
                ..
            } => !identities
                .modifier_groups
                .iter()
                .any(|group| group.contains(&(*scan_code, *extended))),
            GameRecordedEvent::MouseMove { .. } => false,
            _ => true,
        })
}

fn completed_hotkey_tail(
    events: &[GameRecordedEvent],
    identities: &HotkeyScanCodes,
    cutoff: u64,
) -> Option<Vec<usize>> {
    struct Candidate {
        main_identity: (u16, bool),
        main_down: usize,
        modifiers: Vec<((u16, bool), usize)>,
    }

    let mut pressed = HashMap::<(u16, bool), usize>::new();
    let mut candidates = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let GameRecordedEvent::Key {
            at_ms,
            scan_code,
            extended,
            pressed: is_pressed,
        } = event
        else {
            continue;
        };
        let identity = (*scan_code, *extended);
        if *is_pressed {
            pressed.insert(identity, index);
            if *at_ms >= cutoff && identities.main.contains(&identity) {
                let modifiers = identities
                    .modifier_groups
                    .iter()
                    .map(|group| {
                        group
                            .iter()
                            .filter_map(|identity| {
                                pressed.get(identity).map(|index| (*identity, *index))
                            })
                            .max_by_key(|(_, index)| *index)
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(modifiers) = modifiers {
                    candidates.push(Candidate {
                        main_identity: identity,
                        main_down: index,
                        modifiers,
                    });
                }
            }
        } else {
            pressed.remove(&identity);
        }
    }

    candidates.into_iter().rev().find_map(|candidate| {
        let has_later_non_chord_key =
            events
                .iter()
                .skip(candidate.main_down + 1)
                .any(|event| match event {
                    GameRecordedEvent::Key {
                        scan_code,
                        extended,
                        ..
                    } => !identities.all.contains(&(*scan_code, *extended)),
                    _ => false,
                });
        if has_later_non_chord_key {
            return None;
        }
        let main_up = matching_key_up(events, candidate.main_down, candidate.main_identity)?;
        let mut remove = vec![candidate.main_down, main_up];
        for (identity, down_index) in candidate.modifiers {
            let down_is_tail_chord = events[down_index].at_ms() >= cutoff
                && !events
                    .iter()
                    .take(candidate.main_down)
                    .skip(down_index + 1)
                    .any(|event| match event {
                        GameRecordedEvent::Key {
                            scan_code,
                            extended,
                            pressed: true,
                            ..
                        } => !identities
                            .modifier_groups
                            .iter()
                            .any(|group| group.contains(&(*scan_code, *extended))),
                        GameRecordedEvent::MouseMove { .. } => false,
                        _ => true,
                    });
            if down_is_tail_chord {
                remove.push(down_index);
                if let Some(up_index) = matching_key_up(events, candidate.main_down, identity) {
                    remove.push(up_index);
                }
            }
        }
        Some(remove)
    })
}

fn modifier_only_hotkey_tail(
    events: &[GameRecordedEvent],
    identities: &HotkeyScanCodes,
    cutoff: u64,
) -> Option<Vec<usize>> {
    if identities.modifier_groups.is_empty() {
        return None;
    }
    let suffix_start = events
        .iter()
        .rposition(|event| match event {
            GameRecordedEvent::Key {
                scan_code,
                extended,
                ..
            } => {
                let identity = (*scan_code, *extended);
                identities.main.contains(&identity)
                    || !identities
                        .modifier_groups
                        .iter()
                        .any(|group| group.contains(&identity))
            }
            GameRecordedEvent::MouseMove { .. } => false,
            GameRecordedEvent::MouseButton { .. } | GameRecordedEvent::MouseWheel { .. } => true,
        })
        .map_or(0, |index| index + 1);
    let suffix = &events[suffix_start..];
    if suffix.is_empty() {
        return None;
    }

    let (mut pressed, _) = pressed_state(&events[..suffix_start]);
    pressed.retain(|identity| {
        identities
            .modifier_groups
            .iter()
            .any(|group| group.contains(identity))
    });
    let mut seen_groups = identities
        .modifier_groups
        .iter()
        .map(|group| !group.is_disjoint(&pressed))
        .collect::<Vec<_>>();
    let mut remove_indices = Vec::new();
    let mut first_states = HashMap::<(u16, bool), (bool, u64)>::new();
    for (offset, event) in suffix.iter().enumerate() {
        let GameRecordedEvent::Key {
            scan_code,
            extended,
            pressed: is_pressed,
            ..
        } = event
        else {
            if matches!(event, GameRecordedEvent::MouseMove { .. }) {
                continue;
            }
            return None;
        };
        let identity = (*scan_code, *extended);
        let group_index = identities
            .modifier_groups
            .iter()
            .position(|group| group.contains(&identity))?;
        let changed = if *is_pressed {
            pressed.insert(identity)
        } else {
            pressed.remove(&identity)
        };
        if !changed {
            return None;
        }
        first_states
            .entry(identity)
            .or_insert((*is_pressed, event.at_ms()));
        seen_groups[group_index] = true;
        remove_indices.push(suffix_start + offset);
    }
    if !seen_groups.into_iter().all(|seen| seen) {
        return None;
    }
    remove_indices.retain(|index| {
        let GameRecordedEvent::Key {
            scan_code,
            extended,
            ..
        } = &events[*index]
        else {
            return false;
        };
        first_states
            .get(&(*scan_code, *extended))
            .is_some_and(|(pressed, at_ms)| *pressed && *at_ms >= cutoff)
    });
    (!remove_indices.is_empty()).then_some(remove_indices)
}

fn incomplete_main_hotkey_tail(
    events: &[GameRecordedEvent],
    identities: &HotkeyScanCodes,
    cutoff: u64,
) -> Option<Vec<usize>> {
    let (pressed_keys, _) = pressed_state(events);
    let main_down = pressed_keys
        .intersection(&identities.main)
        .filter_map(|identity| {
            events.iter().rposition(|event| {
                matches!(
                    event,
                    GameRecordedEvent::Key {
                        at_ms,
                        scan_code,
                        extended,
                        pressed: true,
                    } if *at_ms >= cutoff && (*scan_code, *extended) == *identity
                )
            })
        })
        .max()?;

    let mut active = HashSet::new();
    for event in events.iter().take(main_down + 1) {
        if let GameRecordedEvent::Key {
            scan_code,
            extended,
            pressed,
            ..
        } = event
        {
            let identity = (*scan_code, *extended);
            if *pressed {
                active.insert(identity);
            } else {
                active.remove(&identity);
            }
        }
    }
    if identities
        .modifier_groups
        .iter()
        .any(|group| group.is_disjoint(&active))
    {
        return None;
    }

    let start = events[..main_down]
        .iter()
        .rposition(|event| match event {
            GameRecordedEvent::Key {
                scan_code,
                extended,
                ..
            } => !identities
                .modifier_groups
                .iter()
                .any(|group| group.contains(&(*scan_code, *extended))),
            GameRecordedEvent::MouseMove { .. } => false,
            GameRecordedEvent::MouseButton { .. } | GameRecordedEvent::MouseWheel { .. } => true,
        })
        .map_or(0, |index| index + 1);
    if events.iter().skip(main_down + 1).any(|event| match event {
        GameRecordedEvent::Key {
            scan_code,
            extended,
            ..
        } => !identities
            .modifier_groups
            .iter()
            .any(|group| group.contains(&(*scan_code, *extended))),
        GameRecordedEvent::MouseMove { .. } => false,
        GameRecordedEvent::MouseButton { .. } | GameRecordedEvent::MouseWheel { .. } => true,
    }) {
        return None;
    }

    let mut first_modifier_state = HashMap::<(u16, bool), (bool, u64)>::new();
    for event in events.iter().skip(start) {
        if let GameRecordedEvent::Key {
            scan_code,
            extended,
            pressed,
            ..
        } = event
        {
            let identity = (*scan_code, *extended);
            if identities
                .modifier_groups
                .iter()
                .any(|group| group.contains(&identity))
            {
                first_modifier_state
                    .entry(identity)
                    .or_insert((*pressed, event.at_ms()));
            }
        }
    }
    let mut remove = vec![main_down];
    for (index, event) in events.iter().enumerate().skip(start) {
        if let GameRecordedEvent::Key {
            scan_code,
            extended,
            ..
        } = event
        {
            let identity = (*scan_code, *extended);
            if first_modifier_state
                .get(&identity)
                .is_some_and(|(pressed, at_ms)| *pressed && *at_ms >= cutoff)
            {
                remove.push(index);
            }
        }
    }
    Some(remove)
}

fn matching_key_up(
    events: &[GameRecordedEvent],
    after_index: usize,
    identity: (u16, bool),
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after_index + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameRecordedEvent::Key {
                    scan_code,
                    extended,
                    pressed: false,
                    ..
                } if (*scan_code, *extended) == identity
            )
            .then_some(index)
        })
}

struct HotkeyScanCodes {
    all: HashSet<(u16, bool)>,
    main: HashSet<(u16, bool)>,
    modifier_groups: Vec<HashSet<(u16, bool)>>,
}

fn hotkey_scan_codes(hotkey: &str) -> Option<HotkeyScanCodes> {
    let mut all = HashSet::new();
    let mut main = HashSet::new();
    let mut modifier_groups = Vec::new();
    for part in hotkey.split('+') {
        let lower = part.trim().to_ascii_lowercase();
        let modifier_codes: Option<&[(u16, bool)]> = match lower.as_str() {
            "commandorcontrol" | "control" | "ctrl" => Some(&[(0x1d, false), (0x1d, true)]),
            "controlleft" | "ctrlleft" => Some(&[(0x1d, false)]),
            "controlright" | "ctrlright" => Some(&[(0x1d, true)]),
            "alt" => Some(&[(0x38, false), (0x38, true)]),
            "altleft" => Some(&[(0x38, false)]),
            "altright" => Some(&[(0x38, true)]),
            "shift" => Some(&[(0x2a, false), (0x36, false)]),
            "shiftleft" => Some(&[(0x2a, false)]),
            "shiftright" => Some(&[(0x36, false)]),
            _ => None,
        };
        if let Some(codes) = modifier_codes {
            all.extend(codes.iter().copied());
            modifier_groups.push(codes.iter().copied().collect());
            continue;
        }
        if !main.is_empty() {
            return None;
        }
        let codes = main_key_scan_codes(&lower)?;
        all.extend(codes.iter().copied());
        main.extend(codes);
    }
    (!main.is_empty()).then_some(HotkeyScanCodes {
        all,
        main,
        modifier_groups,
    })
}

fn main_key_scan_codes(key: &str) -> Option<Vec<(u16, bool)>> {
    let virtual_key = match key {
        "esc" | "escape" => 0x1b,
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" => 0x0d,
        "space" => 0x20,
        "home" => 0x24,
        "arrowup" => 0x26,
        "pageup" => 0x21,
        "arrowleft" => 0x25,
        "arrowright" => 0x27,
        "end" => 0x23,
        "arrowdown" => 0x28,
        "pagedown" => 0x22,
        "insert" => 0x2d,
        "delete" => 0x2e,
        value if value.len() == 1 => character_virtual_key(value.as_bytes()[0])?,
        value if value.starts_with('f') => {
            let number = value[1..].parse::<u16>().ok()?;
            if !(1..=12).contains(&number) {
                return None;
            }
            0x70 + number - 1
        }
        _ => return None,
    };
    Some(vec![scan_code_for_virtual_key(virtual_key)?])
}

fn character_virtual_key(key: u8) -> Option<u16> {
    let key = key.to_ascii_uppercase();
    let virtual_key = match key {
        b'0'..=b'9' | b'A'..=b'Z' => key as u16,
        b';' => 0xba,
        b'=' => 0xbb,
        b',' => 0xbc,
        b'-' => 0xbd,
        b'.' => 0xbe,
        b'/' => 0xbf,
        b'`' => 0xc0,
        b'[' => 0xdb,
        b'\\' => 0xdc,
        b']' => 0xdd,
        b'\'' => 0xde,
        _ => return None,
    };
    Some(virtual_key)
}

#[cfg(target_os = "windows")]
fn scan_code_for_virtual_key(virtual_key: u16) -> Option<(u16, bool)> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VK_TO_VSC_EX, MapVirtualKeyW};

    let mapped = unsafe { MapVirtualKeyW(virtual_key as u32, MAPVK_VK_TO_VSC_EX) };
    let scan_code = (mapped & 0xff) as u16;
    let prefix = (mapped >> 8) & 0xff;
    (scan_code != 0 && prefix != 0xe1).then_some((scan_code, prefix == 0xe0))
}

#[cfg(not(target_os = "windows"))]
fn scan_code_for_virtual_key(virtual_key: u16) -> Option<(u16, bool)> {
    let identity = match virtual_key {
        0x1b => (0x01, false),
        0x08 => (0x0e, false),
        0x09 => (0x0f, false),
        0x0d => (0x1c, false),
        0x20 => (0x39, false),
        0x24 => (0x47, true),
        0x26 => (0x48, true),
        0x21 => (0x49, true),
        0x25 => (0x4b, true),
        0x27 => (0x4d, true),
        0x23 => (0x4f, true),
        0x28 => (0x50, true),
        0x22 => (0x51, true),
        0x2d => (0x52, true),
        0x2e => (0x53, true),
        0x31..=0x39 => (0x02 + virtual_key - 0x31, false),
        0x30 => (0x0b, false),
        0x51 => (0x10, false),
        0x57 => (0x11, false),
        0x45 => (0x12, false),
        0x52 => (0x13, false),
        0x54 => (0x14, false),
        0x59 => (0x15, false),
        0x55 => (0x16, false),
        0x49 => (0x17, false),
        0x4f => (0x18, false),
        0x50 => (0x19, false),
        0x41 => (0x1e, false),
        0x53 => (0x1f, false),
        0x44 => (0x20, false),
        0x46 => (0x21, false),
        0x47 => (0x22, false),
        0x48 => (0x23, false),
        0x4a => (0x24, false),
        0x4b => (0x25, false),
        0x4c => (0x26, false),
        0x5a => (0x2c, false),
        0x58 => (0x2d, false),
        0x43 => (0x2e, false),
        0x56 => (0x2f, false),
        0x42 => (0x30, false),
        0x4e => (0x31, false),
        0x4d => (0x32, false),
        0xbd => (0x0c, false),
        0xbb => (0x0d, false),
        0xdb => (0x1a, false),
        0xdd => (0x1b, false),
        0xba => (0x27, false),
        0xde => (0x28, false),
        0xc0 => (0x29, false),
        0xdc => (0x2b, false),
        0xbc => (0x33, false),
        0xbe => (0x34, false),
        0xbf => (0x35, false),
        0x70..=0x79 => (0x3b + virtual_key - 0x70, false),
        0x7a => (0x57, false),
        0x7b => (0x58, false),
        _ => return None,
    };
    Some(identity)
}

fn pressed_state(events: &[GameRecordedEvent]) -> (HashSet<(u16, bool)>, HashSet<RawMouseButton>) {
    let mut keys = HashSet::new();
    let mut buttons = HashSet::new();
    for event in events {
        match event {
            GameRecordedEvent::Key {
                scan_code,
                extended,
                pressed,
                ..
            } => {
                let identity = (*scan_code, *extended);
                if *pressed {
                    keys.insert(identity);
                } else {
                    keys.remove(&identity);
                }
            }
            GameRecordedEvent::MouseButton {
                button, pressed, ..
            } => {
                if *pressed {
                    buttons.insert(*button);
                } else {
                    buttons.remove(button);
                }
            }
            _ => {}
        }
    }
    (keys, buttons)
}

fn summary_from_recording(
    recording: &GameRecordingFile,
    playback: GamePlaybackSettings,
) -> GameRecordingSummary {
    let keyboard_event_count = recording
        .events
        .iter()
        .filter(|event| event.is_keyboard())
        .count();
    GameRecordingSummary {
        id: recording.id.clone(),
        name: recording.name.clone(),
        duration_ms: recording.duration_ms,
        event_count: recording.events.len(),
        keyboard_event_count,
        mouse_event_count: recording.events.len() - keyboard_event_count,
        target: recording.target.clone(),
        created_at: recording.created_at,
        updated_at: recording.updated_at,
        playback,
    }
}

fn sanitize_recording_name(value: &str, fallback: &str) -> String {
    let value = truncate_chars(value.trim(), 50);
    if value.is_empty() {
        fallback.into()
    } else {
        value
    }
}

fn load_index(storage_dir: &Path) -> (GameRecorderIndex, Vec<String>) {
    let path = storage_dir.join(INDEX_FILE_NAME);
    if !path.exists() {
        return (GameRecorderIndex::default(), Vec::new());
    }
    match fs::read_to_string(&path)
        .map_err(|error| error.to_string())
        .and_then(|contents| serde_json::from_str(&contents).map_err(|error| error.to_string()))
    {
        Ok(index) => (index, Vec::new()),
        Err(error) => (
            GameRecorderIndex::default(),
            vec![format!("游戏录制索引读取失败，已使用空录制库：{error}")],
        ),
    }
}

fn sanitize_index(index: &mut GameRecorderIndex, storage_dir: &Path) {
    index.schema_version = SCHEMA_VERSION;
    index.hotkeys = GameRecorderHotkeys {
        record_start: normalize_or_default(
            &index.hotkeys.record_start,
            &default_hotkeys().record_start,
        ),
        stop: normalize_or_default(&index.hotkeys.stop, &default_hotkeys().stop),
        playback_start: normalize_or_default(
            &index.hotkeys.playback_start,
            &default_hotkeys().playback_start,
        ),
    };
    if hotkey_scan_codes(&index.hotkeys.stop).is_none() {
        index.hotkeys.stop = default_hotkeys().stop;
    }
    index.recordings.retain_mut(|summary| {
        summary.name = sanitize_recording_name(&summary.name, "游戏录制");
        summary.playback = sanitize_playback(summary.playback.clone());
        recording_path(storage_dir, &summary.id)
            .ok()
            .is_some_and(|path| path.is_file())
    });
    if index
        .active_recording_id
        .as_ref()
        .is_none_or(|id| !index.recordings.iter().any(|recording| &recording.id == id))
    {
        index.active_recording_id = index
            .recordings
            .first()
            .map(|recording| recording.id.clone());
    }
}

fn normalize_or_default(value: &str, default: &str) -> String {
    let value = normalize_hotkey(value);
    if value.is_empty() {
        default.into()
    } else {
        value
    }
}

fn recording_path(storage_dir: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("游戏录制 ID 无效".into());
    }
    Ok(storage_dir.join(format!("{id}.json")))
}

fn persist_index(inner: &GameRecorderRuntime) -> Result<(), String> {
    fs::create_dir_all(&inner.storage_dir)
        .map_err(|error| format!("创建游戏录制目录失败：{error}"))?;
    write_json(
        &inner.storage_dir.join(INDEX_FILE_NAME),
        &inner.index,
        "游戏录制索引",
    )
}

fn save_recording(path: &Path, recording: &GameRecordingFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建游戏录制目录失败：{error}"))?;
    }
    write_json(path, recording, "游戏录制")
}

fn load_recording(path: &Path, expected_id: &str) -> Result<GameRecordingFile, String> {
    let file_size = fs::metadata(path)
        .map_err(|error| format!("读取游戏录制信息失败：{error}"))?
        .len();
    if file_size > MAX_RECORDING_FILE_BYTES {
        return Err("游戏录制文件过大，已拒绝加载".into());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("读取游戏录制失败：{error}"))?;
    let mut recording: GameRecordingFile =
        serde_json::from_str(&contents).map_err(|error| format!("解析游戏录制失败：{error}"))?;
    if recording.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "不支持的游戏录制版本：{}",
            recording.schema_version
        ));
    }
    if recording.duration_ms == 0 {
        recording.duration_ms = recording
            .events
            .last()
            .map(GameRecordedEvent::at_ms)
            .unwrap_or(0);
    }
    validate_loaded_recording(&recording, expected_id)?;
    Ok(recording)
}

fn validate_loaded_recording(
    recording: &GameRecordingFile,
    expected_id: &str,
) -> Result<(), String> {
    if recording.id != expected_id || recording_path(Path::new("."), &recording.id).is_err() {
        return Err("游戏录制文件 ID 与索引不一致".into());
    }
    if recording.events.len() > MAX_STORED_EVENTS {
        return Err(format!(
            "游戏录制事件数量超过安全上限（{} > {MAX_STORED_EVENTS}）",
            recording.events.len()
        ));
    }
    if recording.duration_ms > MAX_RECORDING_DURATION_MS {
        return Err("游戏录制时长超过 10 分钟上限".into());
    }

    let mut previous_at = 0;
    let mut pressed_keys = HashSet::new();
    let mut pressed_buttons = HashSet::new();
    for (index, event) in recording.events.iter().enumerate() {
        let at_ms = event.at_ms();
        if at_ms > MAX_RECORDING_DURATION_MS {
            return Err(format!("游戏录制第 {} 个事件超过 10 分钟上限", index + 1));
        }
        if index > 0 && at_ms < previous_at {
            return Err(format!("游戏录制第 {} 个事件时间倒退", index + 1));
        }
        previous_at = at_ms;
        match event {
            GameRecordedEvent::Key {
                scan_code,
                extended,
                pressed,
                ..
            } => {
                if *scan_code == 0 {
                    return Err(format!("游戏录制第 {} 个键盘事件扫描码无效", index + 1));
                }
                let identity = (*scan_code, *extended);
                let changed = if *pressed {
                    pressed_keys.insert(identity)
                } else {
                    pressed_keys.remove(&identity)
                };
                if !changed {
                    return Err(format!(
                        "游戏录制第 {} 个键盘事件状态不完整（重复按下或孤立抬起）",
                        index + 1
                    ));
                }
            }
            GameRecordedEvent::MouseButton {
                button, pressed, ..
            } => {
                let changed = if *pressed {
                    pressed_buttons.insert(*button)
                } else {
                    pressed_buttons.remove(button)
                };
                if !changed {
                    return Err(format!(
                        "游戏录制第 {} 个鼠标按钮事件状态不完整（重复按下或孤立抬起）",
                        index + 1
                    ));
                }
            }
            GameRecordedEvent::MouseMove { .. } | GameRecordedEvent::MouseWheel { .. } => {}
        }
    }
    if !pressed_keys.is_empty() || !pressed_buttons.is_empty() {
        return Err("游戏录制包含未闭合的按键或鼠标按钮".into());
    }
    if previous_at > recording.duration_ms {
        return Err("游戏录制时长早于最后一个事件".into());
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize, label: &str) -> Result<(), String> {
    let mut contents =
        serde_json::to_string(value).map_err(|error| format!("序列化{label}失败：{error}"))?;
    contents.push('\n');
    let parent = path
        .parent()
        .ok_or_else(|| format!("保存{label}失败：目标路径没有父目录"))?;
    fs::create_dir_all(parent).map_err(|error| format!("保存{label}失败：{error}"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("game-recording.json");
    let temporary_path = parent.join(format!(".{file_name}.{}.tmp", create_id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| format!("创建{label}临时文件失败：{error}"))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| format!("写入{label}临时文件失败：{error}"))?;
        file.flush()
            .map_err(|error| format!("刷新{label}临时文件失败：{error}"))?;
        file.sync_all()
            .map_err(|error| format!("同步{label}临时文件失败：{error}"))?;
        drop(file);
        replace_file(&temporary_path, path).map_err(|error| format!("替换{label}文件失败：{error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

fn target_matches(recorded: &GameTarget, current: &GameTarget) -> bool {
    if !recorded.process_name.is_empty() && !current.process_name.is_empty() {
        recorded
            .process_name
            .eq_ignore_ascii_case(&current.process_name)
    } else {
        !recorded.window_title.is_empty()
            && recorded
                .window_title
                .eq_ignore_ascii_case(&current.window_title)
    }
}

fn display_target(target: &GameTarget) -> String {
    match (
        target.process_name.is_empty(),
        target.window_title.is_empty(),
    ) {
        (false, false) => format!("{}（{}）", target.process_name, target.window_title),
        (false, true) => target.process_name.clone(),
        (true, false) => target.window_title.clone(),
        (true, true) => "未知前台程序".into(),
    }
}

fn is_current_process_target(target: &GameTarget) -> bool {
    let current_name = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    });
    current_name.is_some_and(|name| name.eq_ignore_ascii_case(&target.process_name))
}

#[cfg(target_os = "windows")]
fn foreground_target() -> Result<GameTarget, String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId},
    };

    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return Err("无法获取当前前台窗口".into());
    }
    let mut title_buffer = vec![0u16; 1024];
    let title_length =
        unsafe { GetWindowTextW(window, title_buffer.as_mut_ptr(), title_buffer.len() as i32) };
    let window_title = String::from_utf16_lossy(&title_buffer[..title_length.max(0) as usize]);

    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(window, &mut process_id);
    }
    if process_id == 0 {
        return Err("无法确定前台窗口所属进程".into());
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return Err(format!(
            "无法打开前台进程：{}",
            std::io::Error::last_os_error()
        ));
    }
    let mut path_buffer = vec![0u16; 32_768];
    let mut path_length = path_buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(process, 0, path_buffer.as_mut_ptr(), &mut path_length)
    };
    let query_error = (queried == 0).then(std::io::Error::last_os_error);
    unsafe {
        CloseHandle(process);
    }
    if let Some(error) = query_error {
        return Err(format!("无法读取前台进程路径：{error}"));
    }
    let process_path = String::from_utf16_lossy(&path_buffer[..path_length as usize]);
    let process_name = Path::new(&process_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "前台进程名称为空".to_string())?
        .to_string();
    Ok(GameTarget {
        process_name,
        window_title,
    })
}

#[cfg(not(target_os = "windows"))]
fn foreground_target() -> Result<GameTarget, String> {
    Err("游戏操作录制仅支持 Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_is_accumulated_in_fixed_sixteen_millisecond_buckets() {
        let mut session = RecordingSession::new(GameTarget::default());
        assert!(
            session
                .record_at(RawInputKind::MouseMove { dx: 2, dy: 3 }, 0)
                .is_none()
        );
        assert!(
            session
                .record_at(RawInputKind::MouseMove { dx: 4, dy: -1 }, 10)
                .is_none()
        );
        assert!(
            session
                .record_at(RawInputKind::MouseMove { dx: 8, dy: 2 }, 17)
                .is_none()
        );
        session.flush_move();
        assert_eq!(
            session.events,
            vec![
                GameRecordedEvent::MouseMove {
                    at_ms: 0,
                    dx: 6,
                    dy: 2,
                },
                GameRecordedEvent::MouseMove {
                    at_ms: 17,
                    dx: 8,
                    dy: 2,
                },
            ]
        );
    }

    #[test]
    fn button_events_flush_movement_and_repeated_keys_are_ignored() {
        let mut session = RecordingSession::new(GameTarget::default());
        session.record_at(RawInputKind::MouseMove { dx: 5, dy: 6 }, 0);
        session.record_at(
            RawInputKind::Key {
                scan_code: 0x11,
                extended: false,
                pressed: true,
            },
            5,
        );
        session.record_at(
            RawInputKind::Key {
                scan_code: 0x11,
                extended: false,
                pressed: true,
            },
            6,
        );
        session.record_at(
            RawInputKind::Key {
                scan_code: 0x12,
                extended: false,
                pressed: false,
            },
            7,
        );
        assert_eq!(session.events.len(), 2);
        assert!(matches!(
            session.events[0],
            GameRecordedEvent::MouseMove { .. }
        ));
        assert!(matches!(
            session.events[1],
            GameRecordedEvent::Key { pressed: true, .. }
        ));
    }

    #[test]
    fn finishing_completes_pressed_inputs_and_trims_the_stop_hotkey() {
        let mut session = RecordingSession::new(GameTarget::default());
        session.record_at(
            RawInputKind::Key {
                scan_code: 0x11,
                extended: false,
                pressed: true,
            },
            10,
        );
        session.record_at(
            RawInputKind::Key {
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            20,
        );
        session.record_at(
            RawInputKind::Key {
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            21,
        );
        session.record_at(
            RawInputKind::Key {
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            22,
        );
        let ended_at = session.started_at + Duration::from_millis(100);
        let (events, _, _) = session.finish(Some("Control+Alt+S"), Some(ended_at));
        assert!(!events.iter().any(|event| matches!(
            event,
            GameRecordedEvent::Key {
                scan_code: 0x1d | 0x38 | 0x1f,
                ..
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            GameRecordedEvent::Key {
                scan_code: 0x11,
                pressed: false,
                ..
            }
        )));
    }

    #[test]
    fn playback_settings_are_clamped_to_supported_values() {
        let sanitized = sanitize_playback(GamePlaybackSettings {
            speed: 1.8,
            loop_mode: LoopMode::Count,
            loop_count: 0,
            loop_interval_seconds: f64::INFINITY,
        });
        assert_eq!(sanitized.speed, 2.0);
        assert_eq!(sanitized.loop_count, 1);
        assert_eq!(sanitized.loop_interval_seconds, 1.0);
        assert_eq!(scaled_event_delay(1_000, 2.0), Duration::from_millis(500));
    }

    #[test]
    fn target_matching_prefers_the_process_name() {
        let recorded = GameTarget {
            process_name: "game.exe".into(),
            window_title: "Old title".into(),
        };
        assert!(target_matches(
            &recorded,
            &GameTarget {
                process_name: "GAME.EXE".into(),
                window_title: "New title".into(),
            }
        ));
        assert!(!target_matches(
            &recorded,
            &GameTarget {
                process_name: "other.exe".into(),
                window_title: "Old title".into(),
            }
        ));
    }

    #[test]
    fn game_hotkeys_allow_macro_shortcuts_but_reject_reserved_shortcuts() {
        let macro_hotkeys = crate::model::default_settings().hotkeys;
        let mut game = default_hotkeys();
        game.record_start = macro_hotkeys.start.clone();
        game.stop = EMERGENCY_STOP_HOTKEY.into();
        let errors = validate_game_hotkeys(&game);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("紧急停止"));
    }

    #[test]
    fn public_activity_and_event_models_use_the_frontend_contract() {
        assert_eq!(
            serde_json::to_value(GameRecorderActivity::RecordingCountdown)
                .expect("serialize activity"),
            serde_json::json!("recordingCountdown")
        );
        assert_eq!(
            serde_json::to_value(GameRecordedEvent::Key {
                at_ms: 25,
                scan_code: 0x1e,
                extended: false,
                pressed: true,
            })
            .expect("serialize event"),
            serde_json::json!({
                "type": "key",
                "atMs": 25,
                "scanCode": 0x1e,
                "extended": false,
                "pressed": true,
            })
        );
    }

    #[test]
    fn raw_events_captured_before_the_session_are_discarded() {
        let mut session = RecordingSession::new(GameTarget::default());
        let captured_at = session
            .started_at
            .checked_sub(Duration::from_millis(1))
            .expect("instant supports a short subtraction");
        assert!(
            session
                .record(RawInputEvent {
                    captured_at,
                    kind: RawInputKind::MouseMove { dx: 40, dy: 20 },
                })
                .is_none()
        );
        assert!(session.events.is_empty());
        assert!(session.pending_move.is_none());
    }

    #[test]
    fn capacity_stop_keeps_safety_releases_even_past_the_capture_limit() {
        let mut session = RecordingSession::new(GameTarget::default());
        session.events = vec![
            GameRecordedEvent::MouseWheel {
                at_ms: 1,
                delta: 120,
            };
            MAX_RECORDED_EVENTS - 1
        ];
        session.events.push(GameRecordedEvent::Key {
            at_ms: 2,
            scan_code: 0x11,
            extended: false,
            pressed: true,
        });
        let ended_at = session.started_at + Duration::from_millis(10);
        let (events, _, _) = session.finish(None, Some(ended_at));
        assert_eq!(events.len(), MAX_RECORDED_EVENTS + 1);
        assert!(matches!(
            events.last(),
            Some(GameRecordedEvent::Key {
                scan_code: 0x11,
                pressed: false,
                ..
            })
        ));
    }

    #[test]
    fn recording_ids_cannot_escape_the_storage_directory() {
        let directory = Path::new("recordings");
        assert!(recording_path(directory, "safe-id_123").is_ok());
        assert!(recording_path(directory, "../profiles").is_err());
        assert!(recording_path(directory, "a/b").is_err());
        assert!(recording_path(directory, "C:\\temp").is_err());
    }

    #[test]
    fn unsupported_stop_hotkeys_do_not_partially_trim_modifiers() {
        let mut events = vec![GameRecordedEvent::Key {
            at_ms: 100,
            scan_code: 0x1d,
            extended: false,
            pressed: true,
        }];
        trim_hotkey_tail(&mut events, "Control+Hyper+S", 100);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn stop_trimming_preserves_a_completed_main_key_tap() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 120,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 200,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 210,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 220);
        assert_eq!(
            events,
            vec![
                GameRecordedEvent::Key {
                    at_ms: 100,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 120,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: false,
                },
            ]
        );
    }

    #[test]
    fn stop_trimming_removes_only_the_final_completed_chord() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 120,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::MouseMove {
                at_ms: 805,
                dx: 3,
                dy: -2,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 830,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 840,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 850,
                scan_code: 0x1d,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 900);
        assert_eq!(
            events,
            vec![
                GameRecordedEvent::Key {
                    at_ms: 100,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 120,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: false,
                },
                GameRecordedEvent::MouseMove {
                    at_ms: 805,
                    dx: 3,
                    dy: -2,
                },
            ]
        );
    }

    #[test]
    fn stop_trimming_keeps_a_modifier_that_was_already_held_for_gameplay() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 830,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 840,
                scan_code: 0x1d,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 900);
        assert_eq!(
            events,
            vec![
                GameRecordedEvent::Key {
                    at_ms: 100,
                    scan_code: 0x1d,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 840,
                    scan_code: 0x1d,
                    extended: false,
                    pressed: false,
                },
            ]
        );
    }

    #[test]
    fn stop_trimming_preserves_modifier_for_an_earlier_shortcut() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 700,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 720,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 730,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 830,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 840,
                scan_code: 0x1d,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 900);
        assert_eq!(
            events,
            vec![
                GameRecordedEvent::Key {
                    at_ms: 700,
                    scan_code: 0x1d,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 720,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 730,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: false,
                },
                GameRecordedEvent::Key {
                    at_ms: 840,
                    scan_code: 0x1d,
                    extended: false,
                    pressed: false,
                },
            ]
        );
    }

    #[test]
    fn eof_stop_trimming_preserves_modifier_used_by_an_earlier_shortcut() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 700,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 720,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 730,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::MouseMove {
                at_ms: 805,
                dx: 1,
                dy: 2,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 820);
        assert_eq!(events.len(), 4);
        assert!(matches!(
            events[0],
            GameRecordedEvent::Key {
                scan_code: 0x1d,
                pressed: true,
                ..
            }
        ));
        assert!(matches!(
            events[1],
            GameRecordedEvent::Key {
                scan_code: 0x1f,
                pressed: true,
                ..
            }
        ));
        assert!(matches!(
            events[2],
            GameRecordedEvent::Key {
                scan_code: 0x1f,
                pressed: false,
                ..
            }
        ));
        assert!(matches!(events[3], GameRecordedEvent::MouseMove { .. }));
    }

    #[test]
    fn stop_trimming_removes_released_modifiers_when_main_raw_event_is_missing() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 120,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::MouseMove {
                at_ms: 805,
                dx: 2,
                dy: 1,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 830,
                scan_code: 0x1d,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 900);
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0],
            GameRecordedEvent::Key {
                scan_code: 0x1f,
                pressed: true,
                ..
            }
        ));
        assert!(matches!(
            events[1],
            GameRecordedEvent::Key {
                scan_code: 0x1f,
                pressed: false,
                ..
            }
        ));
        assert!(matches!(events[2], GameRecordedEvent::MouseMove { .. }));
    }

    #[test]
    fn stop_trimming_removes_partial_modifier_release_without_main_event() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 830);
        assert!(events.is_empty());
    }

    #[test]
    fn stop_trimming_removes_partial_release_after_unmatched_main_down() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 825,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 830);
        assert!(events.is_empty());
    }

    #[test]
    fn unmatched_main_preserves_a_modifier_held_before_the_tail_window() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 810,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 900);
        assert_eq!(
            events,
            vec![GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            }]
        );
    }

    #[test]
    fn missing_main_trims_new_modifier_but_preserves_long_held_modifier() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 900);
        assert_eq!(
            events,
            vec![GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            }]
        );
    }

    #[test]
    fn missing_main_preserves_long_modifier_released_after_a_gameplay_boundary() {
        let mut events = vec![
            GameRecordedEvent::Key {
                at_ms: 100,
                scan_code: 0x1d,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 200,
                scan_code: 0x1f,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 210,
                scan_code: 0x1f,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 800,
                scan_code: 0x38,
                extended: false,
                pressed: true,
            },
            GameRecordedEvent::Key {
                at_ms: 820,
                scan_code: 0x38,
                extended: false,
                pressed: false,
            },
            GameRecordedEvent::Key {
                at_ms: 830,
                scan_code: 0x1d,
                extended: false,
                pressed: false,
            },
        ];
        trim_hotkey_tail(&mut events, "Control+Alt+S", 900);
        assert_eq!(
            events,
            vec![
                GameRecordedEvent::Key {
                    at_ms: 100,
                    scan_code: 0x1d,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 200,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 210,
                    scan_code: 0x1f,
                    extended: false,
                    pressed: false,
                },
                GameRecordedEvent::Key {
                    at_ms: 830,
                    scan_code: 0x1d,
                    extended: false,
                    pressed: false,
                },
            ]
        );
    }

    #[test]
    fn canonical_hotkeys_fold_reserved_key_aliases() {
        assert_eq!(
            canonical_hotkey("Control+Alt+Escape"),
            canonical_hotkey("Alt+Ctrl+Esc")
        );
        let mut game = default_hotkeys();
        game.stop = "Control+Alt+Escape".into();
        assert!(
            validate_game_hotkeys(&game)
                .iter()
                .any(|error| error.contains("紧急停止"))
        );
    }

    #[test]
    fn loaded_recordings_reject_unsafe_timelines() {
        let base = GameRecordingFile {
            schema_version: SCHEMA_VERSION,
            id: "recording-1".into(),
            name: "测试".into(),
            target: GameTarget::default(),
            created_at: 1,
            updated_at: 1,
            duration_ms: 100,
            events: vec![
                GameRecordedEvent::Key {
                    at_ms: 10,
                    scan_code: 0x1e,
                    extended: false,
                    pressed: true,
                },
                GameRecordedEvent::Key {
                    at_ms: 20,
                    scan_code: 0x1e,
                    extended: false,
                    pressed: false,
                },
            ],
        };
        assert!(validate_loaded_recording(&base, "recording-1").is_ok());
        assert!(validate_loaded_recording(&base, "different-id").is_err());

        let mut invalid = base.clone();
        invalid.events[1] = GameRecordedEvent::Key {
            at_ms: 5,
            scan_code: 0x1e,
            extended: false,
            pressed: false,
        };
        assert!(validate_loaded_recording(&invalid, "recording-1").is_err());

        let mut invalid = base.clone();
        invalid.events[0] = GameRecordedEvent::Key {
            at_ms: 10,
            scan_code: 0,
            extended: false,
            pressed: true,
        };
        assert!(validate_loaded_recording(&invalid, "recording-1").is_err());

        let mut invalid = base.clone();
        invalid.events.insert(1, invalid.events[0].clone());
        assert!(validate_loaded_recording(&invalid, "recording-1").is_err());

        let mut invalid = base.clone();
        invalid.events[1] = GameRecordedEvent::Key {
            at_ms: MAX_RECORDING_DURATION_MS + 1,
            scan_code: 0x1e,
            extended: false,
            pressed: false,
        };
        assert!(validate_loaded_recording(&invalid, "recording-1").is_err());

        let oversized = GameRecordingFile {
            events: vec![
                GameRecordedEvent::MouseWheel {
                    at_ms: 0,
                    delta: 120,
                };
                MAX_STORED_EVENTS + 1
            ],
            ..base
        };
        assert!(validate_loaded_recording(&oversized, "recording-1").is_err());
    }

    #[test]
    fn invalid_persisted_stop_hotkey_falls_back_to_default() {
        let mut index = GameRecorderIndex::default();
        index.hotkeys.stop = "Control+Hyper+S".into();
        sanitize_index(&mut index, Path::new("unused"));
        assert_eq!(index.hotkeys.stop, default_hotkeys().stop);
    }
}

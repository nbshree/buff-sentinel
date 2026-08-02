mod audio;
mod capture;
mod detector;
mod model;
mod storage;
mod timeline;
mod windows;

use std::{
    collections::VecDeque,
    io::Cursor,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use audio::{AudioCue, AudioEngine};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use capture::{
    CapturePurpose, CapturedImage, RuntimeCaptureControl, RuntimeCaptureFlags, capture_snapshot,
    start_runtime_capture,
};
use image::{DynamicImage, GrayImage, ImageEncoder, Luma, RgbaImage, codecs::jpeg::JpegEncoder};
pub use model::{
    BuffAssistantActivity, BuffAssistantConfig, BuffAssistantSettings, BuffAssistantState,
    BuffOverlayMode, BuffOverlayState, BuffTarget, CapturePreview, CaptureWindowCandidate,
    NormalizedRect, SampleFrameSummary,
};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
};
use timeline::{BuffTimeline, TimelineAction, TimelinePhase};

const SAMPLE_RETENTION_MS: i64 = 120_000;
const SAMPLE_MEMORY_LIMIT: usize = 64 * 1024 * 1024;
const MONITOR_FRAME_TIMEOUT: Duration = Duration::from_secs(3);
const OVERLAY_LABEL: &str = "buff-overlay";

struct StoredSample {
    id: u64,
    captured_at_unix_ms: i64,
    width: u32,
    height: u32,
    png: Vec<u8>,
    thumbnail_data_url: String,
}

struct RuntimeData {
    config: BuffAssistantConfig,
    activity: BuffAssistantActivity,
    monitor_requested: bool,
    expected_at_unix_ms: Option<i64>,
    last_confidence: f32,
    last_error: Option<String>,
    storage_directory: PathBuf,
    capture: Option<RuntimeCaptureControl>,
    capture_purpose: Option<CapturePurpose>,
    timeline: BuffTimeline,
    samples: VecDeque<StoredSample>,
    sample_bytes: usize,
    next_sample_id: u64,
    sample_target: Option<BuffTarget>,
    sample_region: Option<NormalizedRect>,
    last_frame_at: Option<Instant>,
    reconnect_generation: u64,
    overlay_generation: u64,
    overlay_editing: bool,
}

pub struct BuffAssistant {
    inner: Mutex<RuntimeData>,
    audio: AudioEngine,
}

impl BuffAssistant {
    pub fn load(app: &AppHandle) -> Result<(Self, Vec<String>), String> {
        let directory = storage::storage_directory(app)?;
        let (config, mut notices) = storage::load_config(&directory);
        let (audio, audio_warning) = AudioEngine::start();
        if let Some(warning) = audio_warning {
            notices.push(warning);
        }
        Ok((
            Self {
                inner: Mutex::new(RuntimeData {
                    timeline: BuffTimeline::new(config.settings.cycle_ms),
                    config,
                    activity: BuffAssistantActivity::Stopped,
                    monitor_requested: false,
                    expected_at_unix_ms: None,
                    last_confidence: 0.0,
                    last_error: None,
                    storage_directory: directory,
                    capture: None,
                    capture_purpose: None,
                    samples: VecDeque::new(),
                    sample_bytes: 0,
                    next_sample_id: 1,
                    sample_target: None,
                    sample_region: None,
                    last_frame_at: None,
                    reconnect_generation: 0,
                    overlay_generation: 0,
                    overlay_editing: false,
                }),
                audio,
            },
            notices,
        ))
    }

    fn lock(&self) -> MutexGuard<'_, RuntimeData> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn snapshot(&self) -> BuffAssistantState {
        snapshot_from_runtime(&self.lock())
    }
}

pub fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(OVERLAY_LABEL).is_some() {
        return Ok(());
    }
    let config = app.state::<BuffAssistant>().lock().config.clone();
    let overlay = WebviewWindowBuilder::new(
        app,
        OVERLAY_LABEL,
        WebviewUrl::App("index.html?window=buff-overlay".into()),
    )
    .title("金周天提醒")
    .inner_size(330.0, 92.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focusable(false)
    .visible(false)
    .build()?;
    overlay.set_position(PhysicalPosition::new(
        config.settings.overlay.x,
        config.settings.overlay.y,
    ))?;
    overlay.set_ignore_cursor_events(true)?;
    Ok(())
}

#[tauri::command]
pub fn get_buff_assistant_state(state: State<'_, BuffAssistant>) -> BuffAssistantState {
    state.snapshot()
}

#[tauri::command]
pub fn list_buff_capture_windows() -> Result<Vec<CaptureWindowCandidate>, String> {
    windows::enumerate_candidates()
}

#[tauri::command]
pub fn capture_buff_preview(window_id: String) -> Result<CapturePreview, String> {
    let (window, candidate) = windows::resolve_window(&window_id)?;
    let image = capture_snapshot(window)?;
    Ok(CapturePreview {
        data_url: png_data_url(&image)?,
        width: image.width,
        height: image.height,
        target: BuffTarget {
            reference_width: image.width,
            reference_height: image.height,
            ..windows::target_from_candidate(&candidate)
        },
    })
}

#[tauri::command]
pub fn start_buff_sample_capture(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    window_id: String,
    region: NormalizedRect,
) -> Result<BuffAssistantState, String> {
    let region = region.sanitized();
    let (window, candidate) = windows::resolve_window(&window_id)?;
    let target = windows::target_from_candidate(&candidate);
    stop_buff_monitor_internal(&app);
    {
        let mut inner = state.lock();
        inner.samples.clear();
        inner.sample_bytes = 0;
        inner.sample_target = Some(target.clone());
        inner.sample_region = Some(region);
        inner.activity = BuffAssistantActivity::CapturingSamples;
        inner.monitor_requested = false;
        inner.last_error = None;
        inner.capture_purpose = Some(CapturePurpose::Samples);
    }
    let flags = RuntimeCaptureFlags {
        app: app.clone(),
        purpose: CapturePurpose::Samples,
        region,
        template: None,
        reference_width: target.reference_width,
        reference_height: target.reference_height,
        threshold: 1.0,
        confirm_frames: 1,
        missing_frames: 1,
    };
    match start_runtime_capture(window, flags) {
        Ok(control) => state.lock().capture = Some(control),
        Err(error) => {
            let mut inner = state.lock();
            inner.activity = BuffAssistantActivity::Error;
            inner.capture_purpose = None;
            inner.last_error = Some(error.clone());
            emit_state(&app, &snapshot_from_runtime(&inner));
            return Err(error);
        }
    }
    let snapshot = state.snapshot();
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn pause_buff_sample_capture(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
) -> BuffAssistantState {
    pause_sample_capture_internal(&app);
    state.snapshot()
}

pub fn pause_sample_capture_internal(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let control = {
        let mut inner = state.lock();
        if inner.capture_purpose != Some(CapturePurpose::Samples) {
            return;
        }
        inner.capture_purpose = None;
        inner.activity = BuffAssistantActivity::Stopped;
        inner.capture.take()
    };
    if let Some(control) = control {
        let _ = control.stop();
    }
    emit_state(app, &state.snapshot());
}

#[tauri::command]
pub fn clear_buff_sample_frames(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
) -> BuffAssistantState {
    let snapshot = {
        let mut inner = state.lock();
        inner.samples.clear();
        inner.sample_bytes = 0;
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn list_buff_sample_frames(state: State<'_, BuffAssistant>) -> Vec<SampleFrameSummary> {
    state
        .lock()
        .samples
        .iter()
        .map(|sample| SampleFrameSummary {
            id: sample.id,
            captured_at_unix_ms: sample.captured_at_unix_ms,
            width: sample.width,
            height: sample.height,
            thumbnail_data_url: sample.thumbnail_data_url.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn get_buff_sample_frame(state: State<'_, BuffAssistant>, id: u64) -> Result<String, String> {
    state
        .lock()
        .samples
        .iter()
        .find(|sample| sample.id == id)
        .map(|sample| format!("data:image/png;base64,{}", BASE64.encode(&sample.png)))
        .ok_or_else(|| "找不到采集帧，请重新采集".into())
}

#[tauri::command]
pub fn save_buff_template(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    sample_id: u64,
    crop: NormalizedRect,
    mask_data_url: Option<String>,
) -> Result<BuffAssistantState, String> {
    let (png, target, region, directory) = {
        let inner = state.lock();
        let sample = inner
            .samples
            .iter()
            .find(|sample| sample.id == sample_id)
            .ok_or_else(|| "找不到用于制作模板的采集帧".to_string())?;
        (
            sample.png.clone(),
            inner
                .sample_target
                .clone()
                .ok_or_else(|| "缺少采集窗口信息".to_string())?,
            inner
                .sample_region
                .ok_or_else(|| "缺少 Buff 搜索区域".to_string())?,
            inner.storage_directory.clone(),
        )
    };
    let source =
        image::load_from_memory(&png).map_err(|error| format!("读取采集帧失败：{error}"))?;
    let (x, y, end_x, end_y) = crop.pixel_bounds(source.width(), source.height());
    let width = end_x - x;
    let height = end_y - y;
    if width < 8 || height < 8 {
        return Err("模板区域过小，请重新框选图标".into());
    }
    let template = source.crop_imm(x, y, width, height);
    let mask = decode_mask(mask_data_url.as_deref(), width, height)?;
    let id = format!("jinzhoutian-{}", now_millis());
    let summary = storage::save_template(&directory, &id, &template, &mask)?;
    let snapshot = {
        let mut inner = state.lock();
        inner.config.target = Some(BuffTarget {
            reference_width: target.reference_width,
            reference_height: target.reference_height,
            ..target
        });
        inner.config.search_region = Some(region);
        inner.config.template = Some(summary);
        inner.config.sanitize();
        storage::save_config(&inner.storage_directory, &inner.config)?;
        inner.last_error = None;
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn delete_buff_template(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
) -> Result<BuffAssistantState, String> {
    stop_buff_monitor_internal(&app);
    let snapshot = {
        let mut inner = state.lock();
        if let Some(template) = inner.config.template.take() {
            storage::delete_template(&inner.storage_directory, &template)?;
        }
        inner.config.target = None;
        inner.config.search_region = None;
        storage::save_config(&inner.storage_directory, &inner.config)?;
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn update_buff_assistant_settings(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    mut settings: BuffAssistantSettings,
) -> Result<BuffAssistantState, String> {
    settings.sanitize();
    let was_monitoring = {
        let mut inner = state.lock();
        inner.config.settings = settings;
        storage::save_config(&inner.storage_directory, &inner.config)?;
        inner.monitor_requested
    };
    apply_overlay_position(&app);
    if was_monitoring {
        start_buff_monitor_internal(&app)?;
        return Ok(state.snapshot());
    }
    let snapshot = state.snapshot();
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn start_buff_monitor(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
) -> Result<BuffAssistantState, String> {
    start_buff_monitor_internal(&app)?;
    Ok(state.snapshot())
}

pub fn start_buff_monitor_internal(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<BuffAssistant>();
    stop_current_capture(&state);
    let (target, generation) = {
        let mut inner = state.lock();
        if inner.config.template.is_none()
            || inner.config.target.is_none()
            || inner.config.search_region.is_none()
        {
            return Err("请先完成金周天模板采集".into());
        }
        inner.monitor_requested = true;
        inner.reconnect_generation = inner.reconnect_generation.wrapping_add(1);
        let cycle_ms = inner.config.settings.cycle_ms;
        inner.timeline.start_waiting(cycle_ms);
        inner.activity = BuffAssistantActivity::Waiting;
        inner.expected_at_unix_ms = None;
        inner.last_error = None;
        (
            inner.config.target.clone().unwrap(),
            inner.reconnect_generation,
        )
    };
    match windows::find_target(&target) {
        Ok(Some(window)) => {
            if let Err(error) = attach_monitor_capture(app, window, generation) {
                mark_target_unavailable(app, &error);
                schedule_reconnect(app.clone(), generation);
            }
        }
        Ok(None) => {
            mark_target_unavailable(app, "等待游戏窗口");
            schedule_reconnect(app.clone(), generation);
        }
        Err(error) => {
            mark_target_unavailable(app, &error);
            schedule_reconnect(app.clone(), generation);
        }
    }
    emit_state(app, &state.snapshot());
    Ok(())
}

#[tauri::command]
pub fn stop_buff_monitor(app: AppHandle, state: State<'_, BuffAssistant>) -> BuffAssistantState {
    stop_buff_monitor_internal(&app);
    state.snapshot()
}

pub fn stop_buff_monitor_internal(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let control = {
        let mut inner = state.lock();
        inner.monitor_requested = false;
        inner.reconnect_generation = inner.reconnect_generation.wrapping_add(1);
        inner.capture_purpose = None;
        inner.activity = BuffAssistantActivity::Stopped;
        inner.expected_at_unix_ms = None;
        inner.last_frame_at = None;
        inner.timeline.stop();
        inner.capture.take()
    };
    if let Some(control) = control {
        let _ = control.stop();
    }
    hide_overlay(app);
    emit_state(app, &state.snapshot());
}

#[tauri::command]
pub fn start_buff_template_test(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    window_id: String,
) -> Result<BuffAssistantState, String> {
    let (window, _) = windows::resolve_window(&window_id)?;
    stop_buff_monitor_internal(&app);
    let (flags, config) = capture_flags(&app, CapturePurpose::Test)?;
    let control = start_runtime_capture(window, flags)?;
    let snapshot = {
        let mut inner = state.lock();
        inner.capture = Some(control);
        inner.capture_purpose = Some(CapturePurpose::Test);
        inner.monitor_requested = false;
        inner.activity = BuffAssistantActivity::Testing;
        inner.last_error = None;
        inner.config = config;
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn stop_buff_template_test(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
) -> BuffAssistantState {
    let control = {
        let mut inner = state.lock();
        if inner.capture_purpose != Some(CapturePurpose::Test) {
            return snapshot_from_runtime(&inner);
        }
        inner.capture_purpose = None;
        inner.activity = BuffAssistantActivity::Stopped;
        inner.capture.take()
    };
    if let Some(control) = control {
        let _ = control.stop();
    }
    let snapshot = state.snapshot();
    emit_state(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn play_buff_assistant_sound(
    state: State<'_, BuffAssistant>,
    cue: String,
) -> Result<(), String> {
    let inner = state.lock();
    let audio_cue = match cue.as_str() {
        "triggered" => AudioCue::Triggered,
        "prewarnThree" => AudioCue::PrewarnThree,
        "prewarnTwo" => AudioCue::PrewarnTwo,
        "prewarnOne" => AudioCue::PrewarnOne,
        _ => return Err("未知提示音类型".into()),
    };
    state
        .audio
        .play(audio_cue, inner.config.settings.sound.volume);
    Ok(())
}

#[tauri::command]
pub fn set_buff_overlay_edit_mode(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    enabled: bool,
) -> Result<BuffAssistantState, String> {
    let overlay = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "Buff 悬浮窗口尚未创建".to_string())?;
    if enabled {
        {
            let mut inner = state.lock();
            inner.overlay_editing = true;
            inner.overlay_generation = inner.overlay_generation.wrapping_add(1);
        }
        overlay
            .set_focusable(true)
            .map_err(|error| error.to_string())?;
        overlay
            .set_ignore_cursor_events(false)
            .map_err(|error| error.to_string())?;
        overlay.show().map_err(|error| error.to_string())?;
        emit_overlay(
            &app,
            BuffOverlayState {
                mode: BuffOverlayMode::Editing,
                message: "拖动调整提醒位置".into(),
                expected_at_unix_ms: None,
                emitted_at_unix_ms: now_millis(),
                editable: true,
            },
        );
    } else {
        let position = overlay
            .outer_position()
            .map_err(|error| error.to_string())?;
        {
            let mut inner = state.lock();
            inner.overlay_editing = false;
            inner.config.settings.overlay.x = position.x;
            inner.config.settings.overlay.y = position.y;
            storage::save_config(&inner.storage_directory, &inner.config)?;
        }
        overlay
            .set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
        overlay
            .set_focusable(false)
            .map_err(|error| error.to_string())?;
        overlay.hide().map_err(|error| error.to_string())?;
    }
    let snapshot = state.snapshot();
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

pub(crate) fn handle_sample_frame(
    app: &AppHandle,
    frame_width: u32,
    frame_height: u32,
    image: CapturedImage,
) {
    let Ok(png) = encode_png(&image) else {
        return;
    };
    let Ok(thumbnail_data_url) = encode_thumbnail(&image) else {
        return;
    };
    let state = app.state::<BuffAssistant>();
    let snapshot = {
        let mut inner = state.lock();
        if inner.capture_purpose != Some(CapturePurpose::Samples) {
            return;
        }
        let captured_at_unix_ms = now_millis();
        if inner.samples.is_empty()
            && let Some(target) = &mut inner.sample_target
        {
            target.reference_width = frame_width.max(1);
            target.reference_height = frame_height.max(1);
        }
        let id = inner.next_sample_id;
        inner.next_sample_id = inner.next_sample_id.wrapping_add(1).max(1);
        inner.sample_bytes = inner
            .sample_bytes
            .saturating_add(png.len())
            .saturating_add(thumbnail_data_url.len());
        inner.samples.push_back(StoredSample {
            id,
            captured_at_unix_ms,
            width: image.width,
            height: image.height,
            png,
            thumbnail_data_url,
        });
        while inner.samples.front().is_some_and(|sample| {
            captured_at_unix_ms - sample.captured_at_unix_ms > SAMPLE_RETENTION_MS
                || inner.sample_bytes > SAMPLE_MEMORY_LIMIT
        }) {
            if let Some(removed) = inner.samples.pop_front() {
                inner.sample_bytes = inner
                    .sample_bytes
                    .saturating_sub(removed.png.len())
                    .saturating_sub(removed.thumbnail_data_url.len());
            }
        }
        snapshot_from_runtime(&inner)
    };
    if snapshot.sample_count == 1 || snapshot.sample_count.is_multiple_of(6) {
        emit_state(app, &snapshot);
    }
}

pub(crate) fn handle_capture_frame(app: &AppHandle, purpose: CapturePurpose) {
    let state = app.state::<BuffAssistant>();
    let mut inner = state.lock();
    if inner.capture_purpose == Some(purpose) {
        inner.last_frame_at = Some(Instant::now());
    }
}

pub(crate) fn handle_detection_frame(
    app: &AppHandle,
    purpose: CapturePurpose,
    confidence: f32,
    present: bool,
    detected_at: Option<Instant>,
    emit_metric: bool,
) {
    let state = app.state::<BuffAssistant>();
    if purpose == CapturePurpose::Test {
        {
            let mut inner = state.lock();
            if inner.capture_purpose != Some(CapturePurpose::Test) {
                return;
            }
            inner.last_confidence = confidence;
        }
        if emit_metric {
            let _ = app.emit(
                "buff-assistant-metric",
                BuffMetric {
                    confidence,
                    present,
                },
            );
        }
        return;
    }
    if purpose != CapturePurpose::Monitor {
        return;
    }

    let (actions, snapshot, sound) = {
        let mut inner = state.lock();
        if inner.capture_purpose != Some(CapturePurpose::Monitor) || !inner.monitor_requested {
            return;
        }
        inner.last_confidence = confidence;
        let actions = inner
            .timeline
            .update_with_detected_at(Instant::now(), present, detected_at);
        inner.activity = match inner.timeline.phase() {
            TimelinePhase::Stopped => BuffAssistantActivity::Stopped,
            TimelinePhase::Waiting => BuffAssistantActivity::Waiting,
            TimelinePhase::Tracking => BuffAssistantActivity::Tracking,
            TimelinePhase::Prewarning => BuffAssistantActivity::Prewarning,
        };
        inner.expected_at_unix_ms = inner.timeline.expected_at().map(|expected| {
            now_millis()
                + expected
                    .saturating_duration_since(Instant::now())
                    .as_millis() as i64
        });
        (
            actions,
            snapshot_from_runtime(&inner),
            inner.config.settings.sound.clone(),
        )
    };
    if emit_metric {
        let _ = app.emit(
            "buff-assistant-metric",
            BuffMetric {
                confidence,
                present,
            },
        );
    }
    if actions.is_empty() {
        return;
    }
    emit_state(app, &snapshot);
    for action in actions {
        match action {
            TimelineAction::Triggered => {
                if sound.trigger_enabled {
                    state.audio.play(AudioCue::Triggered, sound.volume);
                }
                emit_execution_log(app, "真实触发已确认，新的倒计时已开始");
                show_countdown_overlay(app, snapshot.expected_at_unix_ms);
            }
            TimelineAction::PrewarnThree => {
                if sound.prewarn_three_enabled {
                    state.audio.play(AudioCue::PrewarnThree, sound.volume);
                }
                emit_execution_log(app, "倒计时剩余 3 秒");
            }
            TimelineAction::PrewarnTwo => {
                if sound.prewarn_two_enabled {
                    state.audio.play(AudioCue::PrewarnTwo, sound.volume);
                }
                emit_execution_log(app, "倒计时剩余 2 秒");
            }
            TimelineAction::PrewarnOne => {
                if sound.prewarn_one_enabled {
                    state.audio.play(AudioCue::PrewarnOne, sound.volume);
                }
                emit_execution_log(app, "倒计时剩余 1 秒");
            }
            TimelineAction::Reset => {
                emit_execution_log(app, "截止点未确认金周天，时间轴已重置");
                show_transient_overlay(
                    app,
                    BuffOverlayMode::Reset,
                    "时间轴已重置",
                    None,
                    Duration::from_millis(1_200),
                );
            }
        }
    }
}

pub(crate) fn handle_capture_closed(app: &AppHandle, purpose: CapturePurpose) {
    let state = app.state::<BuffAssistant>();
    let generation = {
        let mut inner = state.lock();
        if inner.capture_purpose != Some(purpose) {
            return;
        }
        inner.capture = None;
        inner.capture_purpose = None;
        inner.last_frame_at = None;
        if purpose == CapturePurpose::Monitor && inner.monitor_requested {
            inner.timeline.reset_waiting();
            inner.expected_at_unix_ms = None;
            inner.activity = BuffAssistantActivity::TargetUnavailable;
            inner.last_error = Some("游戏窗口捕获已中断，正在重新连接".into());
            Some(inner.reconnect_generation)
        } else {
            inner.activity = BuffAssistantActivity::Stopped;
            None
        }
    };
    emit_state(app, &state.snapshot());
    if let Some(generation) = generation {
        show_target_unavailable_overlay(app);
        schedule_reconnect(app.clone(), generation);
    }
}

pub(crate) fn handle_capture_error(app: &AppHandle, purpose: CapturePurpose, error: String) {
    let state = app.state::<BuffAssistant>();
    {
        let mut inner = state.lock();
        if inner.capture_purpose != Some(purpose) {
            return;
        }
        inner.capture = None;
        inner.capture_purpose = None;
        inner.last_frame_at = None;
        inner.monitor_requested = false;
        inner.timeline.stop();
        inner.expected_at_unix_ms = None;
        inner.activity = BuffAssistantActivity::Error;
        inner.last_error = Some(error);
    }
    show_transient_overlay(
        app,
        BuffOverlayMode::TargetUnavailable,
        "Buff 识别已停止",
        None,
        Duration::from_millis(1_500),
    );
    emit_state(app, &state.snapshot());
}

fn attach_monitor_capture(
    app: &AppHandle,
    window: windows_capture::window::Window,
    generation: u64,
) -> Result<(), String> {
    let state = app.state::<BuffAssistant>();
    let (flags, _) = capture_flags(app, CapturePurpose::Monitor)?;
    let control = start_runtime_capture(window, flags)?;
    let mut rejected = None;
    {
        let mut inner = state.lock();
        if !inner.monitor_requested || inner.reconnect_generation != generation {
            rejected = Some(control);
        } else {
            inner.capture = Some(control);
            inner.capture_purpose = Some(CapturePurpose::Monitor);
            inner.last_frame_at = Some(Instant::now());
            let cycle_ms = inner.config.settings.cycle_ms;
            inner.timeline.start_waiting(cycle_ms);
            inner.activity = BuffAssistantActivity::Waiting;
            inner.expected_at_unix_ms = None;
            inner.last_error = None;
        }
    }
    if let Some(control) = rejected {
        let _ = control.stop();
        return Ok(());
    }
    show_waiting_overlay(app);
    emit_state(app, &state.snapshot());
    schedule_monitor_watchdog(app.clone(), generation);
    Ok(())
}

fn capture_flags(
    app: &AppHandle,
    purpose: CapturePurpose,
) -> Result<(RuntimeCaptureFlags, BuffAssistantConfig), String> {
    let state = app.state::<BuffAssistant>();
    let inner = state.lock();
    let target = inner
        .config
        .target
        .clone()
        .ok_or_else(|| "尚未选择游戏窗口".to_string())?;
    let region = inner
        .config
        .search_region
        .ok_or_else(|| "尚未设置 Buff 搜索区域".to_string())?;
    let summary = inner
        .config
        .template
        .clone()
        .ok_or_else(|| "尚未采集金周天图标模板".to_string())?;
    let template = storage::load_template(&inner.storage_directory, &summary)?;
    Ok((
        RuntimeCaptureFlags {
            app: app.clone(),
            purpose,
            region,
            template: Some(template),
            reference_width: target.reference_width,
            reference_height: target.reference_height,
            threshold: inner.config.settings.threshold,
            confirm_frames: inner.config.settings.confirm_frames,
            missing_frames: inner.config.settings.missing_frames,
        },
        inner.config.clone(),
    ))
}

fn schedule_reconnect(app: AppHandle, generation: u64) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(2));
            let target = {
                let state = app.state::<BuffAssistant>();
                let inner = state.lock();
                if !inner.monitor_requested || inner.reconnect_generation != generation {
                    return;
                }
                if inner.capture_purpose == Some(CapturePurpose::Monitor) {
                    return;
                }
                inner.config.target.clone()
            };
            let Some(target) = target else {
                return;
            };
            match windows::find_target(&target) {
                Ok(Some(window)) => match attach_monitor_capture(&app, window, generation) {
                    Ok(()) => return,
                    Err(error) => {
                        let state = app.state::<BuffAssistant>();
                        let mut inner = state.lock();
                        if inner.last_error.as_deref() != Some(error.as_str()) {
                            inner.last_error = Some(error);
                            let snapshot = snapshot_from_runtime(&inner);
                            drop(inner);
                            emit_state(&app, &snapshot);
                        }
                    }
                },
                Ok(None) => continue,
                Err(error) => {
                    let state = app.state::<BuffAssistant>();
                    state.lock().last_error = Some(error);
                    emit_state(&app, &state.snapshot());
                }
            }
        }
    });
}

fn schedule_monitor_watchdog(app: AppHandle, generation: u64) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            let control = {
                let state = app.state::<BuffAssistant>();
                let mut inner = state.lock();
                if !inner.monitor_requested
                    || inner.reconnect_generation != generation
                    || inner.capture_purpose != Some(CapturePurpose::Monitor)
                {
                    return;
                }
                let timed_out = inner
                    .last_frame_at
                    .is_some_and(|last_frame| last_frame.elapsed() >= MONITOR_FRAME_TIMEOUT);
                if !timed_out {
                    continue;
                }
                inner.capture_purpose = None;
                inner.last_frame_at = None;
                inner.timeline.reset_waiting();
                inner.expected_at_unix_ms = None;
                inner.activity = BuffAssistantActivity::TargetUnavailable;
                inner.last_error = Some("游戏窗口长时间无画面，正在重新连接".into());
                inner.capture.take()
            };
            if let Some(control) = control {
                let _ = control.stop();
            }
            show_target_unavailable_overlay(&app);
            let state = app.state::<BuffAssistant>();
            emit_state(&app, &state.snapshot());
            schedule_reconnect(app.clone(), generation);
            return;
        }
    });
}

fn mark_target_unavailable(app: &AppHandle, message: &str) {
    let state = app.state::<BuffAssistant>();
    {
        let mut inner = state.lock();
        inner.activity = BuffAssistantActivity::TargetUnavailable;
        inner.expected_at_unix_ms = None;
        inner.last_frame_at = None;
        inner.timeline.reset_waiting();
        inner.last_error = Some(message.into());
    }
    show_target_unavailable_overlay(app);
    emit_state(app, &state.snapshot());
}

fn stop_current_capture(state: &BuffAssistant) {
    let control = {
        let mut inner = state.lock();
        inner.capture_purpose = None;
        inner.last_frame_at = None;
        inner.capture.take()
    };
    if let Some(control) = control {
        let _ = control.stop();
    }
}

fn snapshot_from_runtime(inner: &RuntimeData) -> BuffAssistantState {
    BuffAssistantState {
        config: inner.config.clone(),
        activity: inner.activity,
        is_monitoring: inner.monitor_requested,
        expected_at_unix_ms: inner.expected_at_unix_ms,
        last_confidence: inner.last_confidence,
        sample_count: inner.samples.len(),
        last_error: inner.last_error.clone(),
    }
}

fn emit_state(app: &AppHandle, state: &BuffAssistantState) {
    let _ = app.emit("buff-assistant-state", state);
}

fn emit_execution_log(app: &AppHandle, message: &str) {
    let _ = app.emit("buff-assistant-execution-log", message);
}

fn emit_overlay(app: &AppHandle, state: BuffOverlayState) {
    let _ = app.emit_to(OVERLAY_LABEL, "buff-overlay-state", state);
}

fn show_countdown_overlay(app: &AppHandle, expected_at_unix_ms: Option<i64>) {
    let state = app.state::<BuffAssistant>();
    {
        let mut inner = state.lock();
        if inner.overlay_editing {
            return;
        }
        inner.overlay_generation = inner.overlay_generation.wrapping_add(1);
    }
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.set_focusable(false);
        let _ = overlay.show();
    }
    emit_overlay(
        app,
        BuffOverlayState {
            mode: BuffOverlayMode::Countdown,
            message: "金周天即将触发".into(),
            expected_at_unix_ms,
            emitted_at_unix_ms: now_millis(),
            editable: false,
        },
    );
}

fn show_transient_overlay(
    app: &AppHandle,
    mode: BuffOverlayMode,
    message: &str,
    expected_at_unix_ms: Option<i64>,
    duration: Duration,
) {
    let state = app.state::<BuffAssistant>();
    let generation = {
        let mut inner = state.lock();
        if inner.overlay_editing {
            return;
        }
        inner.overlay_generation = inner.overlay_generation.wrapping_add(1);
        inner.overlay_generation
    };
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.set_focusable(false);
        let _ = overlay.show();
    }
    emit_overlay(
        app,
        BuffOverlayState {
            mode,
            message: message.into(),
            expected_at_unix_ms,
            emitted_at_unix_ms: now_millis(),
            editable: false,
        },
    );
    let app_handle = app.clone();
    thread::spawn(move || {
        thread::sleep(duration);
        let state = app_handle.state::<BuffAssistant>();
        let (should_hide, show_waiting) = {
            let inner = state.lock();
            (
                !inner.overlay_editing && inner.overlay_generation == generation,
                inner.monitor_requested && inner.activity == BuffAssistantActivity::Waiting,
            )
        };
        if !should_hide {
            return;
        }
        if show_waiting {
            show_waiting_overlay(&app_handle);
        } else {
            hide_overlay(&app_handle);
        }
    });
}

fn show_waiting_overlay(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let show = {
        let inner = state.lock();
        inner.monitor_requested && !inner.overlay_editing
    };
    if !show {
        hide_overlay(app);
        return;
    }
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.show();
    }
    emit_overlay(
        app,
        BuffOverlayState {
            mode: BuffOverlayMode::Waiting,
            message: "等待金周天".into(),
            expected_at_unix_ms: None,
            emitted_at_unix_ms: now_millis(),
            editable: false,
        },
    );
}

fn show_target_unavailable_overlay(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    {
        let mut inner = state.lock();
        if inner.overlay_editing {
            return;
        }
        inner.overlay_generation = inner.overlay_generation.wrapping_add(1);
    }
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.set_focusable(false);
        let _ = overlay.show();
    }
    emit_overlay(
        app,
        BuffOverlayState {
            mode: BuffOverlayMode::TargetUnavailable,
            message: "等待游戏窗口".into(),
            expected_at_unix_ms: None,
            emitted_at_unix_ms: now_millis(),
            editable: false,
        },
    );
}

fn hide_overlay(app: &AppHandle) {
    emit_overlay(
        app,
        BuffOverlayState {
            mode: BuffOverlayMode::Hidden,
            message: String::new(),
            expected_at_unix_ms: None,
            emitted_at_unix_ms: now_millis(),
            editable: false,
        },
    );
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
}

fn apply_overlay_position(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let position = state.lock().config.settings.overlay.clone();
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_position(PhysicalPosition::new(position.x, position.y));
    }
}

fn encode_png(image: &CapturedImage) -> Result<Vec<u8>, String> {
    let rgba = RgbaImage::from_raw(image.width, image.height, image.rgba.clone())
        .ok_or_else(|| "捕获画面像素格式无效".to_string())?;
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(rgba)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|error| format!("编码 PNG 失败：{error}"))?;
    Ok(bytes.into_inner())
}

fn png_data_url(image: &CapturedImage) -> Result<String, String> {
    Ok(format!(
        "data:image/png;base64,{}",
        BASE64.encode(encode_png(image)?)
    ))
}

fn encode_thumbnail(image: &CapturedImage) -> Result<String, String> {
    let rgba = RgbaImage::from_raw(image.width, image.height, image.rgba.clone())
        .ok_or_else(|| "捕获画面像素格式无效".to_string())?;
    let thumbnail = DynamicImage::ImageRgba8(rgba).thumbnail(180, 100).to_rgb8();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 72)
        .write_image(
            thumbnail.as_raw(),
            thumbnail.width(),
            thumbnail.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| format!("编码缩略图失败：{error}"))?;
    Ok(format!("data:image/jpeg;base64,{}", BASE64.encode(bytes)))
}

fn decode_mask(data_url: Option<&str>, width: u32, height: u32) -> Result<GrayImage, String> {
    let Some(data_url) = data_url else {
        return Ok(GrayImage::from_pixel(width, height, Luma([255])));
    };
    let encoded = data_url
        .split_once(',')
        .map(|(_, encoded)| encoded)
        .ok_or_else(|| "模板遮罩格式无效".to_string())?;
    let bytes = BASE64
        .decode(encoded)
        .map_err(|error| format!("解析模板遮罩失败：{error}"))?;
    let mask = image::load_from_memory(&bytes)
        .map_err(|error| format!("读取模板遮罩失败：{error}"))?
        .into_luma8();
    if mask.dimensions() == (width, height) {
        Ok(mask)
    } else {
        Ok(image::imageops::resize(
            &mask,
            width,
            height,
            image::imageops::FilterType::Nearest,
        ))
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuffMetric {
    confidence: f32,
    present: bool,
}

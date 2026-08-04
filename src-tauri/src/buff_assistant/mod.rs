mod audio;
mod capture;
mod detector;
mod model;
mod storage;
mod timeline;
mod windows;

use std::{
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
use image::{DynamicImage, GrayImage, Luma, RgbaImage};
pub use model::{
    BuffAssistantActivity, BuffAssistantConfig, BuffAssistantSettings, BuffAssistantState,
    BuffOverlayColorScheme, BuffOverlayMode, BuffOverlayState, BuffTarget, CapturePreview,
    CaptureWindowCandidate, MAX_OVERLAY_HEIGHT, MAX_OVERLAY_WIDTH, MIN_OVERLAY_HEIGHT,
    MIN_OVERLAY_WIDTH, NormalizedRect,
};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl,
    WebviewWindowBuilder,
};
use timeline::{BuffTimeline, TimelineAction, TimelinePhase};

const MONITOR_FRAME_TIMEOUT: Duration = Duration::from_secs(3);
const OVERLAY_LABEL: &str = "buff-overlay";

struct StoredPreview {
    png: Vec<u8>,
    target: BuffTarget,
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
    template_preview: Option<StoredPreview>,
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
                    template_preview: None,
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
    .inner_size(
        f64::from(config.settings.overlay.width),
        f64::from(config.settings.overlay.height),
    )
    .min_inner_size(f64::from(MIN_OVERLAY_WIDTH), f64::from(MIN_OVERLAY_HEIGHT))
    .max_inner_size(f64::from(MAX_OVERLAY_WIDTH), f64::from(MAX_OVERLAY_HEIGHT))
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
pub fn capture_buff_preview(
    state: State<'_, BuffAssistant>,
    window_id: String,
) -> Result<CapturePreview, String> {
    let (window, candidate) = windows::resolve_window(&window_id)?;
    let image = capture_snapshot(window)?;
    let png = encode_png(&image)?;
    let target = BuffTarget {
        reference_width: image.width,
        reference_height: image.height,
        ..windows::target_from_candidate(&candidate)
    };
    let data_url = png_bytes_data_url(&png);
    state.lock().template_preview = Some(StoredPreview {
        png,
        target: target.clone(),
    });
    Ok(CapturePreview {
        data_url,
        width: image.width,
        height: image.height,
        target,
    })
}

#[tauri::command]
pub fn save_buff_template(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    search_region: NormalizedRect,
    crop: NormalizedRect,
    mask_data_url: Option<String>,
) -> Result<BuffAssistantState, String> {
    let (png, target, directory) = {
        let inner = state.lock();
        let preview = inner
            .template_preview
            .as_ref()
            .ok_or_else(|| "找不到捕获预览，请重新捕获".to_string())?;
        (
            preview.png.clone(),
            preview.target.clone(),
            inner.storage_directory.clone(),
        )
    };
    let source =
        image::load_from_memory(&png).map_err(|error| format!("读取捕获预览失败：{error}"))?;
    let region = search_region.sanitized();
    let template = crop_template_from_preview(&source, region, crop)?;
    let width = template.width();
    let height = template.height();
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
    apply_overlay_geometry(&app);
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
            return Err("请先完成金周天模板配置".into());
        }
        inner.monitor_requested = true;
        inner.reconnect_generation = inner.reconnect_generation.wrapping_add(1);
        let cycle_ms = inner.config.settings.cycle_ms;
        let deadline_grace_ms = inner.config.settings.deadline_grace_ms;
        inner
            .timeline
            .start_waiting_with_grace(cycle_ms, deadline_grace_ms);
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
    set_buff_overlay_edit_mode_internal(&app, enabled)?;
    Ok(state.snapshot())
}

fn set_buff_overlay_edit_mode_internal(
    app: &AppHandle,
    enabled: bool,
) -> Result<BuffAssistantState, String> {
    let state = app.state::<BuffAssistant>();
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
            .set_resizable(true)
            .map_err(|error| error.to_string())?;
        overlay
            .set_focusable(true)
            .map_err(|error| error.to_string())?;
        overlay
            .set_ignore_cursor_events(false)
            .map_err(|error| error.to_string())?;
        overlay.show().map_err(|error| error.to_string())?;
        emit_overlay(
            app,
            BuffOverlayState {
                mode: BuffOverlayMode::Editing,
                message: "拖动调整位置与大小".into(),
                expected_at_unix_ms: None,
                emitted_at_unix_ms: now_millis(),
                editable: true,
                show_border: overlay_border_setting(app),
                color_scheme: overlay_color_scheme(app),
            },
        );
    } else {
        let position = overlay
            .outer_position()
            .map_err(|error| error.to_string())?;
        let size = overlay.inner_size().map_err(|error| error.to_string())?;
        let scale_factor = overlay.scale_factor().map_err(|error| error.to_string())?;
        {
            let mut inner = state.lock();
            inner.overlay_editing = false;
            inner.config.settings.overlay.x = position.x;
            inner.config.settings.overlay.y = position.y;
            inner.config.settings.overlay.width =
                (f64::from(size.width) / scale_factor).round() as u32;
            inner.config.settings.overlay.height =
                (f64::from(size.height) / scale_factor).round() as u32;
            inner.config.settings.sanitize();
            storage::save_config(&inner.storage_directory, &inner.config)?;
        }
        overlay
            .set_resizable(false)
            .map_err(|error| error.to_string())?;
        overlay
            .set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
        overlay
            .set_focusable(false)
            .map_err(|error| error.to_string())?;
    }
    let snapshot = state.snapshot();
    emit_state(app, &snapshot);
    if !enabled {
        restore_overlay_after_edit(app, &snapshot);
    }
    Ok(snapshot)
}

fn restore_overlay_after_edit(app: &AppHandle, snapshot: &BuffAssistantState) {
    if !snapshot.is_monitoring {
        hide_overlay(app);
        return;
    }
    match snapshot.activity {
        BuffAssistantActivity::Waiting => show_waiting_overlay(app),
        BuffAssistantActivity::Tracking | BuffAssistantActivity::Prewarning => {
            show_countdown_overlay(app, snapshot.expected_at_unix_ms);
        }
        BuffAssistantActivity::Confirming => show_confirming_overlay(app),
        BuffAssistantActivity::TargetUnavailable => show_target_unavailable_overlay(app),
        _ => hide_overlay(app),
    }
}

pub(crate) fn stop_buff_workspace_activity_internal(app: &AppHandle) -> Result<(), String> {
    stop_buff_monitor_internal(app);
    let editing = app.state::<BuffAssistant>().lock().overlay_editing;
    if editing {
        set_buff_overlay_edit_mode_internal(app, false)?;
    }
    Ok(())
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
    absence_confirmed: bool,
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
        let actions = inner.timeline.update_with_detected_at(
            Instant::now(),
            present,
            absence_confirmed,
            detected_at,
        );
        inner.activity = match inner.timeline.phase() {
            TimelinePhase::Stopped => BuffAssistantActivity::Stopped,
            TimelinePhase::Waiting => BuffAssistantActivity::Waiting,
            TimelinePhase::Tracking => BuffAssistantActivity::Tracking,
            TimelinePhase::Prewarning => BuffAssistantActivity::Prewarning,
            TimelinePhase::Confirming => BuffAssistantActivity::Confirming,
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
                emit_execution_log(app, "真实触发已确认，已按实际触发时间校准倒计时");
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
            TimelineAction::ConfirmationPending => {
                emit_execution_log(app, "倒计时已结束，正在宽限期内等待金周天确认");
                show_confirming_overlay(app);
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
            let deadline_grace_ms = inner.config.settings.deadline_grace_ms;
            inner
                .timeline
                .start_waiting_with_grace(cycle_ms, deadline_grace_ms);
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
        .ok_or_else(|| "尚未配置金周天图标模板".to_string())?;
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
            show_border: overlay_border_setting(app),
            color_scheme: overlay_color_scheme(app),
        },
    );
}

fn show_confirming_overlay(app: &AppHandle) {
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
            mode: BuffOverlayMode::Confirming,
            message: "等待金周天确认".into(),
            expected_at_unix_ms: None,
            emitted_at_unix_ms: now_millis(),
            editable: false,
            show_border: overlay_border_setting(app),
            color_scheme: overlay_color_scheme(app),
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
            show_border: overlay_border_setting(app),
            color_scheme: overlay_color_scheme(app),
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
            show_border: overlay_border_setting(app),
            color_scheme: overlay_color_scheme(app),
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
            show_border: overlay_border_setting(app),
            color_scheme: overlay_color_scheme(app),
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
            show_border: overlay_border_setting(app),
            color_scheme: overlay_color_scheme(app),
        },
    );
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
}

fn apply_overlay_geometry(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let settings = state.lock().config.settings.overlay.clone();
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_position(PhysicalPosition::new(settings.x, settings.y));
        let _ = overlay.set_size(LogicalSize::new(
            f64::from(settings.width),
            f64::from(settings.height),
        ));
    }
}

fn overlay_border_setting(app: &AppHandle) -> bool {
    app.state::<BuffAssistant>()
        .lock()
        .config
        .settings
        .overlay
        .show_border
}

fn overlay_color_scheme(app: &AppHandle) -> BuffOverlayColorScheme {
    app.state::<BuffAssistant>()
        .lock()
        .config
        .settings
        .overlay
        .color_scheme
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

fn png_bytes_data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", BASE64.encode(png))
}

fn crop_template_from_preview(
    preview: &DynamicImage,
    search_region: NormalizedRect,
    crop: NormalizedRect,
) -> Result<DynamicImage, String> {
    let (region_x, region_y, region_end_x, region_end_y) =
        search_region.pixel_bounds(preview.width(), preview.height());
    let region = preview.crop_imm(
        region_x,
        region_y,
        region_end_x - region_x,
        region_end_y - region_y,
    );
    let (x, y, end_x, end_y) = crop.pixel_bounds(region.width(), region.height());
    let width = end_x - x;
    let height = end_y - y;
    if width < 8 || height < 8 {
        return Err("模板区域过小，请重新框选图标".into());
    }
    Ok(region.crop_imm(x, y, width, height))
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

#[cfg(test)]
mod tests {
    use image::DynamicImage;

    use super::{NormalizedRect, crop_template_from_preview};

    #[test]
    fn template_crop_is_relative_to_the_selected_search_region() {
        let preview = DynamicImage::new_rgba8(200, 100);
        let template = crop_template_from_preview(
            &preview,
            NormalizedRect {
                x: 0.25,
                y: 0.2,
                width: 0.5,
                height: 0.5,
            },
            NormalizedRect {
                x: 0.1,
                y: 0.2,
                width: 0.4,
                height: 0.6,
            },
        )
        .expect("template crop should succeed");

        assert_eq!((template.width(), template.height()), (40, 30));
    }

    #[test]
    fn template_crop_rejects_tiny_regions() {
        let preview = DynamicImage::new_rgba8(100, 100);
        let result = crop_template_from_preview(
            &preview,
            NormalizedRect {
                x: 0.0,
                y: 0.0,
                width: 0.1,
                height: 0.1,
            },
            NormalizedRect {
                x: 0.0,
                y: 0.0,
                width: 0.1,
                height: 0.1,
            },
        );

        assert!(result.is_err());
    }
}

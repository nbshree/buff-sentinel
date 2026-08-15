mod audio;
mod capture;
mod detector;
mod model;
mod storage;
mod timeline;
mod windows;

use std::{
    collections::HashMap,
    io::Cursor,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ::windows::{
    Graphics::Capture::{GraphicsCaptureAccess, GraphicsCaptureAccessKind},
    Security::Authorization::AppCapabilityAccess::AppCapabilityAccessStatus,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};
use audio::{AudioEngine, ResolvedSoundSource};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use capture::{
    CapturePurpose, CapturedImage, RuntimeCaptureControl, RuntimeCaptureFlags, RuntimeDetection,
    RuntimeListenerFlags, capture_border_supported, capture_snapshot, start_runtime_capture,
};
use image::{DynamicImage, GrayImage, Luma, RgbaImage};
pub use model::{
    BorderlessCaptureAccessResult, BuffAssistantActivity, BuffAssistantConfig, BuffAssistantState,
    BuffCustomSoundAsset, BuffGlobalSettings, BuffListenerConfig, BuffListenerRuntimeState,
    BuffListenerSettings, BuffOverlayColorScheme, BuffOverlayItem, BuffOverlayMode,
    BuffOverlayState, BuffSoundCue, BuffSoundSource, BuffSoundTemplateSummary, BuffTarget,
    BuffTemplatePreview, CapturePreview, CaptureWindowCandidate, DEFAULT_OVERLAY_HEIGHT,
    MAX_LISTENERS, MAX_OVERLAY_HEIGHT, MAX_OVERLAY_WIDTH, MIN_OVERLAY_HEIGHT, MIN_OVERLAY_WIDTH,
    NormalizedRect,
};
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_dialog::DialogExt;
use timeline::{BuffTimeline, TimelineAction, TimelinePhase};

const MONITOR_FRAME_TIMEOUT: Duration = Duration::from_secs(3);
const OVERLAY_LABEL: &str = "buff-overlay";
const TTS_ONLINE_URL: &str = "https://www.ttsonline.cn/";
const CAPTURE_BORDER_FALLBACK_NOTICE: &str = "无法隐藏系统捕获黄色边框，已保留边框并继续捕获";

struct StoredPreview {
    png: Vec<u8>,
    target: BuffTarget,
}

struct RuntimeData {
    config: BuffAssistantConfig,
    activity: BuffAssistantActivity,
    monitor_requested: bool,
    last_error: Option<String>,
    capture_border_supported: bool,
    capture_border_notice: Option<String>,
    storage_directory: PathBuf,
    sound_templates: Vec<storage::SoundTemplate>,
    capture: Option<RuntimeCaptureControl>,
    capture_purpose: Option<CapturePurpose>,
    listeners: HashMap<String, ListenerRuntime>,
    template_preview: Option<StoredPreview>,
    last_frame_at: Option<Instant>,
    reconnect_generation: u64,
    overlay_generation: u64,
    overlay_editing: bool,
    overlay_window: OverlayWindowCache,
}

#[derive(Default)]
struct OverlayWindowCache {
    visible: bool,
    size: Option<(u32, u32)>,
    rows: Option<usize>,
}

impl OverlayWindowCache {
    fn mark_visible(&mut self) -> bool {
        if self.visible {
            false
        } else {
            self.visible = true;
            true
        }
    }

    fn mark_hidden(&mut self) -> bool {
        let was_visible = self.visible;
        self.visible = false;
        was_visible
    }

    fn update_size(&mut self, size: (u32, u32), rows: Option<usize>) -> bool {
        if self.size == Some(size) && self.rows == rows {
            false
        } else {
            self.size = Some(size);
            self.rows = rows;
            true
        }
    }

    fn invalidate_size(&mut self) {
        self.size = None;
        self.rows = None;
    }
}

struct ListenerRuntime {
    activity: BuffAssistantActivity,
    expected_at_unix_ms: Option<i64>,
    last_confidence: f32,
    last_error: Option<String>,
    timeline: BuffTimeline,
}

impl ListenerRuntime {
    fn new(settings: &BuffListenerSettings) -> Self {
        Self {
            activity: BuffAssistantActivity::Stopped,
            expected_at_unix_ms: None,
            last_confidence: 0.0,
            last_error: None,
            timeline: BuffTimeline::new(settings.cycle_ms),
        }
    }
}

pub struct BuffAssistant {
    inner: Mutex<RuntimeData>,
    audio: AudioEngine,
}

impl BuffAssistant {
    pub fn load(app: &AppHandle) -> Result<(Self, Vec<String>), String> {
        let directory = storage::storage_directory(app)?;
        let templates_directory = storage::sound_templates_directory(app)?;
        let (sound_templates, template_notices) =
            storage::load_sound_templates(&templates_directory);
        let (mut config, mut notices) = storage::load_config(&directory);
        notices.extend(template_notices);
        if repair_missing_sound_sources(&directory, &sound_templates, &mut config, &mut notices)
            && let Err(error) = storage::save_config(&directory, &config)
        {
            notices.push(error);
        }
        let capture_border_supported = capture_border_supported();
        if !capture_border_supported {
            config.settings.capture.show_system_border = true;
        }
        let audio = AudioEngine::start(app.clone());
        Ok((
            Self {
                inner: Mutex::new(RuntimeData {
                    listeners: listener_runtime_map(&config),
                    config,
                    activity: BuffAssistantActivity::Stopped,
                    monitor_requested: false,
                    last_error: None,
                    capture_border_supported,
                    capture_border_notice: None,
                    storage_directory: directory,
                    sound_templates,
                    capture: None,
                    capture_purpose: None,
                    template_preview: None,
                    last_frame_at: None,
                    reconnect_generation: 0,
                    overlay_generation: 0,
                    overlay_editing: false,
                    overlay_window: OverlayWindowCache::default(),
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
    .title("BuffFlow 提醒")
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
    .content_protected(config.settings.overlay.exclude_from_capture)
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
pub fn list_buff_sound_templates(state: State<'_, BuffAssistant>) -> Vec<BuffSoundTemplateSummary> {
    state
        .lock()
        .sound_templates
        .iter()
        .map(|template| template.summary.clone())
        .collect()
}

#[tauri::command]
pub fn capture_buff_preview(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    window_id: String,
) -> Result<CapturePreview, String> {
    let (window, candidate) = windows::resolve_window(&window_id)?;
    let show_system_border = state.lock().config.settings.capture.show_system_border;
    let outcome = capture_snapshot(window, show_system_border)?;
    let image = outcome.value;
    let png = encode_png(&image)?;
    let target = BuffTarget {
        reference_width: image.width,
        reference_height: image.height,
        ..windows::target_from_candidate(&candidate)
    };
    let data_url = png_bytes_data_url(&png);
    {
        let mut inner = state.lock();
        inner.template_preview = Some(StoredPreview {
            png,
            target: target.clone(),
        });
        update_capture_border_notice(&mut inner, outcome.used_border_fallback);
    }
    emit_state(&app, &state.snapshot());
    Ok(CapturePreview {
        data_url,
        width: image.width,
        height: image.height,
        target,
    })
}

#[tauri::command]
pub fn update_buff_search_region(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    search_region: NormalizedRect,
) -> Result<BuffAssistantState, String> {
    ensure_configuration_unlocked(&state)?;
    let snapshot = {
        let mut inner = state.lock();
        let target = inner
            .template_preview
            .as_ref()
            .map(|preview| preview.target.clone())
            .or_else(|| inner.config.target.clone())
            .ok_or_else(|| "请先捕获游戏窗口预览".to_string())?;
        inner.config.target = Some(target);
        inner.config.search_region = Some(search_region.sanitized());
        inner.config.sanitize();
        inner.listeners = listener_runtime_map(&inner.config);
        storage::save_config(&inner.storage_directory, &inner.config)?;
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn request_buff_borderless_capture_access(
    state: State<'_, BuffAssistant>,
) -> Result<BorderlessCaptureAccessResult, String> {
    if !state.lock().capture_border_supported {
        return Ok(BorderlessCaptureAccessResult::Unsupported);
    }
    let result = tauri::async_runtime::spawn_blocking(request_borderless_capture_access)
        .await
        .unwrap_or(BorderlessCaptureAccessResult::DeniedBySystem);
    {
        let mut inner = state.lock();
        inner.capture_border_notice = borderless_access_notice(result).map(str::to_string);
    }
    Ok(result)
}

fn request_borderless_capture_access() -> BorderlessCaptureAccessResult {
    let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
    let result =
        match GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Borderless) {
            Ok(operation) => match operation.join() {
                Ok(status) if status == AppCapabilityAccessStatus::Allowed => {
                    BorderlessCaptureAccessResult::Allowed
                }
                Ok(status) if status == AppCapabilityAccessStatus::DeniedByUser => {
                    BorderlessCaptureAccessResult::DeniedByUser
                }
                Ok(status) if status == AppCapabilityAccessStatus::NotDeclaredByApp => {
                    BorderlessCaptureAccessResult::NotDeclared
                }
                Ok(_) | Err(_) => BorderlessCaptureAccessResult::DeniedBySystem,
            },
            Err(_) => BorderlessCaptureAccessResult::DeniedBySystem,
        };
    if initialized {
        unsafe { RoUninitialize() };
    }
    result
}

#[tauri::command]
pub fn save_buff_listener(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    request: SaveBuffListenerRequest,
) -> Result<BuffAssistantState, String> {
    let SaveBuffListenerRequest {
        listener_id,
        name,
        enabled,
        mut settings,
        search_region,
        crop,
        mask_data_url,
    } = request;
    ensure_configuration_unlocked(&state)?;
    settings.sanitize();
    let name = validate_listener_name(&state, listener_id.as_deref(), &name)?;
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
    let region = search_region.sanitized();
    let source =
        image::load_from_memory(&png).map_err(|error| format!("读取捕获预览失败：{error}"))?;
    let template = crop_template_from_preview(&source, region, crop)?;
    let width = template.width();
    let height = template.height();
    let mask = decode_mask(mask_data_url.as_deref(), width, height)?;
    let template_id = format!("buff-listener-{}", now_millis());
    let (region_x, region_y, region_end_x, region_end_y) =
        region.pixel_bounds(source.width(), source.height());
    let source_region = source.crop_imm(
        region_x,
        region_y,
        region_end_x - region_x,
        region_end_y - region_y,
    );
    let mut summary = storage::save_template(
        &directory,
        &template_id,
        &template,
        &mask,
        Some(&source_region),
    )?;
    summary.crop = Some(crop);
    let snapshot = {
        let mut inner = state.lock();
        if listener_id.is_none() && inner.config.listeners.len() >= MAX_LISTENERS {
            storage::delete_template(&directory, &summary)?;
            return Err(format!("最多只能添加 {MAX_LISTENERS} 个监听图标"));
        }
        inner.config.target = Some(BuffTarget {
            reference_width: target.reference_width,
            reference_height: target.reference_height,
            ..target
        });
        inner.config.search_region = Some(region);
        let id = listener_id.unwrap_or_else(|| format!("listener-{}", now_millis()));
        if let Some(listener) = inner.config.listeners.iter_mut().find(|item| item.id == id) {
            if let Some(previous) = listener.template.replace(summary) {
                storage::delete_template(&directory, &previous)?;
            }
            listener.name = name;
            listener.enabled = enabled;
            listener.settings = settings;
        } else {
            inner.config.listeners.push(BuffListenerConfig {
                id: id.clone(),
                name,
                enabled,
                template: Some(summary),
                settings,
            });
        }
        inner.config.sanitize();
        inner.listeners = listener_runtime_map(&inner.config);
        storage::save_config(&inner.storage_directory, &inner.config)?;
        inner.last_error = None;
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBuffListenerRequest {
    listener_id: Option<String>,
    name: String,
    enabled: bool,
    settings: BuffListenerSettings,
    search_region: NormalizedRect,
    crop: NormalizedRect,
    mask_data_url: Option<String>,
}

#[tauri::command]
pub fn get_buff_listener_template(
    state: State<'_, BuffAssistant>,
    listener_id: String,
) -> Result<BuffTemplatePreview, String> {
    let (directory, summary) = {
        let inner = state.lock();
        let listener = inner
            .config
            .listeners
            .iter()
            .find(|listener| listener.id == listener_id)
            .ok_or_else(|| "监听项不存在".to_string())?;
        let summary = listener
            .template
            .clone()
            .ok_or_else(|| "监听项尚未配置图标模板".to_string())?;
        (inner.storage_directory.clone(), summary)
    };
    let (image, mask, source) = storage::load_template_editor_assets(&directory, &summary)?;
    Ok(BuffTemplatePreview {
        image_data_url: png_bytes_data_url(&image),
        mask_data_url: png_bytes_data_url(&mask),
        source_data_url: source.as_deref().map(png_bytes_data_url),
        crop: summary.crop,
    })
}

#[tauri::command]
pub fn delete_buff_listener(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    listener_id: String,
) -> Result<BuffAssistantState, String> {
    ensure_configuration_unlocked(&state)?;
    let snapshot = {
        let mut inner = state.lock();
        let index = inner
            .config
            .listeners
            .iter()
            .position(|listener| listener.id == listener_id)
            .ok_or_else(|| "监听项不存在".to_string())?;
        let listener = inner.config.listeners.remove(index);
        if let Some(template) = listener.template {
            storage::delete_template(&inner.storage_directory, &template)?;
        }
        inner.listeners.remove(&listener_id);
        storage::save_config(&inner.storage_directory, &inner.config)?;
        storage::cleanup_unused_sound_assets(&inner.storage_directory, &inner.config);
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn update_buff_listener(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    request: UpdateBuffListenerRequest,
) -> Result<BuffAssistantState, String> {
    let UpdateBuffListenerRequest {
        listener_id,
        name,
        enabled,
        mut settings,
        mask_data_url,
        crop,
    } = request;
    ensure_configuration_unlocked(&state)?;
    settings.sanitize();
    let name = validate_listener_name(&state, Some(&listener_id), &name)?;
    let snapshot = {
        let mut inner = state.lock();
        let mut next_config = inner.config.clone();
        let listener_index = next_config
            .listeners
            .iter()
            .position(|listener| listener.id == listener_id)
            .ok_or_else(|| "监听项不存在".to_string())?;
        let template = {
            let listener = &mut next_config.listeners[listener_index];
            listener.name = name;
            listener.enabled = enabled;
            listener.settings = settings;
            listener.template.clone()
        };
        storage::validate_sound_sources(
            &inner.storage_directory,
            &inner.sound_templates,
            &next_config,
        )?;
        if let Some(crop) = crop {
            let template = template.ok_or_else(|| "监听项尚未配置图标模板".to_string())?;
            let (image_bytes, mask_bytes) =
                storage::load_template_assets(&inner.storage_directory, &template)?;
            let image = image::load_from_memory(&image_bytes)
                .map_err(|error| format!("读取模板图片失败：{error}"))?;
            let cropped_image = crop_saved_template(&image, crop)?;
            let mask = if let Some(mask_data_url) = mask_data_url.as_deref() {
                decode_mask(
                    Some(mask_data_url),
                    cropped_image.width(),
                    cropped_image.height(),
                )?
            } else {
                crop_saved_template(
                    &image::load_from_memory(&mask_bytes)
                        .map_err(|error| format!("读取模板遮罩失败：{error}"))?,
                    crop,
                )?
                .into_luma8()
            };
            let summary = storage::save_template(
                &inner.storage_directory,
                &template.id,
                &cropped_image,
                &mask,
                None,
            )?;
            next_config.listeners[listener_index].template = Some(summary);
        } else if let Some(mask_data_url) = mask_data_url.as_deref() {
            let template = template.ok_or_else(|| "监听项尚未配置图标模板".to_string())?;
            let mask = decode_mask(Some(mask_data_url), template.width, template.height)?;
            storage::save_template_mask(&inner.storage_directory, &template, &mask)?;
        }
        inner.config = next_config;
        inner.listeners = listener_runtime_map(&inner.config);
        storage::save_config(&inner.storage_directory, &inner.config)?;
        storage::cleanup_unused_sound_assets(&inner.storage_directory, &inner.config);
        snapshot_from_runtime(&inner)
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBuffListenerRequest {
    listener_id: String,
    name: String,
    enabled: bool,
    settings: BuffListenerSettings,
    mask_data_url: Option<String>,
    crop: Option<NormalizedRect>,
}

#[tauri::command]
pub fn update_buff_assistant_settings(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    mut settings: BuffGlobalSettings,
) -> Result<BuffAssistantState, String> {
    ensure_configuration_unlocked(&state)?;
    settings.sanitize();
    {
        let mut inner = state.lock();
        if !inner.capture_border_supported {
            settings.capture.show_system_border = true;
        }
        inner.config.settings = settings;
        storage::save_config(&inner.storage_directory, &inner.config)?;
    }
    apply_overlay_geometry(&app);
    let capture_protection_result = apply_overlay_capture_protection(&app);
    let snapshot = state.snapshot();
    emit_state(&app, &snapshot);
    capture_protection_result?;
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
        if inner.overlay_editing {
            return Err("请先保存悬浮窗位置再开始监控".into());
        }
        if inner.config.target.is_none() || inner.config.search_region.is_none() {
            return Err("请先选择游戏窗口并设置 Buff 搜索区域".into());
        }
        let enabled = inner
            .config
            .listeners
            .iter()
            .filter(|listener| listener.enabled && listener.template.is_some())
            .map(|listener| (listener.id.clone(), listener.settings.clone()))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return Err("请至少启用一个已配置模板的监听项".into());
        }
        inner.monitor_requested = true;
        inner.reconnect_generation = inner.reconnect_generation.wrapping_add(1);
        for (id, settings) in enabled {
            let runtime = inner
                .listeners
                .entry(id)
                .or_insert_with(|| ListenerRuntime::new(&settings));
            runtime
                .timeline
                .start_waiting_with_grace(settings.cycle_ms, settings.deadline_grace_ms);
            runtime.activity = BuffAssistantActivity::Waiting;
            runtime.expected_at_unix_ms = None;
            runtime.last_error = None;
        }
        inner.activity = BuffAssistantActivity::Waiting;
        inner.last_error = None;
        (
            inner.config.target.clone().unwrap(),
            inner.reconnect_generation,
        )
    };
    preload_monitor_sounds(&state);
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
        inner.last_frame_at = None;
        for runtime in inner.listeners.values_mut() {
            runtime.timeline.stop();
            runtime.activity = BuffAssistantActivity::Stopped;
            runtime.expected_at_unix_ms = None;
        }
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
    listener_id: String,
) -> Result<BuffAssistantState, String> {
    let (window, _) = windows::resolve_window(&window_id)?;
    stop_buff_monitor_internal(&app);
    let (flags, config) = capture_flags(&app, CapturePurpose::Test, Some(&listener_id))?;
    let outcome = start_runtime_capture(window, flags)?;
    let snapshot = {
        let mut inner = state.lock();
        inner.capture = Some(outcome.value);
        inner.capture_purpose = Some(CapturePurpose::Test);
        inner.monitor_requested = false;
        inner.activity = BuffAssistantActivity::Testing;
        inner.last_error = None;
        inner.config = config;
        if let Some(runtime) = inner.listeners.get_mut(&listener_id) {
            runtime.activity = BuffAssistantActivity::Testing;
            runtime.last_confidence = 0.0;
        }
        update_capture_border_notice(&mut inner, outcome.used_border_fallback);
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
        for runtime in inner.listeners.values_mut() {
            if runtime.activity == BuffAssistantActivity::Testing {
                runtime.activity = BuffAssistantActivity::Stopped;
            }
        }
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
    cue: BuffSoundCue,
    source: BuffSoundSource,
    volume: f32,
) -> Result<(), String> {
    let inner = state.lock();
    let resolved = resolve_sound_source(&inner, cue, &source)?;
    state.audio.play(cue, resolved, volume);
    Ok(())
}

#[tauri::command]
pub async fn import_buff_assistant_sound(
    app: AppHandle,
    state: State<'_, BuffAssistant>,
    cue: BuffSoundCue,
) -> Result<Option<BuffCustomSoundAsset>, String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title(format!("选择{} WAV", sound_cue_label(cue)))
        .add_filter("WAV 音频", &["wav"]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|error| format!("读取所选 WAV 路径失败：{error}"))?;
    storage::validate_sound_asset_candidate(&path)?;
    audio::validate_wav_file(&path)?;
    let directory = state.lock().storage_directory.clone();
    let asset_id = format!("{}-{}", sound_cue_id(cue), now_millis());
    let asset = storage::import_sound_asset(&directory, &path, &asset_id)?;
    let copied = storage::custom_sound_path(&directory, &asset.asset_id)?;
    if let Err(error) = audio::validate_wav_file(&copied) {
        let _ = std::fs::remove_file(copied);
        return Err(error);
    }
    Ok(Some(asset))
}

#[tauri::command]
pub fn open_tts_online() -> Result<(), String> {
    open_fixed_url(TTS_ONLINE_URL)
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
            inner.overlay_window.invalidate_size();
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
        state.lock().overlay_window.visible = true;
        show_overlay_preview(app, BuffOverlayMode::Countdown)?;
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
            inner.overlay_window.visible = true;
            inner.overlay_window.invalidate_size();
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

#[tauri::command]
pub fn set_buff_overlay_preview_mode(app: AppHandle, mode: BuffOverlayMode) -> Result<(), String> {
    show_overlay_preview(&app, mode)
}

fn show_overlay_preview(app: &AppHandle, mode: BuffOverlayMode) -> Result<(), String> {
    let state = app.state::<BuffAssistant>();
    let listeners = {
        let mut inner = state.lock();
        if !inner.overlay_editing {
            return Err("请先进入悬浮窗调整模式".into());
        }
        inner.overlay_generation = inner.overlay_generation.wrapping_add(1);
        let listeners = inner
            .config
            .listeners
            .iter()
            .filter(|listener| listener.enabled && listener.template.is_some())
            .map(|listener| {
                (
                    listener.id.clone(),
                    listener.name.clone(),
                    listener.settings.cycle_ms,
                )
            })
            .collect::<Vec<_>>();
        if listeners.is_empty() {
            vec![("preview".into(), "监听图标".into(), 20_000)]
        } else {
            listeners
        }
    };
    let emitted_at_unix_ms = now_millis();
    let (message, items, rows) = match mode {
        BuffOverlayMode::Waiting => {
            let items = listeners
                .iter()
                .map(|(id, name, _)| BuffOverlayItem {
                    listener_id: id.clone(),
                    name: name.clone(),
                    mode: BuffOverlayMode::Waiting,
                    expected_at_unix_ms: None,
                })
                .collect::<Vec<_>>();
            (String::new(), items, listeners.len())
        }
        BuffOverlayMode::Countdown => {
            let items = listeners
                .iter()
                .enumerate()
                .map(|(index, (id, name, cycle_ms))| BuffOverlayItem {
                    listener_id: id.clone(),
                    name: name.clone(),
                    mode: BuffOverlayMode::Countdown,
                    expected_at_unix_ms: Some(
                        emitted_at_unix_ms
                            + i64::try_from((*cycle_ms).min(60_000)).unwrap_or(60_000)
                            + i64::try_from(index).unwrap_or_default() * 1_500,
                    ),
                })
                .collect::<Vec<_>>();
            (String::new(), items, listeners.len())
        }
        BuffOverlayMode::Confirming => {
            let items = listeners
                .iter()
                .map(|(id, name, _)| BuffOverlayItem {
                    listener_id: id.clone(),
                    name: name.clone(),
                    mode: BuffOverlayMode::Confirming,
                    expected_at_unix_ms: None,
                })
                .collect::<Vec<_>>();
            (String::new(), items, listeners.len())
        }
        BuffOverlayMode::TargetUnavailable => ("等待游戏窗口".into(), Vec::new(), 1),
        _ => return Err("不支持的悬浮窗预览状态".into()),
    };
    resize_overlay_for_rows(app, rows);
    ensure_overlay_visible(app);
    emit_overlay(
        app,
        BuffOverlayState {
            mode,
            message,
            items,
            emitted_at_unix_ms,
            editable: true,
            color_scheme: overlay_color_scheme(app),
        },
    );
    Ok(())
}

fn restore_overlay_after_edit(app: &AppHandle, snapshot: &BuffAssistantState) {
    if !snapshot.is_monitoring {
        hide_overlay(app);
        return;
    }
    if snapshot.activity == BuffAssistantActivity::TargetUnavailable {
        show_target_unavailable_overlay(app);
    } else {
        refresh_active_overlay(app);
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

pub(crate) fn handle_detection_batch(
    app: &AppHandle,
    purpose: CapturePurpose,
    detections: &[RuntimeDetection],
    emit_metric: bool,
) {
    let state = app.state::<BuffAssistant>();
    if purpose == CapturePurpose::Test {
        {
            let mut inner = state.lock();
            if inner.capture_purpose != Some(CapturePurpose::Test) {
                return;
            }
            for detection in detections {
                if let Some(runtime) = inner.listeners.get_mut(&detection.listener_id) {
                    runtime.last_confidence = detection.confidence;
                    runtime.activity = BuffAssistantActivity::Testing;
                }
            }
        }
        if emit_metric {
            let _ = app.emit(
                "buff-assistant-metric",
                BuffMetricBatch::from_detections(detections),
            );
        }
        return;
    }
    if purpose != CapturePurpose::Monitor {
        return;
    }

    let (actions, snapshot) = {
        let mut inner = state.lock();
        if inner.capture_purpose != Some(CapturePurpose::Monitor) || !inner.monitor_requested {
            return;
        }
        let now = Instant::now();
        let mut actions = Vec::new();
        for detection in detections {
            let Some(listener_index) = inner
                .config
                .listeners
                .iter()
                .position(|listener| listener.id == detection.listener_id && listener.enabled)
            else {
                continue;
            };
            let Some(runtime) = inner.listeners.get_mut(&detection.listener_id) else {
                continue;
            };
            runtime.last_confidence = detection.confidence;
            let next_actions = runtime.timeline.update_with_detected_at(
                now,
                detection.present,
                detection.absence_confirmed,
                detection.detected_at,
            );
            runtime.activity = timeline_activity(runtime.timeline.phase());
            runtime.expected_at_unix_ms = runtime.timeline.expected_at().map(|expected| {
                now_millis() + expected.saturating_duration_since(now).as_millis() as i64
            });
            if !next_actions.is_empty() {
                let listener = &inner.config.listeners[listener_index];
                let name = listener.name.clone();
                let sound = listener.settings.sound.clone();
                for action in next_actions {
                    actions.push((name.clone(), sound.clone(), action));
                }
            }
        }
        inner.activity = aggregate_activity(&inner);
        let snapshot = (!actions.is_empty()).then(|| snapshot_from_runtime(&inner));
        (actions, snapshot)
    };
    if emit_metric {
        let _ = app.emit(
            "buff-assistant-metric",
            BuffMetricBatch::from_detections(detections),
        );
    }
    if actions.is_empty() {
        return;
    }
    emit_state(app, snapshot.as_ref().unwrap());
    for (listener_name, sound, action) in actions {
        match action {
            TimelineAction::Triggered => {
                if sound.trigger_enabled {
                    play_configured_sound(&state, BuffSoundCue::Triggered, &sound);
                }
                emit_execution_log(
                    app,
                    &format!("{listener_name}：真实触发已确认，已校准倒计时"),
                );
            }
            TimelineAction::PrewarnThree => {
                if sound.prewarn_three_enabled {
                    play_configured_sound(&state, BuffSoundCue::PrewarnThree, &sound);
                }
                emit_execution_log(app, &format!("{listener_name}：倒计时剩余 3 秒"));
            }
            TimelineAction::PrewarnTwo => {
                if sound.prewarn_two_enabled {
                    play_configured_sound(&state, BuffSoundCue::PrewarnTwo, &sound);
                }
                emit_execution_log(app, &format!("{listener_name}：倒计时剩余 2 秒"));
            }
            TimelineAction::PrewarnOne => {
                if sound.prewarn_one_enabled {
                    play_configured_sound(&state, BuffSoundCue::PrewarnOne, &sound);
                }
                emit_execution_log(app, &format!("{listener_name}：倒计时剩余 1 秒"));
            }
            TimelineAction::ConfirmationPending => {
                emit_execution_log(
                    app,
                    &format!("{listener_name}：倒计时结束，正在宽限期内等待确认"),
                );
            }
            TimelineAction::Reset => {
                emit_execution_log(app, &format!("{listener_name}：截止点未确认，时间轴已重置"));
            }
        }
    }
    refresh_active_overlay(app);
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
            reset_listener_timelines(&mut inner);
            inner.activity = BuffAssistantActivity::TargetUnavailable;
            inner.last_error = Some("游戏窗口捕获已中断，正在重新连接".into());
            Some(inner.reconnect_generation)
        } else {
            inner.activity = BuffAssistantActivity::Stopped;
            for runtime in inner.listeners.values_mut() {
                if runtime.activity == BuffAssistantActivity::Testing {
                    runtime.activity = BuffAssistantActivity::Stopped;
                }
            }
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
        for runtime in inner.listeners.values_mut() {
            runtime.timeline.stop();
            runtime.activity = BuffAssistantActivity::Error;
            runtime.expected_at_unix_ms = None;
            runtime.last_error = Some(error.clone());
        }
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
    let (flags, _) = capture_flags(app, CapturePurpose::Monitor, None)?;
    let outcome = start_runtime_capture(window, flags)?;
    let control = outcome.value;
    let mut rejected = None;
    {
        let mut inner = state.lock();
        if !inner.monitor_requested || inner.reconnect_generation != generation {
            rejected = Some(control);
        } else {
            inner.capture = Some(control);
            inner.capture_purpose = Some(CapturePurpose::Monitor);
            inner.last_frame_at = Some(Instant::now());
            let enabled = inner
                .config
                .listeners
                .iter()
                .filter(|listener| listener.enabled && listener.template.is_some())
                .map(|listener| (listener.id.clone(), listener.settings.clone()))
                .collect::<Vec<_>>();
            for (id, settings) in enabled {
                if let Some(runtime) = inner.listeners.get_mut(&id) {
                    runtime
                        .timeline
                        .start_waiting_with_grace(settings.cycle_ms, settings.deadline_grace_ms);
                    runtime.activity = BuffAssistantActivity::Waiting;
                    runtime.expected_at_unix_ms = None;
                }
            }
            inner.activity = BuffAssistantActivity::Waiting;
            inner.last_error = None;
            update_capture_border_notice(&mut inner, outcome.used_border_fallback);
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
    listener_id: Option<&str>,
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
    let listeners = inner
        .config
        .listeners
        .iter()
        .filter(|listener| {
            listener.template.is_some()
                && (purpose == CapturePurpose::Test || listener.enabled)
                && listener_id.is_none_or(|id| listener.id == id)
        })
        .map(|listener| {
            let summary = listener.template.as_ref().unwrap();
            Ok(RuntimeListenerFlags {
                id: listener.id.clone(),
                template: storage::load_template(&inner.storage_directory, summary)?,
                threshold: listener.settings.threshold,
                confirm_frames: listener.settings.confirm_frames,
                missing_frames: listener.settings.missing_frames,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if listeners.is_empty() {
        return Err("没有可用的监听图标模板".into());
    }
    Ok((
        RuntimeCaptureFlags {
            app: app.clone(),
            purpose,
            region,
            listeners,
            reference_width: target.reference_width,
            reference_height: target.reference_height,
            show_system_border: inner.config.settings.capture.show_system_border,
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
                reset_listener_timelines(&mut inner);
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
        inner.last_frame_at = None;
        reset_listener_timelines(&mut inner);
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

fn play_configured_sound(
    state: &BuffAssistant,
    cue: BuffSoundCue,
    sound: &model::BuffSoundSettings,
) {
    let resolved = {
        let inner = state.lock();
        resolve_sound_source(&inner, cue, sound.source(cue)).unwrap_or(ResolvedSoundSource::Sine)
    };
    state.audio.play(cue, resolved, sound.volume);
}

fn preload_monitor_sounds(state: &BuffAssistant) {
    let paths = {
        let inner = state.lock();
        inner
            .config
            .listeners
            .iter()
            .filter(|listener| listener.enabled && listener.template.is_some())
            .flat_map(|listener| {
                let sound = &listener.settings.sound;
                [
                    (sound.trigger_enabled, BuffSoundCue::Triggered),
                    (sound.prewarn_three_enabled, BuffSoundCue::PrewarnThree),
                    (sound.prewarn_two_enabled, BuffSoundCue::PrewarnTwo),
                    (sound.prewarn_one_enabled, BuffSoundCue::PrewarnOne),
                ]
                .into_iter()
                .filter_map(|(enabled, cue)| {
                    enabled
                        .then(|| resolve_sound_source(&inner, cue, sound.source(cue)).ok())
                        .flatten()
                })
                .filter_map(|source| match source {
                    ResolvedSoundSource::Wav(path) => Some(path),
                    ResolvedSoundSource::Sine => None,
                })
            })
            .collect::<Vec<_>>()
    };
    state.audio.preload(paths);
}

fn resolve_sound_source(
    inner: &RuntimeData,
    cue: BuffSoundCue,
    source: &BuffSoundSource,
) -> Result<ResolvedSoundSource, String> {
    match source {
        BuffSoundSource::Sine => Ok(ResolvedSoundSource::Sine),
        BuffSoundSource::Template { template_id } => {
            storage::template_sound_path(&inner.sound_templates, template_id, cue)
                .map(ResolvedSoundSource::Wav)
                .ok_or_else(|| format!("提示音模板不存在：{template_id}"))
        }
        BuffSoundSource::Custom { asset_id, .. } => {
            let path = storage::custom_sound_path(&inner.storage_directory, asset_id)?;
            if path.is_file() {
                Ok(ResolvedSoundSource::Wav(path))
            } else {
                Err("自定义提示音文件不存在，请重新上传".into())
            }
        }
    }
}

fn repair_missing_sound_sources(
    directory: &std::path::Path,
    templates: &[storage::SoundTemplate],
    config: &mut BuffAssistantConfig,
    notices: &mut Vec<String>,
) -> bool {
    let mut repaired = false;
    for listener in &mut config.listeners {
        for cue in [
            BuffSoundCue::Triggered,
            BuffSoundCue::PrewarnThree,
            BuffSoundCue::PrewarnTwo,
            BuffSoundCue::PrewarnOne,
        ] {
            let valid = match listener.settings.sound.source(cue) {
                BuffSoundSource::Sine => true,
                BuffSoundSource::Template { template_id } => templates
                    .iter()
                    .any(|template| template.summary.id == *template_id),
                BuffSoundSource::Custom { asset_id, .. } => {
                    storage::custom_sound_path(directory, asset_id).is_ok_and(|path| path.is_file())
                }
            };
            if !valid {
                *listener.settings.sound.source_mut(cue) = BuffSoundSource::Sine;
                repaired = true;
                notices.push(format!(
                    "{}的{}不可用，已恢复为正弦波",
                    listener.name,
                    sound_cue_label(cue)
                ));
            }
        }
    }
    repaired
}

fn sound_cue_label(cue: BuffSoundCue) -> &'static str {
    match cue {
        BuffSoundCue::Triggered => "真实触发确认音",
        BuffSoundCue::PrewarnThree => "倒计时 3 秒提示音",
        BuffSoundCue::PrewarnTwo => "倒计时 2 秒提示音",
        BuffSoundCue::PrewarnOne => "倒计时 1 秒提示音",
    }
}

fn sound_cue_id(cue: BuffSoundCue) -> &'static str {
    match cue {
        BuffSoundCue::Triggered => "triggered",
        BuffSoundCue::PrewarnThree => "prewarn-three",
        BuffSoundCue::PrewarnTwo => "prewarn-two",
        BuffSoundCue::PrewarnOne => "prewarn-one",
    }
}

#[cfg(target_os = "windows")]
fn open_fixed_url(url: &str) -> Result<(), String> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let operation = OsStr::new("open")
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let url = OsStr::new(url)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            url.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err("无法打开 TTS Online，请检查系统默认浏览器设置。".into())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn open_fixed_url(_url: &str) -> Result<(), String> {
    Err("当前系统不支持打开 TTS Online。".into())
}

fn snapshot_from_runtime(inner: &RuntimeData) -> BuffAssistantState {
    let listeners = inner
        .config
        .listeners
        .iter()
        .map(|listener| {
            let runtime = inner.listeners.get(&listener.id);
            BuffListenerRuntimeState {
                id: listener.id.clone(),
                activity: runtime
                    .map(|runtime| runtime.activity)
                    .unwrap_or(BuffAssistantActivity::Stopped),
                expected_at_unix_ms: runtime.and_then(|runtime| runtime.expected_at_unix_ms),
                last_confidence: runtime
                    .map(|runtime| runtime.last_confidence)
                    .unwrap_or(0.0),
                last_error: runtime.and_then(|runtime| runtime.last_error.clone()),
            }
        })
        .collect();
    BuffAssistantState {
        config: inner.config.clone(),
        activity: inner.activity,
        is_monitoring: inner.monitor_requested,
        listeners,
        last_error: inner.last_error.clone(),
        capture_border_supported: inner.capture_border_supported,
        capture_border_notice: inner.capture_border_notice.clone(),
    }
}

fn listener_runtime_map(config: &BuffAssistantConfig) -> HashMap<String, ListenerRuntime> {
    config
        .listeners
        .iter()
        .map(|listener| {
            (
                listener.id.clone(),
                ListenerRuntime::new(&listener.settings),
            )
        })
        .collect()
}

fn ensure_configuration_unlocked(state: &BuffAssistant) -> Result<(), String> {
    let inner = state.lock();
    if inner.monitor_requested || inner.capture_purpose.is_some() {
        Err("请先停止监控或实时测试，再修改监听配置".into())
    } else {
        Ok(())
    }
}

fn validate_listener_name(
    state: &BuffAssistant,
    listener_id: Option<&str>,
    name: &str,
) -> Result<String, String> {
    let name = name.trim().chars().take(20).collect::<String>();
    if name.is_empty() {
        return Err("监听项名称不能为空".into());
    }
    let duplicate = state.lock().config.listeners.iter().any(|listener| {
        Some(listener.id.as_str()) != listener_id && listener.name.eq_ignore_ascii_case(&name)
    });
    if duplicate {
        Err("监听项名称不能重复".into())
    } else {
        Ok(name)
    }
}

fn reset_listener_timelines(inner: &mut RuntimeData) {
    let enabled = inner
        .config
        .listeners
        .iter()
        .filter(|listener| listener.enabled && listener.template.is_some())
        .map(|listener| listener.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for (id, runtime) in &mut inner.listeners {
        if enabled.contains(id) {
            runtime.timeline.reset_waiting();
            runtime.activity = BuffAssistantActivity::Waiting;
            runtime.expected_at_unix_ms = None;
        } else {
            runtime.timeline.stop();
            runtime.activity = BuffAssistantActivity::Stopped;
            runtime.expected_at_unix_ms = None;
        }
    }
}

fn timeline_activity(phase: TimelinePhase) -> BuffAssistantActivity {
    match phase {
        TimelinePhase::Stopped => BuffAssistantActivity::Stopped,
        TimelinePhase::Waiting => BuffAssistantActivity::Waiting,
        TimelinePhase::Tracking => BuffAssistantActivity::Tracking,
        TimelinePhase::Prewarning => BuffAssistantActivity::Prewarning,
        TimelinePhase::Confirming => BuffAssistantActivity::Confirming,
    }
}

fn aggregate_activity(inner: &RuntimeData) -> BuffAssistantActivity {
    for activity in [
        BuffAssistantActivity::Error,
        BuffAssistantActivity::TargetUnavailable,
        BuffAssistantActivity::Testing,
        BuffAssistantActivity::Confirming,
        BuffAssistantActivity::Prewarning,
        BuffAssistantActivity::Tracking,
        BuffAssistantActivity::Waiting,
    ] {
        if inner
            .listeners
            .values()
            .any(|runtime| runtime.activity == activity)
        {
            return activity;
        }
    }
    BuffAssistantActivity::Stopped
}

fn update_capture_border_notice(inner: &mut RuntimeData, used_border_fallback: bool) {
    inner.capture_border_notice =
        used_border_fallback.then(|| CAPTURE_BORDER_FALLBACK_NOTICE.to_string());
}

fn borderless_access_notice(result: BorderlessCaptureAccessResult) -> Option<&'static str> {
    match result {
        BorderlessCaptureAccessResult::Allowed => None,
        BorderlessCaptureAccessResult::Unsupported => {
            Some("当前 Windows 版本不支持隐藏系统捕获黄色边框")
        }
        BorderlessCaptureAccessResult::DeniedByUser => {
            Some("未获得隐藏系统捕获边框的用户授权，已继续显示黄色边框")
        }
        BorderlessCaptureAccessResult::DeniedBySystem => {
            Some("Windows 未允许隐藏系统捕获边框，已继续显示黄色边框")
        }
        BorderlessCaptureAccessResult::NotDeclared => {
            Some("当前应用安装方式不允许隐藏系统捕获边框")
        }
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

fn refresh_active_overlay(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let items = {
        let mut inner = state.lock();
        if inner.overlay_editing {
            return;
        }
        inner.overlay_generation = inner.overlay_generation.wrapping_add(1);
        inner
            .config
            .listeners
            .iter()
            .filter_map(|listener| {
                if !listener.enabled || listener.template.is_none() {
                    return None;
                }
                let runtime = inner.listeners.get(&listener.id)?;
                let mode = match runtime.activity {
                    BuffAssistantActivity::Waiting => BuffOverlayMode::Waiting,
                    BuffAssistantActivity::Tracking | BuffAssistantActivity::Prewarning => {
                        BuffOverlayMode::Countdown
                    }
                    BuffAssistantActivity::Confirming => BuffOverlayMode::Confirming,
                    _ => return None,
                };
                Some(BuffOverlayItem {
                    listener_id: listener.id.clone(),
                    name: listener.name.clone(),
                    mode,
                    expected_at_unix_ms: runtime.expected_at_unix_ms,
                })
            })
            .collect::<Vec<_>>()
    };
    if items.is_empty() {
        show_waiting_overlay(app);
        return;
    }
    let mode = if items
        .iter()
        .any(|item| item.mode == BuffOverlayMode::Countdown)
    {
        BuffOverlayMode::Countdown
    } else if items
        .iter()
        .any(|item| item.mode == BuffOverlayMode::Confirming)
    {
        BuffOverlayMode::Confirming
    } else {
        BuffOverlayMode::Waiting
    };
    resize_overlay_for_rows(app, items.len());
    ensure_overlay_visible(app);
    emit_overlay(
        app,
        BuffOverlayState {
            mode,
            message: String::new(),
            items,
            emitted_at_unix_ms: now_millis(),
            editable: false,
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
    }
    ensure_overlay_visible(app);
    emit_overlay(
        app,
        BuffOverlayState {
            mode,
            message: message.into(),
            items: expected_at_unix_ms
                .map(|expected_at_unix_ms| BuffOverlayItem {
                    listener_id: "transient".into(),
                    name: message.into(),
                    mode,
                    expected_at_unix_ms: Some(expected_at_unix_ms),
                })
                .into_iter()
                .collect(),
            emitted_at_unix_ms: now_millis(),
            editable: false,
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
    let items = {
        let inner = state.lock();
        if !inner.monitor_requested || inner.overlay_editing {
            None
        } else {
            Some(
                inner
                    .config
                    .listeners
                    .iter()
                    .filter(|listener| listener.enabled && listener.template.is_some())
                    .map(|listener| BuffOverlayItem {
                        listener_id: listener.id.clone(),
                        name: listener.name.clone(),
                        mode: BuffOverlayMode::Waiting,
                        expected_at_unix_ms: None,
                    })
                    .collect::<Vec<_>>(),
            )
        }
    };
    let Some(items) = items else {
        hide_overlay(app);
        return;
    };
    let rows = items.len().max(1);
    resize_overlay_for_rows(app, rows);
    ensure_overlay_visible(app);
    emit_overlay(
        app,
        BuffOverlayState {
            mode: BuffOverlayMode::Waiting,
            message: if items.is_empty() {
                "等待监听图标".into()
            } else {
                String::new()
            },
            items,
            emitted_at_unix_ms: now_millis(),
            editable: false,
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
    resize_overlay_for_rows(app, 1);
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.set_focusable(false);
    }
    ensure_overlay_visible(app);
    emit_overlay(
        app,
        BuffOverlayState {
            mode: BuffOverlayMode::TargetUnavailable,
            message: "等待游戏窗口".into(),
            items: Vec::new(),
            emitted_at_unix_ms: now_millis(),
            editable: false,
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
            items: Vec::new(),
            emitted_at_unix_ms: now_millis(),
            editable: false,
            color_scheme: overlay_color_scheme(app),
        },
    );
    let should_hide = {
        let state = app.state::<BuffAssistant>();
        let mut inner = state.lock();
        inner.overlay_window.mark_hidden()
    };
    if should_hide && let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.hide();
    }
}

fn ensure_overlay_visible(app: &AppHandle) {
    let should_show = {
        let state = app.state::<BuffAssistant>();
        let mut inner = state.lock();
        inner.overlay_window.mark_visible()
    };
    if should_show && let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.show();
    }
}

fn apply_overlay_geometry(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let settings = {
        let mut inner = state.lock();
        let size = (
            inner.config.settings.overlay.width,
            inner.config.settings.overlay.height,
        );
        inner.overlay_window.update_size(size, None);
        inner.config.settings.overlay.clone()
    };
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_position(PhysicalPosition::new(settings.x, settings.y));
        let _ = overlay.set_size(LogicalSize::new(
            f64::from(settings.width),
            f64::from(settings.height),
        ));
    }
}

fn resize_overlay_for_rows(app: &AppHandle, rows: usize) {
    let state = app.state::<BuffAssistant>();
    let desired = {
        let mut inner = state.lock();
        if inner.overlay_editing {
            return;
        }
        let width = inner.config.settings.overlay.width;
        let height = configured_overlay_height(inner.config.settings.overlay.height, rows) as u32;
        if !inner
            .overlay_window
            .update_size((width, height), Some(rows))
        {
            return;
        }
        inner.overlay_window.size = Some((width, height));
        inner.overlay_window.rows = Some(rows);
        (width, height)
    };
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.set_size(LogicalSize::new(f64::from(desired.0), f64::from(desired.1)));
    }
}

fn configured_overlay_height(configured_height: u32, rows: usize) -> f64 {
    if configured_height == DEFAULT_OVERLAY_HEIGHT {
        overlay_height_for_rows(rows)
    } else {
        f64::from(configured_height)
    }
}

fn overlay_height_for_rows(rows: usize) -> f64 {
    let base_height = 28.0 + rows.max(1) as f64 * 52.0;
    base_height.clamp(f64::from(MIN_OVERLAY_HEIGHT), f64::from(MAX_OVERLAY_HEIGHT))
}

fn apply_overlay_capture_protection(app: &AppHandle) -> Result<(), String> {
    let exclude_from_capture = app
        .state::<BuffAssistant>()
        .lock()
        .config
        .settings
        .overlay
        .exclude_from_capture;
    let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) else {
        return Ok(());
    };
    overlay
        .set_content_protected(exclude_from_capture)
        .map_err(|error| format!("设置已保存，但无法应用悬浮窗录屏排除：{error}"))
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

fn crop_saved_template(
    source: &DynamicImage,
    crop: NormalizedRect,
) -> Result<DynamicImage, String> {
    let (x, y, end_x, end_y) = crop.pixel_bounds(source.width(), source.height());
    let width = end_x - x;
    let height = end_y - y;
    if width < 8 || height < 8 {
        return Err("模板区域过小，请重新框选图标".into());
    }
    Ok(source.crop_imm(x, y, width, height))
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuffMetric {
    listener_id: String,
    confidence: f32,
    present: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuffMetricBatch {
    metrics: Vec<BuffMetric>,
}

impl BuffMetricBatch {
    fn from_detections(detections: &[RuntimeDetection]) -> Self {
        Self {
            metrics: detections
                .iter()
                .map(|detection| BuffMetric {
                    listener_id: detection.listener_id.clone(),
                    confidence: detection.confidence,
                    present: detection.present,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use image::DynamicImage;

    use super::{
        DEFAULT_OVERLAY_HEIGHT, NormalizedRect, OverlayWindowCache, configured_overlay_height,
        crop_saved_template, crop_template_from_preview, overlay_height_for_rows,
    };

    #[test]
    fn overlay_window_cache_only_requests_native_changes_when_state_changes() {
        let mut cache = OverlayWindowCache::default();

        assert!(cache.mark_visible());
        assert!(!cache.mark_visible());
        assert!(cache.update_size((330, 92), Some(1)));
        assert!(!cache.update_size((330, 92), Some(1)));
        assert!(cache.update_size((330, 132), Some(2)));
        assert!(cache.mark_hidden());
        assert!(!cache.mark_hidden());

        cache.invalidate_size();
        assert!(cache.update_size((330, 132), Some(2)));
    }

    #[test]
    fn overlay_row_height_does_not_depend_on_width() {
        assert_eq!(overlay_height_for_rows(1), 80.0);
        assert_eq!(overlay_height_for_rows(2), 132.0);
        assert_eq!(configured_overlay_height(70, 2), 70.0);
        assert_eq!(configured_overlay_height(DEFAULT_OVERLAY_HEIGHT, 2), 132.0);
    }

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

    #[test]
    fn saved_template_can_be_cropped_again() {
        let template = DynamicImage::new_rgba8(40, 30);
        let cropped = crop_saved_template(
            &template,
            NormalizedRect {
                x: 0.25,
                y: 0.2,
                width: 0.5,
                height: 0.6,
            },
        )
        .expect("saved template crop should succeed");

        assert_eq!((cropped.width(), cropped.height()), (20, 18));
    }
}

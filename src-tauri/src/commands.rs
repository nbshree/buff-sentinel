use std::{
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    input,
    model::{
        AppearancePatch, Hotkeys, KeyModifier, LoopMode, MacroProfile, MacroState, Point,
        PointAction, PointPatch, SettingsPatch, clamp_f64, create_id, default_settings,
        format_key_step, key_step_conflicts_with_hotkey, normalize_hotkey, normalize_key,
        now_millis, patch_appearance, sanitize_modifier_list, sanitize_profile,
        sanitize_profile_name, truncate_chars, validate_hotkeys, virtual_key_code,
    },
    shortcuts,
    state::{AppState, apply_active_profile, apply_appearance, can_edit_flow, emit_state},
    store,
};

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> MacroState {
    state.snapshot()
}

#[tauri::command]
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
pub fn update_appearance(
    app: AppHandle,
    state: State<'_, AppState>,
    appearance: AppearancePatch,
) -> Result<MacroState, String> {
    let snapshot = {
        let mut inner = state.lock();
        let next = patch_appearance(&inner.store.appearance, &appearance);
        let mut store_snapshot = inner.store.clone();
        store_snapshot.appearance = next.clone();

        store::save_profiles(&inner.profile_file, &store_snapshot)?;
        apply_appearance(&mut inner, next);
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn start_recording(app: AppHandle, state: State<'_, AppState>) -> MacroState {
    let unavailable = {
        let mut inner = state.lock();
        if inner.state.is_running || inner.game_activity {
            true
        } else {
            inner.is_capturing_key = false;
            inner.state.is_recording = true;
            false
        }
    };
    if unavailable {
        state.log(&app, "宏执行或游戏录制任务进行中，不能开始坐标录制")
    } else {
        state.log(&app, "开始录制坐标，按采集热键记录当前鼠标位置")
    }
}

#[tauri::command]
pub fn stop_recording(app: AppHandle, state: State<'_, AppState>) -> MacroState {
    state.lock().state.is_recording = false;
    state.log(&app, "停止录制坐标")
}

#[tauri::command]
pub fn start_run(app: AppHandle) -> MacroState {
    start_run_internal(&app)
}

#[tauri::command]
pub fn stop_run(app: AppHandle) -> MacroState {
    stop_run_internal(&app)
}

#[tauri::command]
pub fn clear_logs(app: AppHandle, state: State<'_, AppState>) -> MacroState {
    let snapshot = {
        let mut inner = state.lock();
        inner.state.logs.clear();
        inner.state.clone()
    };
    emit_state(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn remove_point(app: AppHandle, state: State<'_, AppState>, id: String) -> MacroState {
    {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        inner.state.points.retain(|point| point.id != id);
    }
    finish_flow_edit(&app, &state, "删除一个坐标")
}

#[tauri::command]
pub fn clear_points(app: AppHandle, state: State<'_, AppState>) -> MacroState {
    {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        inner.state.points.clear();
        inner.state.current_index = -1;
    }
    finish_flow_edit(&app, &state, "清空流程步骤")
}

#[tauri::command]
pub fn add_key_point(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    modifiers: Vec<KeyModifier>,
) -> MacroState {
    let normalized_key = normalize_key(&key);
    let modifiers = sanitize_modifier_list(modifiers);
    let outcome = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        if virtual_key_code(&normalized_key).is_none() {
            Err(format!(
                "无法添加键盘步骤：不支持的按键 {}",
                if key.is_empty() { "空" } else { &key }
            ))
        } else if key_step_conflicts_with_hotkey(
            &normalized_key,
            &modifiers,
            &inner.state.settings.hotkeys,
        ) {
            Err(format!(
                "无法添加键盘步骤：{} 与应用全局热键冲突",
                format_key_step(&normalized_key, &modifiers)
            ))
        } else {
            let order = inner.state.points.len() + 1;
            let delay_seconds = inner.state.settings.click_interval_seconds;
            inner.state.points.push(Point {
                id: create_id(),
                label: format!("按键 {order}"),
                action: PointAction::Key,
                enabled: true,
                x: 0,
                y: 0,
                key: normalized_key.clone(),
                modifiers: modifiers.clone(),
                delay_seconds,
                created_at: now_millis(),
            });
            Ok(order)
        }
    };

    match outcome {
        Err(message) => state.log(&app, message),
        Ok(order) => finish_flow_edit(
            &app,
            &state,
            format!(
                "添加键盘步骤 #{order}：{}",
                format_key_step(&normalized_key, &modifiers)
            ),
        ),
    }
}

#[tauri::command]
pub fn set_key_capture(state: State<'_, AppState>, enabled: bool) {
    let mut inner = state.lock();
    inner.is_capturing_key = can_enable_key_capture(
        enabled,
        inner.state.is_running,
        inner.state.is_recording,
        inner.game_activity,
    );
}

#[tauri::command]
pub fn sync_point_delays(app: AppHandle, state: State<'_, AppState>) -> MacroState {
    let (count, delay_seconds) = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        let delay_seconds = clamp_f64(
            inner.state.settings.click_interval_seconds,
            inner.state.settings.click_interval_seconds,
            0.1,
            3600.0,
        );
        for point in &mut inner.state.points {
            point.delay_seconds = delay_seconds;
        }
        (inner.state.points.len(), delay_seconds)
    };
    finish_flow_edit(
        &app,
        &state,
        format!("已同步 {count} 个坐标等待时间为 {delay_seconds}s"),
    )
}

#[tauri::command]
pub fn update_point(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    patch: PointPatch,
) -> MacroState {
    let mut invalid_key = false;
    let mut invalid_action = false;
    {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        let hotkeys = inner.state.settings.hotkeys.clone();
        if let Some(point) = inner.state.points.iter_mut().find(|point| point.id == id) {
            let action = patch.action.unwrap_or(point.action);
            if !can_change_point_action(point.action, action) {
                invalid_action = true;
            } else {
                let key = patch
                    .key
                    .as_deref()
                    .map(normalize_key)
                    .unwrap_or_else(|| point.key.clone());
                let modifiers = patch
                    .modifiers
                    .map(sanitize_modifier_list)
                    .unwrap_or_else(|| point.modifiers.clone());
                if action == PointAction::Key
                    && (virtual_key_code(&key).is_none()
                        || key_step_conflicts_with_hotkey(&key, &modifiers, &hotkeys))
                {
                    invalid_key = true;
                } else {
                    if let Some(label) = patch.label {
                        point.label = truncate_chars(&label, 60);
                    }
                    point.action = action;
                    if let Some(enabled) = patch.enabled {
                        point.enabled = enabled;
                    }
                    if let Some(x) = patch.x {
                        point.x =
                            clamp_f64(x, point.x as f64, -100_000.0, 100_000.0).round() as i32;
                    }
                    if let Some(y) = patch.y {
                        point.y =
                            clamp_f64(y, point.y as f64, -100_000.0, 100_000.0).round() as i32;
                    }
                    point.key = key;
                    point.modifiers = modifiers;
                    if let Some(delay_seconds) = patch.delay_seconds {
                        point.delay_seconds =
                            clamp_f64(delay_seconds, point.delay_seconds, 0.1, 3600.0);
                    }
                }
            }
        }
    }
    if invalid_action {
        state.log(&app, "更新步骤失败：键盘步骤与鼠标步骤不能互相转换")
    } else if invalid_key {
        state.log(&app, "更新键盘步骤失败：按键无效或与应用全局热键冲突")
    } else {
        finish_flow_edit(&app, &state, "更新步骤")
    }
}

#[tauri::command]
pub fn move_point(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    direction: String,
) -> MacroState {
    let changed = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        let Some(index) = inner.state.points.iter().position(|point| point.id == id) else {
            return inner.state.clone();
        };
        let target = match direction.as_str() {
            "up" if index > 0 => Some(index - 1),
            "down" if index + 1 < inner.state.points.len() => Some(index + 1),
            _ => None,
        };
        target
            .map(|target| reorder_points(&mut inner.state.points, &id, target))
            .unwrap_or(false)
    };
    if changed {
        finish_flow_edit(&app, &state, "调整坐标顺序")
    } else {
        state.snapshot()
    }
}

#[tauri::command]
pub fn reorder_point(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    target_index: f64,
) -> MacroState {
    let changed = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        if inner.state.points.is_empty() {
            false
        } else {
            let target = clamp_f64(
                target_index.round(),
                target_index,
                0.0,
                (inner.state.points.len() - 1) as f64,
            ) as usize;
            reorder_points(&mut inner.state.points, &id, target)
        }
    };
    if changed {
        finish_flow_edit(&app, &state, "拖拽调整坐标顺序")
    } else {
        state.snapshot()
    }
}

#[tauri::command]
pub fn test_point(app: AppHandle, state: State<'_, AppState>, id: String) -> MacroState {
    let point = {
        let inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        inner
            .state
            .points
            .iter()
            .find(|point| point.id == id && point.action != PointAction::Key)
            .cloned()
    };
    let Some(point) = point else {
        return state.snapshot();
    };
    let (result, action_label) = match point.action {
        PointAction::Click => (input::click(point.x, point.y), "点击"),
        PointAction::DoubleClick => (input::double_click(point.x, point.y), "双击"),
        PointAction::Key => unreachable!("keyboard points were filtered out"),
    };
    match result {
        Ok(()) => state.log(
            &app,
            format!(
                "测试{action_label} {} ({}, {})",
                unnamed_label(&point.label),
                point.x,
                point.y
            ),
        ),
        Err(error) => state.log(&app, format!("测试{action_label}失败：{error}")),
    }
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: SettingsPatch,
) -> MacroState {
    let next_hotkeys = {
        let inner = state.lock();
        if !can_edit_flow(&inner) {
            drop(inner);
            return state.log(&app, "录制或执行中不能修改配置");
        }
        let current = &inner.state.settings.hotkeys;
        let patch = settings.hotkeys.as_ref();
        Hotkeys {
            capture: patch
                .and_then(|hotkeys| hotkeys.capture.as_deref())
                .map(normalize_hotkey)
                .unwrap_or_else(|| current.capture.clone()),
            start: patch
                .and_then(|hotkeys| hotkeys.start.as_deref())
                .map(normalize_hotkey)
                .unwrap_or_else(|| current.start.clone()),
            stop: patch
                .and_then(|hotkeys| hotkeys.stop.as_deref())
                .map(normalize_hotkey)
                .unwrap_or_else(|| current.stop.clone()),
        }
    };

    let hotkey_errors = validate_hotkeys(&next_hotkeys);
    if !hotkey_errors.is_empty() {
        state.lock().state.hotkey_errors = hotkey_errors;
        return state.log(&app, "配置未保存：热键存在冲突");
    }

    {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        let current = &mut inner.state.settings;
        if let Some(value) = settings.click_interval_seconds {
            current.click_interval_seconds =
                clamp_f64(value, current.click_interval_seconds, 0.1, 3600.0);
        }
        if let Some(value) = settings.loop_interval_seconds {
            current.loop_interval_seconds =
                clamp_f64(value, current.loop_interval_seconds, 0.0, 3600.0);
        }
        if let Some(value) = settings.start_delay_seconds {
            current.start_delay_seconds = clamp_f64(value, current.start_delay_seconds, 0.0, 60.0);
        }
        if let Some(value) = settings.loop_mode {
            current.loop_mode = value;
        }
        if let Some(value) = settings.loop_count {
            current.loop_count =
                clamp_f64(value, current.loop_count as f64, 1.0, 9999.0).round() as u32;
        }
        current.hotkeys = next_hotkeys;
    }

    shortcuts::register_shortcuts(&app);
    state.save_active_profile(&app);
    state.log(&app, "更新配置")
}

#[tauri::command]
pub fn create_profile(app: AppHandle, state: State<'_, AppState>, name: String) -> MacroState {
    if !can_edit(&state) {
        return state.snapshot();
    }
    state.save_active_profile(&app);

    let (profile_name, store_snapshot, path) = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        let now = now_millis();
        let mut settings = default_settings();
        // New profiles inherit the current workspace hotkeys so switching profiles does not
        // unexpectedly change the user's global shortcut layout.
        settings.hotkeys = inner.state.settings.hotkeys.clone();
        let profile = MacroProfile {
            id: create_id(),
            name: sanitize_profile_name(
                Some(&serde_json::Value::String(name)),
                &format!("方案 {}", inner.store.profiles.len() + 1),
            ),
            points: Vec::new(),
            settings,
            created_at: now,
            updated_at: now,
        };
        let profile_name = profile.name.clone();
        inner.store.active_profile_id = profile.id.clone();
        inner.store.profiles.push(profile);
        apply_active_profile(&mut inner);
        (
            profile_name,
            inner.store.clone(),
            inner.profile_file.clone(),
        )
    };
    state.persist_store(&app, store_snapshot, path);
    shortcuts::register_shortcuts(&app);
    state.log(&app, format!("已新建并切换到方案：{profile_name}"))
}

#[tauri::command]
pub fn switch_profile(app: AppHandle, state: State<'_, AppState>, id: String) -> MacroState {
    if !can_edit(&state) {
        return state.snapshot();
    }
    let should_switch = {
        let inner = state.lock();
        inner.state.active_profile_id != id
            && inner.store.profiles.iter().any(|profile| profile.id == id)
    };
    if !should_switch {
        return state.snapshot();
    }
    state.save_active_profile(&app);

    let (name, store_snapshot, path) = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        inner.store.active_profile_id = id;
        apply_active_profile(&mut inner);
        let name = inner
            .store
            .profiles
            .iter()
            .find(|profile| profile.id == inner.store.active_profile_id)
            .map(|profile| profile.name.clone())
            .unwrap_or_default();
        (name, inner.store.clone(), inner.profile_file.clone())
    };
    state.persist_store(&app, store_snapshot, path);
    shortcuts::register_shortcuts(&app);
    state.log(&app, format!("已切换方案：{name}"))
}

#[tauri::command]
pub fn rename_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> MacroState {
    let result = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        let Some(profile) = inner
            .store
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
        else {
            return inner.state.clone();
        };
        let fallback = profile.name.clone();
        profile.name = sanitize_profile_name(Some(&serde_json::Value::String(name)), &fallback);
        profile.updated_at = now_millis();
        let profile_name = profile.name.clone();
        apply_active_profile(&mut inner);
        (
            profile_name,
            inner.store.clone(),
            inner.profile_file.clone(),
        )
    };
    state.persist_store(&app, result.1, result.2);
    state.log(&app, format!("已重命名方案：{}", result.0))
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, state: State<'_, AppState>, id: String) -> MacroState {
    let fallback_profile = {
        let inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        if inner.store.profiles.len() <= 1 {
            drop(inner);
            return state.log(&app, "至少需要保留一个方案");
        }
        if !inner.store.profiles.iter().any(|profile| profile.id == id) {
            return inner.state.clone();
        }
        (inner.store.active_profile_id == id)
            .then(|| {
                inner
                    .store
                    .profiles
                    .iter()
                    .find(|profile| profile.id != id)
                    .cloned()
            })
            .flatten()
    };
    let result = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        if inner.store.profiles.len() <= 1 {
            drop(inner);
            return state.log(&app, "至少需要保留一个方案");
        }
        if !inner.store.profiles.iter().any(|profile| profile.id == id) {
            return inner.state.clone();
        }
        let deleted_name = inner
            .store
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| id.clone());
        let next_active_id = if inner.store.active_profile_id == id {
            let Some(fallback_id) = fallback_profile.as_ref().and_then(|profile| {
                inner
                    .store
                    .profiles
                    .iter()
                    .find(|candidate| candidate.id == profile.id && candidate.id != id)
                    .map(|candidate| candidate.id.clone())
            }) else {
                return inner.state.clone();
            };
            Some(fallback_id)
        } else {
            None
        };
        inner.store.profiles.retain(|profile| profile.id != id);
        if let Some(fallback_id) = next_active_id {
            inner.store.active_profile_id = fallback_id;
        }
        apply_active_profile(&mut inner);
        (
            deleted_name,
            inner.store.clone(),
            inner.profile_file.clone(),
        )
    };
    state.persist_store(&app, result.1, result.2);
    shortcuts::register_shortcuts(&app);
    state.log(&app, format!("已删除方案：{}", result.0))
}

#[tauri::command]
pub async fn export_profile(app: AppHandle, id: String) -> MacroState {
    let state = app.state::<AppState>();
    if !can_edit(&state) {
        return state.snapshot();
    }
    state.save_active_profile(&app);
    let profile = {
        let inner = state.lock();
        inner
            .store
            .profiles
            .iter()
            .find(|item| item.id == id)
            .cloned()
    };
    let Some(profile) = profile else {
        return state.snapshot();
    };

    let mut dialog = app
        .dialog()
        .file()
        .set_title("导出方案")
        .set_file_name(format!("{}.json", safe_file_name(&profile.name)))
        .add_filter("JSON", &["json"]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_save_file() else {
        return state.snapshot();
    };
    let path = match path.into_path() {
        Ok(path) => path,
        Err(error) => return state.log(&app, format!("导出失败：{error}")),
    };

    match store::write_profile(&path, &profile) {
        Ok(()) => state.log(&app, format!("已导出方案：{}", profile.name)),
        Err(error) => state.log(&app, format!("导出失败：{error}")),
    }
}

#[tauri::command]
pub async fn import_profile(app: AppHandle) -> MacroState {
    let state = app.state::<AppState>();
    if !can_edit(&state) {
        return state.snapshot();
    }
    state.save_active_profile(&app);

    let mut dialog = app
        .dialog()
        .file()
        .set_title("导入方案")
        .add_filter("JSON", &["json"]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_pick_file() else {
        return state.snapshot();
    };
    let path = match path.into_path() {
        Ok(path) => path,
        Err(error) => return state.log(&app, format!("导入失败：{error}")),
    };
    let fallback_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(DEFAULT_IMPORT_NAME);
    let parsed = match store::read_json(&path) {
        Ok(parsed) => parsed,
        Err(error) => return state.log(&app, format!("导入失败：{error}")),
    };
    let Some(mut imported) = sanitize_profile(&parsed, fallback_name) else {
        return state.log(&app, "导入失败：JSON 不是有效方案");
    };

    let now = now_millis();
    imported.id = create_id();
    imported.name = sanitize_profile_name(
        Some(&serde_json::Value::String(imported.name)),
        fallback_name,
    );
    imported.created_at = now;
    imported.updated_at = now;
    for point in &mut imported.points {
        point.id = create_id();
    }
    let imported_name = imported.name.clone();

    let (store_snapshot, path) = {
        let mut inner = state.lock();
        if !can_edit_flow(&inner) {
            return inner.state.clone();
        }
        inner.store.active_profile_id = imported.id.clone();
        inner.store.profiles.push(imported);
        apply_active_profile(&mut inner);
        (inner.store.clone(), inner.profile_file.clone())
    };
    state.persist_store(&app, store_snapshot, path);
    shortcuts::register_shortcuts(&app);
    state.log(&app, format!("已导入并切换到方案：{imported_name}"))
}

pub(crate) fn capture_point_internal(app: &AppHandle) -> MacroState {
    let state = app.state::<AppState>();
    {
        let inner = state.lock();
        if !inner.state.is_recording || inner.state.is_running {
            return inner.state.clone();
        }
    }
    let (x, y) = match input::get_cursor_position() {
        Ok(position) => position,
        Err(error) => return state.log(app, format!("记录坐标失败：{error}")),
    };
    let order = {
        let mut inner = state.lock();
        if !inner.state.is_recording || inner.state.is_running {
            return inner.state.clone();
        }
        let order = inner.state.points.len() + 1;
        let delay_seconds = inner.state.settings.click_interval_seconds;
        inner.state.points.push(Point {
            id: create_id(),
            label: format!("步骤 {order}"),
            action: PointAction::Click,
            enabled: true,
            x,
            y,
            key: String::new(),
            modifiers: Vec::new(),
            delay_seconds,
            created_at: now_millis(),
        });
        order
    };
    state.save_active_profile(app);
    state.log(app, format!("记录坐标 #{order} ({x}, {y})"))
}

pub(crate) fn start_run_internal(app: &AppHandle) -> MacroState {
    let state = app.state::<AppState>();
    let result = {
        let mut inner = state.lock();
        if let Err(message) = validate_run_start(
            inner.state.is_recording,
            inner.state.is_running,
            inner.game_activity,
            &inner.state.points,
        ) {
            Err(message)
        } else {
            inner.state.is_recording = false;
            inner.state.is_running = true;
            inner.is_capturing_key = false;
            inner.state.current_index = -1;
            inner.state.completed_loops = 0;
            inner.state.countdown_remaining = 0;
            inner.run_token = inner.run_token.wrapping_add(1);
            Ok(inner.run_token)
        }
    };
    let token = match result {
        Ok(token) => token,
        Err(message) => return state.log(app, message),
    };
    let snapshot = state.log(app, "准备执行宏流程");
    let app_handle = app.clone();
    thread::spawn(move || run_macro(app_handle, token));
    snapshot
}

pub(crate) fn stop_run_internal(app: &AppHandle) -> MacroState {
    let state = app.state::<AppState>();
    let stopped = {
        let mut inner = state.lock();
        if !inner.state.is_running {
            false
        } else {
            inner.state.is_running = false;
            inner.state.current_index = -1;
            inner.state.countdown_remaining = 0;
            inner.run_token = inner.run_token.wrapping_add(1);
            true
        }
    };
    if stopped {
        state.log(app, "停止执行宏流程")
    } else {
        state.snapshot()
    }
}

pub(crate) fn stop_macro_workspace_activity_internal(app: &AppHandle) -> MacroState {
    let state = app.state::<AppState>();
    let was_recording = {
        let mut inner = state.lock();
        inner.is_capturing_key = false;
        let was_recording = inner.state.is_recording;
        inner.state.is_recording = false;
        was_recording
    };
    let snapshot = stop_run_internal(app);
    if was_recording {
        state.log(app, "切换工作区，停止录制坐标")
    } else {
        snapshot
    }
}

fn run_macro(app: AppHandle, token: u64) {
    if !run_countdown(&app, token) {
        return;
    }

    while is_current_run(&app, token) {
        let point_indices = {
            let state = app.state::<AppState>();
            let inner = state.lock();
            enabled_point_indices(&inner.state.points)
        };
        for index in point_indices {
            let point = {
                let state = app.state::<AppState>();
                let mut inner = state.lock();
                if !inner.state.is_running || inner.run_token != token {
                    return;
                }
                let Some(point) = inner.state.points.get(index).cloned() else {
                    return;
                };
                inner.state.current_index = index as i32;
                let snapshot = inner.state.clone();
                drop(inner);
                emit_state(&app, &snapshot);
                point
            };

            let result = match point.action {
                PointAction::Key => input::key(&point.key, &point.modifiers),
                PointAction::Click => input::click(point.x, point.y),
                PointAction::DoubleClick => input::double_click(point.x, point.y),
            };
            let state = app.state::<AppState>();
            match result {
                Ok(()) => match point.action {
                    PointAction::Key => {
                        state.log(
                            &app,
                            format!(
                                "按键 #{} {}：{}",
                                index + 1,
                                unnamed_label(&point.label),
                                format_key_step(&point.key, &point.modifiers)
                            ),
                        );
                    }
                    PointAction::Click => {
                        state.log(
                            &app,
                            format!(
                                "点击 #{} {} ({}, {})",
                                index + 1,
                                unnamed_label(&point.label),
                                point.x,
                                point.y
                            ),
                        );
                    }
                    PointAction::DoubleClick => {
                        state.log(
                            &app,
                            format!(
                                "双击 #{} {} ({}, {})",
                                index + 1,
                                unnamed_label(&point.label),
                                point.x,
                                point.y
                            ),
                        );
                    }
                },
                Err(error) => {
                    state.log(&app, format!("步骤失败：{error}"));
                    stop_run_internal(&app);
                    return;
                }
            }

            if !wait_cancelable(&app, Duration::from_secs_f64(point.delay_seconds), token) {
                return;
            }
        }

        let (completed_loops, loop_mode, loop_count, loop_interval, snapshot) = {
            let state = app.state::<AppState>();
            let mut inner = state.lock();
            if !inner.state.is_running || inner.run_token != token {
                return;
            }
            inner.state.completed_loops = inner.state.completed_loops.saturating_add(1);
            inner.state.current_index = -1;
            (
                inner.state.completed_loops,
                inner.state.settings.loop_mode.clone(),
                inner.state.settings.loop_count,
                inner.state.settings.loop_interval_seconds,
                inner.state.clone(),
            )
        };
        emit_state(&app, &snapshot);
        app.state::<AppState>()
            .log(&app, format!("完成第 {completed_loops} 轮"));

        if loop_mode == LoopMode::Count && completed_loops >= loop_count.max(1) {
            finish_run(&app);
            return;
        }
        if !wait_cancelable(&app, Duration::from_secs_f64(loop_interval), token) {
            return;
        }
    }
}

fn run_countdown(app: &AppHandle, token: u64) -> bool {
    let delay = app
        .state::<AppState>()
        .lock()
        .state
        .settings
        .start_delay_seconds
        .round() as u32;
    if delay == 0 {
        return true;
    }

    for remaining in (1..=delay).rev() {
        let snapshot = {
            let state = app.state::<AppState>();
            let mut inner = state.lock();
            if !inner.state.is_running || inner.run_token != token {
                return false;
            }
            inner.state.countdown_remaining = remaining;
            inner.state.clone()
        };
        emit_state(app, &snapshot);
        app.state::<AppState>()
            .log(app, format!("{remaining} 秒后开始执行"));
        if !wait_cancelable(app, Duration::from_secs(1), token) {
            return false;
        }
    }

    let snapshot = {
        let state = app.state::<AppState>();
        let mut inner = state.lock();
        if !inner.state.is_running || inner.run_token != token {
            return false;
        }
        inner.state.countdown_remaining = 0;
        inner.state.clone()
    };
    emit_state(app, &snapshot);
    true
}

fn finish_run(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut inner = state.lock();
        inner.state.is_running = false;
        inner.state.current_index = -1;
        inner.state.countdown_remaining = 0;
        inner.run_token = inner.run_token.wrapping_add(1);
    }
    state.log(app, "执行完成");
}

fn wait_cancelable(app: &AppHandle, duration: Duration, token: u64) -> bool {
    let started_at = Instant::now();
    loop {
        if !is_current_run(app, token) {
            return false;
        }
        let elapsed = started_at.elapsed();
        if elapsed >= duration {
            return true;
        }
        thread::sleep((duration - elapsed).min(Duration::from_millis(50)));
    }
}

fn is_current_run(app: &AppHandle, token: u64) -> bool {
    let state = app.state::<AppState>();
    let inner = state.lock();
    inner.state.is_running && inner.run_token == token
}

fn finish_flow_edit(app: &AppHandle, state: &AppState, message: impl Into<String>) -> MacroState {
    state.save_active_profile(app);
    state.log(app, message)
}

fn can_edit(state: &AppState) -> bool {
    can_edit_flow(&state.lock())
}

fn can_change_point_action(current: PointAction, next: PointAction) -> bool {
    current == next || (current != PointAction::Key && next != PointAction::Key)
}

fn validate_run_start(
    is_recording: bool,
    is_running: bool,
    game_activity: bool,
    points: &[Point],
) -> Result<(), &'static str> {
    if is_recording {
        Err("录制中不能开始执行")
    } else if game_activity {
        Err("游戏录制或回放中不能开始执行宏")
    } else if points.is_empty() {
        Err("无法开始：流程步骤为空")
    } else if !points.iter().any(|point| point.enabled) {
        Err("无法开始：全部流程步骤已禁用")
    } else if is_running {
        Err("已在执行中")
    } else {
        Ok(())
    }
}

fn can_enable_key_capture(
    requested: bool,
    is_running: bool,
    is_recording: bool,
    game_activity: bool,
) -> bool {
    requested && !is_running && !is_recording && !game_activity
}

fn enabled_point_indices(points: &[Point]) -> Vec<usize> {
    points
        .iter()
        .enumerate()
        .filter_map(|(index, point)| point.enabled.then_some(index))
        .collect()
}

fn unnamed_label(label: &str) -> &str {
    if label.is_empty() { "未命名" } else { label }
}

fn safe_file_name(name: &str) -> String {
    let value = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect::<String>();
    let value = value.trim_matches([' ', '.']);
    if value.is_empty() {
        DEFAULT_IMPORT_NAME.into()
    } else {
        value.into()
    }
}

pub(crate) fn reorder_points(points: &mut Vec<Point>, id: &str, target_index: usize) -> bool {
    let Some(from_index) = points.iter().position(|point| point.id == id) else {
        return false;
    };
    if from_index == target_index || target_index >= points.len() {
        return false;
    }
    let point = points.remove(from_index);
    points.insert(target_index, point);
    true
}

const DEFAULT_IMPORT_NAME: &str = "导入方案";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_matches_the_package_version() {
        assert_eq!(get_app_version(), env!("CARGO_PKG_VERSION"));
    }

    fn point(id: &str) -> Point {
        Point {
            id: id.into(),
            label: id.into(),
            action: PointAction::Click,
            enabled: true,
            x: 0,
            y: 0,
            key: String::new(),
            modifiers: Vec::new(),
            delay_seconds: 0.5,
            created_at: 0,
        }
    }

    #[test]
    fn reorder_moves_a_step_to_the_requested_index() {
        let mut points = vec![point("a"), point("b"), point("c"), point("d")];
        assert!(reorder_points(&mut points, "b", 3));
        assert_eq!(
            points
                .iter()
                .map(|point| point.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "c", "d", "b"]
        );
        assert!(reorder_points(&mut points, "d", 0));
        assert_eq!(
            points
                .iter()
                .map(|point| point.id.as_str())
                .collect::<Vec<_>>(),
            vec!["d", "a", "c", "b"]
        );
        assert!(!reorder_points(&mut points, "missing", 1));
    }

    #[test]
    fn exported_file_name_removes_windows_reserved_characters() {
        assert_eq!(safe_file_name("a:b/c*"), "a_b_c_");
        assert_eq!(safe_file_name("..."), DEFAULT_IMPORT_NAME);
    }

    #[test]
    fn action_changes_only_allow_mouse_variants_to_convert() {
        assert!(can_change_point_action(
            PointAction::Click,
            PointAction::DoubleClick
        ));
        assert!(can_change_point_action(
            PointAction::DoubleClick,
            PointAction::Click
        ));
        assert!(can_change_point_action(PointAction::Key, PointAction::Key));
        assert!(!can_change_point_action(
            PointAction::Click,
            PointAction::Key
        ));
        assert!(!can_change_point_action(
            PointAction::Key,
            PointAction::DoubleClick
        ));
    }

    #[test]
    fn disabled_points_cannot_start_and_are_filtered_with_original_indices() {
        let mut points = vec![point("a"), point("b"), point("c")];
        for point in &mut points {
            point.enabled = false;
        }
        assert_eq!(
            validate_run_start(false, false, false, &points),
            Err("无法开始：全部流程步骤已禁用")
        );

        points[1].enabled = true;
        assert_eq!(validate_run_start(false, false, false, &points), Ok(()));
        assert_eq!(
            validate_run_start(false, false, true, &points),
            Err("游戏录制或回放中不能开始执行宏")
        );
        assert_eq!(enabled_point_indices(&points), vec![1]);
    }

    #[test]
    fn key_capture_cannot_be_enabled_during_any_automation_activity() {
        assert!(can_enable_key_capture(true, false, false, false));
        assert!(!can_enable_key_capture(false, false, false, false));
        assert!(!can_enable_key_capture(true, true, false, false));
        assert!(!can_enable_key_capture(true, false, true, false));
        assert!(!can_enable_key_capture(true, false, false, true));
    }
}

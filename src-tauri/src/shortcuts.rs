use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    buff_assistant,
    commands::{
        capture_point_internal, start_run_internal, stop_macro_workspace_activity_internal,
    },
    desktop::{ShortcutKind, Workspace, WorkspaceState},
    game_recorder::{
        self, GameRecorder, start_game_playback_internal, start_game_recording_internal,
        stop_game_activity_from_hotkey,
    },
    model::EMERGENCY_STOP_HOTKEY,
    state::AppState,
};

pub fn register_shortcuts(app: &AppHandle) {
    let workspace = app.state::<WorkspaceState>().active();
    let manager = app.global_shortcut();
    let mut errors = Vec::new();

    if let Err(error) = manager.unregister_all() {
        errors.push(format!("清理旧热键失败：{error}"));
    }

    if let Err(error) = manager.on_shortcut(EMERGENCY_STOP_HOTKEY, |app, _, event| {
        if event.state == ShortcutState::Pressed {
            stop_macro_workspace_activity_internal(app);
            stop_game_activity_from_hotkey(app, EMERGENCY_STOP_HOTKEY);
            buff_assistant::stop_buff_monitor_internal(app);
        }
    }) {
        errors.push(format!(
            "热键注册失败：紧急停止 {EMERGENCY_STOP_HOTKEY}（{error}）"
        ));
    }

    match workspace.shortcut_kind() {
        ShortcutKind::Macro => register_macro_shortcuts(app, &mut errors),
        ShortcutKind::GameRecorder => register_game_shortcuts(app, &mut errors),
        ShortcutKind::None => {}
    }

    match workspace {
        Workspace::Macro => {
            app.state::<AppState>().replace_hotkey_errors(app, errors);
        }
        Workspace::GameRecorder => {
            app.state::<GameRecorder>()
                .replace_hotkey_errors(app, errors);
        }
        Workspace::BuffAssistant | Workspace::Calculator | Workspace::TowerCalculator => {
            for error in errors {
                app.state::<AppState>().log(app, error);
            }
        }
    }
}

fn register_macro_shortcuts(app: &AppHandle, errors: &mut Vec<String>) {
    let hotkeys = app
        .state::<AppState>()
        .lock()
        .state
        .settings
        .hotkeys
        .clone();
    register_one(
        app,
        &hotkeys.capture,
        "采集坐标",
        |app| {
            capture_point_internal(app);
        },
        errors,
    );
    register_one(
        app,
        &hotkeys.start,
        "开始执行",
        |app| {
            start_run_internal(app);
        },
        errors,
    );
    register_one(
        app,
        &hotkeys.stop,
        "停止执行",
        |app| {
            stop_macro_workspace_activity_internal(app);
        },
        errors,
    );
}

fn register_game_shortcuts(app: &AppHandle, errors: &mut Vec<String>) {
    let hotkeys = game_recorder::hotkeys(app);
    register_one(
        app,
        &hotkeys.record_start,
        "开始游戏录制",
        |app| {
            let _ = start_game_recording_internal(app);
        },
        errors,
    );
    register_one(
        app,
        &hotkeys.stop,
        "停止游戏任务",
        |app| {
            let accelerator = game_recorder::hotkeys(app).stop;
            stop_game_activity_from_hotkey(app, &accelerator);
        },
        errors,
    );
    register_one(
        app,
        &hotkeys.playback_start,
        "开始游戏回放",
        |app| {
            let _ = start_game_playback_internal(app, false);
        },
        errors,
    );
}

pub fn unregister_all(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

fn register_one<F>(
    app: &AppHandle,
    accelerator: &str,
    label: &str,
    handler: F,
    errors: &mut Vec<String>,
) where
    F: Fn(&AppHandle) + Send + Sync + 'static,
{
    let accelerator_owned = accelerator.to_owned();
    if let Err(error) = app
        .global_shortcut()
        .on_shortcut(accelerator, move |app, _, event| {
            if event.state != ShortcutState::Pressed || is_capturing_key(app) {
                return;
            }
            handler(app);
        })
    {
        errors.push(format!(
            "热键注册失败：{label} {accelerator_owned}（{error}）"
        ));
    }
}

fn is_capturing_key(app: &AppHandle) -> bool {
    app.state::<AppState>().lock().is_capturing_key
}

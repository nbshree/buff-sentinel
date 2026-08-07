use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};

use std::sync::{Mutex, MutexGuard};

use serde::Deserialize;

use crate::{buff_assistant, commands, game_recorder, shortcuts, state::AppState};

const MENU_SHOW: &str = "show-window";
const MENU_START: &str = "start-run";
const MENU_STOP: &str = "stop-run";
const MENU_START_BUFF_MONITOR: &str = "start-buff-monitor";
const MENU_STOP_BUFF_MONITOR: &str = "stop-buff-monitor";
const MENU_QUIT: &str = "quit";
const TRAY_ID: &str = "main-tray";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Workspace {
    Macro,
    GameRecorder,
    #[default]
    BuffAssistant,
    Calculator,
    TowerCalculator,
}

pub struct WorkspaceState(Mutex<Workspace>);

impl Default for WorkspaceState {
    fn default() -> Self {
        Self(Mutex::new(Workspace::default()))
    }
}

impl WorkspaceState {
    fn lock(&self) -> MutexGuard<'_, Workspace> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn active(&self) -> Workspace {
        *self.lock()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayMenuKind {
    Macro,
    BuffAssistant,
    Common,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutKind {
    Macro,
    GameRecorder,
    None,
}

impl Workspace {
    fn requires_unlock(self) -> bool {
        matches!(self, Self::Macro | Self::GameRecorder)
    }

    fn menu_kind(self) -> TrayMenuKind {
        match self {
            Self::Macro => TrayMenuKind::Macro,
            Self::BuffAssistant => TrayMenuKind::BuffAssistant,
            Self::GameRecorder | Self::Calculator | Self::TowerCalculator => TrayMenuKind::Common,
        }
    }

    pub(crate) fn shortcut_kind(self) -> ShortcutKind {
        match self {
            Self::Macro => ShortcutKind::Macro,
            Self::GameRecorder => ShortcutKind::GameRecorder,
            Self::BuffAssistant | Self::Calculator | Self::TowerCalculator => ShortcutKind::None,
        }
    }
}

fn create_tray_menu(app: &AppHandle, workspace: Workspace) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, MENU_SHOW, "显示窗口", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;

    match workspace.menu_kind() {
        TrayMenuKind::Macro => {
            let start = MenuItem::with_id(app, MENU_START, "开始执行", true, None::<&str>)?;
            let stop = MenuItem::with_id(app, MENU_STOP, "停止当前任务", true, None::<&str>)?;
            Menu::with_items(app, &[&show, &start, &stop, &separator, &quit])
        }
        TrayMenuKind::BuffAssistant => {
            let start =
                MenuItem::with_id(app, MENU_START_BUFF_MONITOR, "开始监控", true, None::<&str>)?;
            let stop =
                MenuItem::with_id(app, MENU_STOP_BUFF_MONITOR, "停止监控", true, None::<&str>)?;
            Menu::with_items(app, &[&show, &start, &stop, &separator, &quit])
        }
        TrayMenuKind::Common => Menu::with_items(app, &[&show, &separator, &quit]),
    }
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = create_tray_menu(app, Workspace::default())?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("shree盒子")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().0.as_str() {
            MENU_SHOW => show_main_window(app),
            MENU_START => {
                commands::start_run_internal(app);
            }
            MENU_STOP => {
                commands::stop_macro_workspace_activity_internal(app);
            }
            MENU_START_BUFF_MONITOR => {
                let _ = buff_assistant::start_buff_monitor_internal(app);
            }
            MENU_STOP_BUFF_MONITOR => buff_assistant::stop_buff_monitor_internal(app),
            MENU_QUIT => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    Ok(())
}

#[tauri::command]
pub fn switch_workspace(app: AppHandle, workspace: Workspace) -> Result<(), String> {
    if workspace.requires_unlock()
        && !app
            .state::<AppState>()
            .lock()
            .store
            .restricted_workspaces_unlocked
    {
        return Err("该工作区尚未开放".into());
    }

    let workspace_state = app.state::<WorkspaceState>();
    let current = workspace_state.active();
    if current == workspace {
        return Ok(());
    }

    app.state::<AppState>().lock().is_capturing_key = false;
    stop_workspace_activity(&app, current)?;

    let menu = create_tray_menu(&app, workspace).map_err(|error| error.to_string())?;
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "主托盘图标尚未创建".to_string())?;
    tray.set_menu(Some(menu))
        .map_err(|error| error.to_string())?;

    *workspace_state.lock() = workspace;
    shortcuts::register_shortcuts(&app);
    Ok(())
}

fn stop_workspace_activity(app: &AppHandle, workspace: Workspace) -> Result<(), String> {
    match workspace {
        Workspace::Macro => {
            commands::stop_macro_workspace_activity_internal(app);
        }
        Workspace::GameRecorder => {
            let snapshot = game_recorder::stop_game_activity_internal(app);
            if snapshot.activity != game_recorder::GameRecorderActivity::Idle {
                return Err(snapshot.last_error.unwrap_or_else(|| {
                    "游戏任务尚未安全停止，请再次停止或使用紧急停止热键".into()
                }));
            }
        }
        Workspace::BuffAssistant => {
            buff_assistant::stop_buff_workspace_activity_internal(app)?;
        }
        Workspace::Calculator | Workspace::TowerCalculator => {}
    }
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn quit_app(app: &AppHandle) {
    commands::stop_macro_workspace_activity_internal(app);
    game_recorder::stop_game_activity_internal(app);
    buff_assistant::stop_buff_monitor_internal(app);
    {
        let state = app.state::<AppState>();
        state.lock().is_quitting = true;
    }
    shortcuts::unregister_all(app);
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::{ShortcutKind, TrayMenuKind, Workspace};

    #[test]
    fn workspace_uses_the_expected_tray_menu() {
        assert_eq!(Workspace::default(), Workspace::BuffAssistant);
        assert_eq!(Workspace::Macro.menu_kind(), TrayMenuKind::Macro);
        assert_eq!(
            Workspace::BuffAssistant.menu_kind(),
            TrayMenuKind::BuffAssistant
        );
        assert_eq!(Workspace::GameRecorder.menu_kind(), TrayMenuKind::Common);
        assert_eq!(Workspace::Calculator.menu_kind(), TrayMenuKind::Common);
        assert_eq!(Workspace::TowerCalculator.menu_kind(), TrayMenuKind::Common);
    }

    #[test]
    fn only_macro_and_game_recorder_require_unlocking() {
        assert!(Workspace::Macro.requires_unlock());
        assert!(Workspace::GameRecorder.requires_unlock());
        assert!(!Workspace::BuffAssistant.requires_unlock());
        assert!(!Workspace::Calculator.requires_unlock());
        assert!(!Workspace::TowerCalculator.requires_unlock());
    }

    #[test]
    fn workspace_names_match_the_frontend_values() {
        assert!(matches!(
            serde_json::from_str::<Workspace>("\"buffAssistant\""),
            Ok(Workspace::BuffAssistant)
        ));
        assert!(serde_json::from_str::<Workspace>("\"unknown\"").is_err());
    }

    #[test]
    fn workspace_selects_only_its_own_shortcut_group() {
        assert_eq!(Workspace::Macro.shortcut_kind(), ShortcutKind::Macro);
        assert_eq!(
            Workspace::GameRecorder.shortcut_kind(),
            ShortcutKind::GameRecorder
        );
        assert_eq!(Workspace::BuffAssistant.shortcut_kind(), ShortcutKind::None);
        assert_eq!(Workspace::Calculator.shortcut_kind(), ShortcutKind::None);
        assert_eq!(
            Workspace::TowerCalculator.shortcut_kind(),
            ShortcutKind::None
        );
    }
}

use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};

use crate::{buff_assistant, commands, game_recorder, shortcuts, state::AppState};

const MENU_SHOW: &str = "show-window";
const MENU_START: &str = "start-run";
const MENU_STOP: &str = "stop-run";
const MENU_QUIT: &str = "quit";

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "显示窗口", true, None::<&str>)?;
    let start = MenuItem::with_id(app, MENU_START, "开始执行", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, MENU_STOP, "停止当前任务", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &start, &stop, &separator, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("Shree Macro Flow")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().0.as_str() {
            MENU_SHOW => show_main_window(app),
            MENU_START => {
                commands::start_run_internal(app);
            }
            MENU_STOP => {
                commands::stop_run_internal(app);
                game_recorder::stop_game_activity_internal(app);
                buff_assistant::stop_buff_monitor_internal(app);
            }
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

pub fn show_main_window(app: &AppHandle) {
    buff_assistant::pause_sample_capture_internal(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn quit_app(app: &AppHandle) {
    commands::stop_run_internal(app);
    game_recorder::stop_game_activity_internal(app);
    buff_assistant::stop_buff_monitor_internal(app);
    {
        let state = app.state::<AppState>();
        state.lock().is_quitting = true;
    }
    shortcuts::unregister_all(app);
    app.exit(0);
}

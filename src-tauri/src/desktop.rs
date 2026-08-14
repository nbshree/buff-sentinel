use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};

use crate::buff_assistant;

const MENU_SHOW: &str = "show-window";
const MENU_START_BUFF_MONITOR: &str = "start-buff-monitor";
const MENU_STOP_BUFF_MONITOR: &str = "stop-buff-monitor";
const MENU_QUIT: &str = "quit";

#[derive(Default)]
pub struct DesktopState {
    is_quitting: AtomicBool,
}

impl DesktopState {
    pub fn is_quitting(&self) -> bool {
        self.is_quitting.load(Ordering::Relaxed)
    }
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "显示窗口", true, None::<&str>)?;
    let start = MenuItem::with_id(app, MENU_START_BUFF_MONITOR, "开始监控", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, MENU_STOP_BUFF_MONITOR, "停止监控", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &start, &stop, &separator, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("BuffFlow")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().0.as_str() {
            MENU_SHOW => show_main_window(app),
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

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn quit_app(app: &AppHandle) {
    let _ = buff_assistant::stop_buff_workspace_activity_internal(app);
    app.state::<DesktopState>()
        .is_quitting
        .store(true, Ordering::Relaxed);
    app.exit(0);
}

mod buff_assistant;
mod desktop;
mod platform;
mod updater;

use tauri::{Manager, RunEvent, WindowEvent};

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform::enable_per_monitor_dpi_awareness();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            desktop::show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let (buff_state, _notices) = buff_assistant::BuffAssistant::load(app.handle())?;
            app.manage(buff_state);
            app.manage(desktop::DesktopState::default());
            app.manage(updater::PendingUpdate::default());
            updater::schedule_installer_cleanup(app.package_info().name.clone());
            desktop::create_tray(app.handle())?;
            buff_assistant::create_overlay(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if !app.state::<desktop::DesktopState>().is_quitting() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            updater::check_for_update,
            updater::install_update,
            buff_assistant::get_buff_assistant_state,
            buff_assistant::list_buff_capture_windows,
            buff_assistant::list_buff_sound_templates,
            buff_assistant::capture_buff_preview,
            buff_assistant::update_buff_search_region,
            buff_assistant::get_buff_listener_template,
            buff_assistant::request_buff_borderless_capture_access,
            buff_assistant::save_buff_listener,
            buff_assistant::update_buff_listener,
            buff_assistant::delete_buff_listener,
            buff_assistant::update_buff_assistant_settings,
            buff_assistant::start_buff_monitor,
            buff_assistant::stop_buff_monitor,
            buff_assistant::start_buff_template_test,
            buff_assistant::stop_buff_template_test,
            buff_assistant::play_buff_assistant_sound,
            buff_assistant::import_buff_assistant_sound,
            buff_assistant::open_tts_online,
            buff_assistant::set_buff_overlay_edit_mode,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Buff Sentinel");

    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => desktop::show_main_window(app),
        RunEvent::Exit => {
            let _ = buff_assistant::stop_buff_workspace_activity_internal(app);
        }
        _ => {}
    });
}

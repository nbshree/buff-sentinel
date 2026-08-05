mod buff_assistant;
mod commands;
mod desktop;
mod game_recorder;
mod input;
mod internal_skill_ai;
mod model;
mod raw_input;
mod shortcuts;
mod state;
mod store;
mod updater;

use std::io;

use tauri::{Manager, RunEvent, WindowEvent};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    input::enable_per_monitor_dpi_awareness();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            desktop::show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let profile_file = store::profile_file_path(app.handle()).map_err(io::Error::other)?;
            let loaded = store::load_profiles(&profile_file);
            let mut notices = loaded.notices;
            let game_storage =
                game_recorder::storage_directory(app.handle()).map_err(io::Error::other)?;
            let (game_state, game_notices) = game_recorder::GameRecorder::load(game_storage);
            notices.extend(game_notices);
            let (buff_state, buff_notices) = buff_assistant::BuffAssistant::load(app.handle())?;
            notices.extend(buff_notices);
            app.manage(AppState::new(profile_file, loaded.store));
            app.manage(game_state);
            app.manage(buff_state);
            app.manage(desktop::WorkspaceState::default());
            app.manage(updater::PendingUpdate::default());

            let state = app.state::<AppState>();
            state.persist_current_store(app.handle());
            desktop::create_tray(app.handle())?;
            buff_assistant::create_overlay(app.handle())?;
            if let Err(error) = game_recorder::start_raw_input_listener(app.handle()) {
                notices.push(format!("游戏录制不可用：{error}"));
            }
            shortcuts::register_shortcuts(app.handle());
            for notice in notices {
                state.log(app.handle(), notice);
            }
            state.log(app.handle(), "应用已启动");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let is_quitting = app.state::<AppState>().lock().is_quitting;
                if !is_quitting {
                    api.prevent_close();
                    let _ = window.hide();
                    app.state::<AppState>().log(app, "窗口已最小化到托盘");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_version,
            desktop::switch_workspace,
            commands::get_state,
            commands::update_appearance,
            commands::start_recording,
            commands::stop_recording,
            commands::start_run,
            commands::stop_run,
            commands::clear_logs,
            commands::remove_point,
            commands::clear_points,
            commands::add_key_point,
            commands::set_key_capture,
            commands::sync_point_delays,
            commands::update_point,
            commands::move_point,
            commands::reorder_point,
            commands::test_point,
            commands::update_settings,
            commands::create_profile,
            commands::switch_profile,
            commands::rename_profile,
            commands::delete_profile,
            commands::export_profile,
            commands::import_profile,
            game_recorder::get_game_recorder_state,
            game_recorder::start_game_recording,
            game_recorder::stop_game_activity,
            game_recorder::start_game_playback,
            game_recorder::select_game_recording,
            game_recorder::rename_game_recording,
            game_recorder::delete_game_recording,
            game_recorder::update_game_recorder_hotkeys,
            game_recorder::update_game_playback_settings,
            buff_assistant::get_buff_assistant_state,
            buff_assistant::list_buff_capture_windows,
            buff_assistant::list_buff_sound_templates,
            buff_assistant::capture_buff_preview,
            buff_assistant::save_buff_template,
            buff_assistant::delete_buff_template,
            buff_assistant::update_buff_assistant_settings,
            buff_assistant::start_buff_monitor,
            buff_assistant::stop_buff_monitor,
            buff_assistant::start_buff_template_test,
            buff_assistant::stop_buff_template_test,
            buff_assistant::play_buff_assistant_sound,
            buff_assistant::import_buff_assistant_sound,
            buff_assistant::open_tts_online,
            buff_assistant::set_buff_overlay_edit_mode,
            internal_skill_ai::get_mystery_code_status,
            internal_skill_ai::open_ai_provider_registration,
            internal_skill_ai::save_and_validate_mystery_code,
            internal_skill_ai::delete_mystery_code,
            internal_skill_ai::recognize_internal_skill_image,
            updater::check_for_update,
            updater::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Tauri application");

    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => desktop::show_main_window(app),
        RunEvent::Exit => {
            commands::stop_macro_workspace_activity_internal(app);
            game_recorder::stop_game_activity_internal(app);
            buff_assistant::stop_buff_monitor_internal(app);
            shortcuts::unregister_all(app);
        }
        _ => {}
    });
}

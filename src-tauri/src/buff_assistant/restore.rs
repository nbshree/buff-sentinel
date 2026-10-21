use std::{mem::zeroed, ptr::null_mut, thread, time::Duration};

use tauri::{AppHandle, Manager};
use windows_capture::window::Window;
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{
            DispatchMessageW, EVENT_OBJECT_DESTROY, EVENT_OBJECT_SHOW, EVENT_OBJECT_STATECHANGE,
            EVENT_SYSTEM_MINIMIZEEND, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx,
            PM_NOREMOVE, PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage,
            WINEVENT_OUTOFCONTEXT,
        },
    },
};

use super::{
    BuffAssistant, BuffCaptureRestoreStatus, BuffTarget, CaptureWindowCandidate,
    capture_preview_for_window, emit_capture_preview, emit_state, record_capture_preview_result,
    windows,
};

const RESTORE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STABLE_WINDOW_DELAY: Duration = Duration::from_millis(650);

pub(super) fn start(app: AppHandle, target: BuffTarget, generation: u64) {
    thread::spawn(move || watch_target(app, target, generation));
}

fn watch_target(app: AppHandle, target: BuffTarget, generation: u64) {
    let mut resolved: Option<(Window, CaptureWindowCandidate)> = None;
    let mut _hooks = EventHooks::default();
    let mut hooked_process_id = None;
    ensure_message_queue();

    loop {
        if !restore_is_active(&app, generation) {
            return;
        }
        if resolved
            .as_ref()
            .is_some_and(|(window, _)| !windows::is_window_alive(*window))
        {
            resolved = None;
            _hooks = EventHooks::default();
            hooked_process_id = None;
        }
        if resolved.is_none() {
            resolved = match windows::resolve_target(&target) {
                Ok(value) => value,
                Err(error) => {
                    finish_with_error(&app, generation, error);
                    return;
                }
            };
            if let Some((window, _)) = resolved.as_ref() {
                let process_id = window.process_id().ok();
                if process_id != hooked_process_id {
                    _hooks = process_id.map(EventHooks::install).unwrap_or_default();
                    hooked_process_id = process_id;
                }
            }
        }

        if let Some((window, candidate)) = resolved.as_ref()
            && windows::is_window_ready(*window)
        {
            if !set_status(&app, generation, BuffCaptureRestoreStatus::Capturing) {
                return;
            }
            thread::sleep(STABLE_WINDOW_DELAY);
            if !restore_is_active(&app, generation) {
                return;
            }
            if !windows::is_window_ready(*window) {
                if !set_status(&app, generation, BuffCaptureRestoreStatus::WaitingForWindow) {
                    return;
                }
            } else {
                finish_capture(&app, generation, *window, candidate.clone());
                return;
            }
        }

        wait_for_window_activity();
    }
}

fn finish_capture(
    app: &AppHandle,
    generation: u64,
    window: Window,
    candidate: CaptureWindowCandidate,
) {
    let state = app.state::<BuffAssistant>();
    let result = capture_preview_for_window(app, &state, window, candidate);
    record_capture_preview_result(&state, &result);
    let (status, error) = match &result {
        Ok(_) => (BuffCaptureRestoreStatus::Idle, None),
        Err(error) => (BuffCaptureRestoreStatus::Idle, Some(error.clone())),
    };
    let snapshot = {
        let mut inner = state.lock();
        if inner.capture_restore_generation != generation {
            return;
        }
        inner.capture_restore_status = status;
        inner.last_error = error;
        super::snapshot_from_runtime(&inner)
    };
    emit_state(app, &snapshot);
    if let Ok(preview) = result {
        emit_capture_preview(app, &preview);
    }
}

fn finish_with_error(app: &AppHandle, generation: u64, error: String) {
    let state = app.state::<BuffAssistant>();
    let snapshot = {
        let mut inner = state.lock();
        if inner.capture_restore_generation != generation {
            return;
        }
        inner.capture_restore_status = BuffCaptureRestoreStatus::Idle;
        inner.capture_recovery_required = true;
        inner.last_error = Some(error);
        super::snapshot_from_runtime(&inner)
    };
    emit_state(app, &snapshot);
}

fn restore_is_active(app: &AppHandle, generation: u64) -> bool {
    let state = app.state::<BuffAssistant>();
    let inner = state.lock();
    inner.capture_restore_generation == generation
        && inner.capture_restore_status != BuffCaptureRestoreStatus::Idle
        && !inner.monitor_requested
        && inner.capture_purpose.is_none()
}

fn set_status(app: &AppHandle, generation: u64, status: BuffCaptureRestoreStatus) -> bool {
    let state = app.state::<BuffAssistant>();
    let snapshot = {
        let mut inner = state.lock();
        if inner.capture_restore_generation != generation {
            return false;
        }
        inner.capture_restore_status = status;
        super::snapshot_from_runtime(&inner)
    };
    emit_state(app, &snapshot);
    true
}

#[derive(Default)]
struct EventHooks {
    handles: Vec<HWINEVENTHOOK>,
}

impl EventHooks {
    fn install(process_id: u32) -> Self {
        let mut handles = Vec::new();
        for event in [
            EVENT_SYSTEM_MINIMIZEEND,
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_STATECHANGE,
            EVENT_OBJECT_DESTROY,
        ] {
            let hook = unsafe {
                SetWinEventHook(
                    event,
                    event,
                    null_mut(),
                    Some(ignore_window_event),
                    process_id,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                )
            };
            if !hook.is_null() {
                handles.push(hook);
            }
        }
        Self { handles }
    }
}

impl Drop for EventHooks {
    fn drop(&mut self) {
        for hook in self.handles.drain(..) {
            unsafe {
                UnhookWinEvent(hook);
            }
        }
    }
}

unsafe extern "system" fn ignore_window_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _window: HWND,
    _object_id: i32,
    _child_id: i32,
    _thread_id: u32,
    _event_time: u32,
) {
}

fn ensure_message_queue() {
    unsafe {
        let mut message: MSG = zeroed();
        PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
    }
}

fn wait_for_window_activity() {
    unsafe {
        MsgWaitForMultipleObjectsEx(
            0,
            std::ptr::null(),
            RESTORE_POLL_INTERVAL.as_millis() as u32,
            QS_ALLINPUT,
            MWMO_INPUTAVAILABLE,
        );
        let mut message: MSG = zeroed();
        while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

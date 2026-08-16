use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use super::{BuffAssistant, emit_execution_log, emit_state};

const HOTKEY_ERROR_PREFIX: &str = "监控热键注册失败";

trait ShortcutRegistrar {
    fn register(&self, shortcut: &str) -> Result<(), String>;
    fn unregister(&self, shortcut: &str) -> Result<(), String>;
}

struct TauriShortcutRegistrar<'a> {
    app: &'a AppHandle,
}

impl ShortcutRegistrar for TauriShortcutRegistrar<'_> {
    fn register(&self, shortcut: &str) -> Result<(), String> {
        self.app
            .global_shortcut()
            .register(shortcut)
            .map_err(|error| error.to_string())
    }

    fn unregister(&self, shortcut: &str) -> Result<(), String> {
        self.app
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug)]
struct RebindError {
    message: String,
    active: Option<String>,
}

pub fn normalize_monitor_hotkey(shortcut: Option<&str>) -> Result<Option<String>, String> {
    let Some(shortcut) = shortcut.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut primary = None;

    for part in shortcut.split('+').map(str::trim) {
        if part.is_empty() {
            return Err("热键格式无效，请重新录入组合键".into());
        }
        match part.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => ctrl = true,
            "ALT" => alt = true,
            "SHIFT" => shift = true,
            "WIN" | "WINDOWS" | "META" | "SUPER" => {
                return Err("监控热键不支持 Windows 键".into());
            }
            value if is_supported_primary_key(value) => {
                if primary.replace(value.to_string()).is_some() {
                    return Err("监控热键只能包含一个主按键".into());
                }
            }
            _ => return Err("监控热键仅支持字母、数字和 F1-F24".into()),
        }
    }

    let primary = primary.ok_or_else(|| "监控热键必须包含一个主按键".to_string())?;
    let is_function_key = primary
        .strip_prefix('F')
        .and_then(|value| value.parse::<u8>().ok())
        .is_some_and(|value| (1..=24).contains(&value));
    if !is_function_key && !ctrl && !alt && !shift {
        return Err("字母或数字热键至少需要 Ctrl、Alt、Shift 中的一个修饰键".into());
    }

    let mut parts = Vec::with_capacity(4);
    if ctrl {
        parts.push("Ctrl".to_string());
    }
    if alt {
        parts.push("Alt".to_string());
    }
    if shift {
        parts.push("Shift".to_string());
    }
    parts.push(primary);
    Ok(Some(parts.join("+")))
}

pub fn initialize(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let configured = state.lock().config.settings.monitor_hotkey.clone();
    match normalize_monitor_hotkey(configured.as_deref()) {
        Ok(shortcut) => {
            if let Err(error) = rebind(app, shortcut.as_deref()) {
                set_registration_error(&state, Some(error));
            }
        }
        Err(error) => set_registration_error(&state, Some(error)),
    }
}

pub fn rebind(app: &AppHandle, requested: Option<&str>) -> Result<(), String> {
    let state = app.state::<BuffAssistant>();
    let requested = normalize_monitor_hotkey(requested)?;
    let registered = state.lock().registered_monitor_hotkey.clone();
    if registered == requested {
        set_registration_error(&state, None);
        return Ok(());
    }
    let registrar = TauriShortcutRegistrar { app };
    match replace_registered_shortcut(&registrar, registered.as_deref(), requested.as_deref()) {
        Ok(active) => {
            let mut inner = state.lock();
            inner.registered_monitor_hotkey = active;
            inner.hotkey_registration_error = None;
            Ok(())
        }
        Err(error) => {
            let mut inner = state.lock();
            inner.registered_monitor_hotkey = error.active;
            inner.hotkey_registration_error = Some(error.message.clone());
            Err(error.message)
        }
    }
}

pub fn handle_pressed(app: &AppHandle) {
    let state = app.state::<BuffAssistant>();
    let snapshot = state.snapshot();
    if snapshot.is_monitoring {
        super::stop_buff_monitor_internal(app);
        emit_execution_log(app, "监控热键：停止日常监控");
        return;
    }
    if snapshot.activity == super::BuffAssistantActivity::Testing {
        report_toggle_error(app, "请先停止实时识别测试，再使用监控热键");
        return;
    }

    match super::start_buff_monitor_internal(app) {
        Ok(()) => emit_execution_log(app, "监控热键：开始日常监控"),
        Err(error) => report_toggle_error(app, &error),
    }
}

fn is_supported_primary_key(value: &str) -> bool {
    if value.len() == 1 {
        return value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase() || character.is_ascii_digit());
    }
    value
        .strip_prefix('F')
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (1..=24).contains(&number))
}

fn replace_registered_shortcut(
    registrar: &impl ShortcutRegistrar,
    current: Option<&str>,
    requested: Option<&str>,
) -> Result<Option<String>, RebindError> {
    if current == requested {
        return Ok(current.map(str::to_string));
    }
    if let Some(current) = current
        && let Err(error) = registrar.unregister(current)
    {
        return Err(RebindError {
            message: format!("无法注销旧监控热键 {current}：{error}"),
            active: Some(current.to_string()),
        });
    }
    if let Some(requested) = requested
        && let Err(error) = registrar.register(requested)
    {
        let rollback_error = current.and_then(|shortcut| registrar.register(shortcut).err());
        let (message, active) = if let Some(rollback_error) = rollback_error {
            (
                format!(
                    "{HOTKEY_ERROR_PREFIX}：{requested} 可能已被其他程序占用；恢复旧热键也失败：{rollback_error}"
                ),
                None,
            )
        } else {
            (
                format!("{HOTKEY_ERROR_PREFIX}：{requested} 可能已被其他程序占用：{error}"),
                current.map(str::to_string),
            )
        };
        return Err(RebindError { message, active });
    }
    Ok(requested.map(str::to_string))
}

fn report_toggle_error(app: &AppHandle, error: &str) {
    let state = app.state::<BuffAssistant>();
    let message = format!("监控热键操作失败：{error}");
    let snapshot = {
        let mut inner = state.lock();
        inner.last_error = Some(message.clone());
        super::snapshot_from_runtime(&inner)
    };
    emit_execution_log(app, &message);
    emit_state(app, &snapshot);
}

fn set_registration_error(state: &BuffAssistant, error: Option<String>) {
    let mut inner = state.lock();
    inner.hotkey_registration_error = error;
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashSet};

    use super::{ShortcutRegistrar, normalize_monitor_hotkey, replace_registered_shortcut};

    #[derive(Default)]
    struct FakeRegistrar {
        registered: RefCell<HashSet<String>>,
        rejected: RefCell<HashSet<String>>,
    }

    impl ShortcutRegistrar for FakeRegistrar {
        fn register(&self, shortcut: &str) -> Result<(), String> {
            if self.rejected.borrow().contains(shortcut) {
                return Err("already registered".into());
            }
            self.registered.borrow_mut().insert(shortcut.to_string());
            Ok(())
        }

        fn unregister(&self, shortcut: &str) -> Result<(), String> {
            self.registered.borrow_mut().remove(shortcut);
            Ok(())
        }
    }

    #[test]
    fn normalizes_supported_shortcuts() {
        assert_eq!(
            normalize_monitor_hotkey(Some("shift + ctrl + a")).unwrap(),
            Some("Ctrl+Shift+A".into())
        );
        assert_eq!(
            normalize_monitor_hotkey(Some("f24")).unwrap(),
            Some("F24".into())
        );
        assert_eq!(normalize_monitor_hotkey(Some("  ")).unwrap(), None);
    }

    #[test]
    fn rejects_unsafe_or_incomplete_shortcuts() {
        assert!(normalize_monitor_hotkey(Some("Ctrl+Shift")).is_err());
        assert!(normalize_monitor_hotkey(Some("A")).is_err());
        assert!(normalize_monitor_hotkey(Some("Win+F10")).is_err());
        assert!(normalize_monitor_hotkey(Some("Ctrl+Escape")).is_err());
    }

    #[test]
    fn clears_a_registered_shortcut() {
        let registrar = FakeRegistrar::default();
        registrar
            .registered
            .borrow_mut()
            .insert("Ctrl+Alt+F10".into());

        let active = replace_registered_shortcut(&registrar, Some("Ctrl+Alt+F10"), None).unwrap();

        assert_eq!(active, None);
        assert!(registrar.registered.borrow().is_empty());
    }

    #[test]
    fn restores_the_old_shortcut_when_the_new_one_conflicts() {
        let registrar = FakeRegistrar::default();
        registrar
            .registered
            .borrow_mut()
            .insert("Ctrl+Alt+F10".into());
        registrar
            .rejected
            .borrow_mut()
            .insert("Ctrl+Shift+K".into());

        let error =
            replace_registered_shortcut(&registrar, Some("Ctrl+Alt+F10"), Some("Ctrl+Shift+K"))
                .err()
                .unwrap();

        assert_eq!(error.active.as_deref(), Some("Ctrl+Alt+F10"));
        assert!(registrar.registered.borrow().contains("Ctrl+Alt+F10"));
        assert!(!registrar.registered.borrow().contains("Ctrl+Shift+K"));
    }
}

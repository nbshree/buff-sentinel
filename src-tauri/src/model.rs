use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const EMERGENCY_STOP_HOTKEY: &str = "CommandOrControl+Alt+Esc";
pub const DEFAULT_PROFILE_NAME: &str = "默认方案";
pub const DEFAULT_THEME_ID: &str = "longyin";
pub const DEFAULT_AI_BASE_URL: &str = "https://gzxsy.vip";
pub const PROFILE_FILE_NAME: &str = "macro-profiles.json";

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PointAction {
    Click,
    DoubleClick,
    Key,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KeyModifier {
    Control,
    Alt,
    Shift,
}

impl KeyModifier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Control => "Ctrl",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
        }
    }

    pub fn accelerator_label(self) -> &'static str {
        match self {
            Self::Control => "Control",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub id: String,
    pub label: String,
    pub action: PointAction,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub x: i32,
    pub y: i32,
    pub key: String,
    pub modifiers: Vec<KeyModifier>,
    pub delay_seconds: f64,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hotkeys {
    pub capture: String,
    pub start: String,
    pub stop: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopMode {
    Count,
    Infinite,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub click_interval_seconds: f64,
    pub loop_interval_seconds: f64,
    pub start_delay_seconds: f64,
    pub loop_mode: LoopMode,
    pub loop_count: u32,
    pub hotkeys: Hotkeys,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroProfile {
    pub id: String,
    pub name: String,
    pub points: Vec<Point>,
    pub settings: Settings,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearancePreferences {
    pub theme_id: String,
    pub clean_mode: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearancePatch {
    pub theme_id: Option<String>,
    pub clean_mode: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedData {
    pub active_profile_id: String,
    pub profiles: Vec<MacroProfile>,
    #[serde(default = "default_appearance")]
    pub appearance: AppearancePreferences,
    #[serde(default)]
    pub restricted_workspaces_unlocked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mystery_code: Option<String>,
    #[serde(default = "default_ai_base_url")]
    pub ai_base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_api_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroState {
    pub points: Vec<Point>,
    pub settings: Settings,
    pub appearance: AppearancePreferences,
    pub active_profile_id: String,
    pub profiles: Vec<ProfileSummary>,
    pub is_recording: bool,
    pub is_running: bool,
    pub current_index: i32,
    pub countdown_remaining: u32,
    pub completed_loops: u32,
    pub hotkey_errors: Vec<String>,
    pub logs: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointPatch {
    pub label: Option<String>,
    pub action: Option<PointAction>,
    pub enabled: Option<bool>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub key: Option<String>,
    pub modifiers: Option<Vec<KeyModifier>>,
    pub delay_seconds: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeysPatch {
    pub capture: Option<String>,
    pub start: Option<String>,
    pub stop: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub click_interval_seconds: Option<f64>,
    pub loop_interval_seconds: Option<f64>,
    pub start_delay_seconds: Option<f64>,
    pub loop_mode: Option<LoopMode>,
    pub loop_count: Option<f64>,
    pub hotkeys: Option<HotkeysPatch>,
}

pub fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

fn default_enabled() -> bool {
    true
}

pub fn create_id() -> String {
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}-{sequence:x}", now_millis())
}

pub fn default_settings() -> Settings {
    Settings {
        click_interval_seconds: 0.5,
        loop_interval_seconds: 1.0,
        start_delay_seconds: 1.0,
        loop_mode: LoopMode::Infinite,
        loop_count: 1,
        hotkeys: Hotkeys {
            capture: "CommandOrControl+Alt+Q".into(),
            start: "CommandOrControl+Alt+P".into(),
            stop: "CommandOrControl+Alt+O".into(),
        },
    }
}

pub fn default_appearance() -> AppearancePreferences {
    AppearancePreferences {
        theme_id: DEFAULT_THEME_ID.into(),
        clean_mode: false,
    }
}

pub fn default_ai_base_url() -> String {
    DEFAULT_AI_BASE_URL.into()
}

pub fn create_default_profile_store() -> PersistedData {
    let now = now_millis();
    let profile = MacroProfile {
        id: create_id(),
        name: DEFAULT_PROFILE_NAME.into(),
        points: Vec::new(),
        settings: default_settings(),
        created_at: now,
        updated_at: now,
    };

    PersistedData {
        active_profile_id: profile.id.clone(),
        profiles: vec![profile],
        appearance: default_appearance(),
        restricted_workspaces_unlocked: false,
        mystery_code: None,
        ai_base_url: default_ai_base_url(),
        ai_api_key: None,
    }
}

pub fn state_from_store(store: &mut PersistedData) -> MacroState {
    if store.profiles.is_empty() {
        *store = create_default_profile_store();
    }

    let active_index = store
        .profiles
        .iter()
        .position(|profile| profile.id == store.active_profile_id)
        .unwrap_or(0);
    let active = &store.profiles[active_index];
    store.active_profile_id = active.id.clone();

    MacroState {
        points: active.points.clone(),
        settings: active.settings.clone(),
        appearance: store.appearance.clone(),
        active_profile_id: active.id.clone(),
        profiles: profile_summaries(store),
        is_recording: false,
        is_running: false,
        current_index: -1,
        countdown_remaining: 0,
        completed_loops: 0,
        hotkey_errors: Vec::new(),
        logs: Vec::new(),
    }
}

pub fn profile_summaries(store: &PersistedData) -> Vec<ProfileSummary> {
    store
        .profiles
        .iter()
        .map(|profile| ProfileSummary {
            id: profile.id.clone(),
            name: profile.name.clone(),
            updated_at: profile.updated_at,
        })
        .collect()
}

pub fn sanitize_persisted(value: &Value) -> PersistedData {
    let Some(object) = value.as_object() else {
        return create_default_profile_store();
    };

    let profiles = object
        .get("profiles")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .enumerate()
                .filter_map(|(index, profile)| {
                    sanitize_profile(profile, &format!("{DEFAULT_PROFILE_NAME} {}", index + 1))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let appearance = sanitize_appearance(object.get("appearance"));
    let restricted_workspaces_unlocked = object
        .get("restrictedWorkspacesUnlocked")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mystery_code = object
        .get("mysteryCode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let ai_base_url = object
        .get("aiBaseUrl")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
        .unwrap_or_else(default_ai_base_url);
    let ai_api_key = object
        .get("aiApiKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if profiles.is_empty() {
        let mut store = create_default_profile_store();
        store.appearance = appearance;
        store.restricted_workspaces_unlocked = restricted_workspaces_unlocked;
        store.mystery_code = mystery_code;
        store.ai_base_url = ai_base_url;
        store.ai_api_key = ai_api_key;
        return store;
    }

    let requested_active = object
        .get("activeProfileId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active_profile_id = profiles
        .iter()
        .find(|profile| profile.id == requested_active)
        .unwrap_or(&profiles[0])
        .id
        .clone();

    PersistedData {
        active_profile_id,
        profiles,
        appearance,
        restricted_workspaces_unlocked,
        mystery_code,
        ai_base_url,
        ai_api_key,
    }
}

pub fn sanitize_appearance(value: Option<&Value>) -> AppearancePreferences {
    let object = value.and_then(Value::as_object);
    AppearancePreferences {
        theme_id: sanitize_theme_id(
            object
                .and_then(|appearance| appearance.get("themeId"))
                .and_then(Value::as_str),
        ),
        clean_mode: object
            .and_then(|appearance| appearance.get("cleanMode"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

pub fn patch_appearance(
    current: &AppearancePreferences,
    patch: &AppearancePatch,
) -> AppearancePreferences {
    AppearancePreferences {
        theme_id: patch
            .theme_id
            .as_deref()
            .map(|theme_id| sanitize_theme_id(Some(theme_id)))
            .unwrap_or_else(|| sanitize_theme_id(Some(&current.theme_id))),
        clean_mode: patch.clean_mode.unwrap_or(current.clean_mode),
    }
}

pub fn sanitize_theme_id(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("default") => "default".into(),
        Some("longyin") => "longyin".into(),
        Some("chaoguang") => "chaoguang".into(),
        Some("xuehe") => "xuehe".into(),
        Some("jiuling") => "jiuling".into(),
        Some("suwen") => "suwen".into(),
        Some("shenxiang") => "shenxiang".into(),
        _ => DEFAULT_THEME_ID.into(),
    }
}

pub fn sanitize_profile(value: &Value, fallback_name: &str) -> Option<MacroProfile> {
    let object = value.as_object()?;
    let now = now_millis();
    let points = object
        .get("points")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .enumerate()
                .filter_map(|(index, point)| sanitize_point(point, index))
                .collect()
        })
        .unwrap_or_default();

    Some(MacroProfile {
        id: non_empty_string(object.get("id")).unwrap_or_else(create_id),
        name: sanitize_profile_name(object.get("name"), fallback_name),
        points,
        settings: sanitize_settings(object.get("settings")),
        created_at: finite_number(object.get("createdAt"))
            .map(|value| value as i64)
            .unwrap_or(now),
        updated_at: finite_number(object.get("updatedAt"))
            .map(|value| value as i64)
            .unwrap_or(now),
    })
}

pub fn sanitize_profile_name(value: Option<&Value>, fallback: &str) -> String {
    let name = value
        .and_then(Value::as_str)
        .map(str::trim)
        .map(|value| truncate_chars(value, 50))
        .unwrap_or_default();
    if name.is_empty() {
        fallback.into()
    } else {
        name
    }
}

pub fn sanitize_point(value: &Value, index: usize) -> Option<Point> {
    let object = value.as_object()?;
    let action = match object.get("action").and_then(Value::as_str) {
        Some("key") => PointAction::Key,
        Some("doubleClick") => PointAction::DoubleClick,
        _ => PointAction::Click,
    };
    let x = finite_number(object.get("x"));
    let y = finite_number(object.get("y"));
    if action != PointAction::Key && (x.is_none() || y.is_none()) {
        return None;
    }

    let key = object
        .get("key")
        .and_then(Value::as_str)
        .map(normalize_key)
        .unwrap_or_default();
    if action == PointAction::Key && virtual_key_code(&key).is_none() {
        return None;
    }

    Some(Point {
        id: non_empty_string(object.get("id")).unwrap_or_else(create_id),
        label: object
            .get("label")
            .and_then(Value::as_str)
            .map(|value| truncate_chars(value, 60))
            .unwrap_or_else(|| format!("步骤 {}", index + 1)),
        action,
        enabled: object
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or_else(default_enabled),
        x: clamp_f64(x.unwrap_or(0.0), 0.0, -100_000.0, 100_000.0).round() as i32,
        y: clamp_f64(y.unwrap_or(0.0), 0.0, -100_000.0, 100_000.0).round() as i32,
        key,
        modifiers: sanitize_modifiers(object.get("modifiers")),
        delay_seconds: clamp_f64(
            finite_number(object.get("delaySeconds")).unwrap_or(f64::NAN),
            default_settings().click_interval_seconds,
            0.1,
            3600.0,
        ),
        created_at: finite_number(object.get("createdAt"))
            .map(|value| value as i64)
            .unwrap_or_else(now_millis),
    })
}

pub fn sanitize_settings(value: Option<&Value>) -> Settings {
    let defaults = default_settings();
    let object = value.and_then(Value::as_object);
    let hotkey_object = object
        .and_then(|settings| settings.get("hotkeys"))
        .and_then(Value::as_object);

    Settings {
        click_interval_seconds: clamp_optional_number(
            object.and_then(|settings| settings.get("clickIntervalSeconds")),
            defaults.click_interval_seconds,
            0.1,
            3600.0,
        ),
        loop_interval_seconds: clamp_optional_number(
            object.and_then(|settings| settings.get("loopIntervalSeconds")),
            defaults.loop_interval_seconds,
            0.0,
            3600.0,
        ),
        start_delay_seconds: clamp_optional_number(
            object.and_then(|settings| settings.get("startDelaySeconds")),
            defaults.start_delay_seconds,
            0.0,
            60.0,
        ),
        loop_mode: if object
            .and_then(|settings| settings.get("loopMode"))
            .and_then(Value::as_str)
            == Some("infinite")
        {
            LoopMode::Infinite
        } else {
            LoopMode::Count
        },
        loop_count: clamp_optional_number(
            object.and_then(|settings| settings.get("loopCount")),
            defaults.loop_count as f64,
            1.0,
            9999.0,
        )
        .round() as u32,
        hotkeys: Hotkeys {
            capture: sanitize_hotkey_field(hotkey_object, "capture", &defaults.hotkeys.capture),
            start: sanitize_hotkey_field(hotkey_object, "start", &defaults.hotkeys.start),
            stop: sanitize_hotkey_field(hotkey_object, "stop", &defaults.hotkeys.stop),
        },
    }
}

fn sanitize_hotkey_field(object: Option<&Map<String, Value>>, key: &str, fallback: &str) -> String {
    object
        .and_then(|hotkeys| hotkeys.get(key))
        .and_then(Value::as_str)
        .map(normalize_hotkey)
        .unwrap_or_else(|| fallback.into())
}

fn clamp_optional_number(value: Option<&Value>, fallback: f64, min: f64, max: f64) -> f64 {
    clamp_f64(finite_number(value).unwrap_or(f64::NAN), fallback, min, max)
}

pub fn clamp_f64(value: f64, fallback: f64, min: f64, max: f64) -> f64 {
    if !value.is_finite() {
        fallback
    } else {
        value.clamp(min, max)
    }
}

fn finite_number(value: Option<&Value>) -> Option<f64> {
    let number = match value? {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Some(0.0)
            } else {
                trimmed.parse().ok()
            }
        }
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        Value::Null => Some(0.0),
        _ => None,
    }?;
    number.is_finite().then_some(number)
}

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub fn normalize_key(value: &str) -> String {
    let key = value.trim();
    if key.chars().count() == 1 {
        return key.to_uppercase();
    }
    match key {
        "Escape" => "Esc".into(),
        " " => "Space".into(),
        _ => key.into(),
    }
}

pub fn normalize_hotkey(value: &str) -> String {
    let compact: String = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let mut parts = compact.split('+').map(str::to_owned).collect::<Vec<_>>();
    for part in &mut parts {
        if part.eq_ignore_ascii_case("ctrl") {
            *part = "CommandOrControl".into();
        }
    }
    parts.join("+")
}

pub fn sanitize_modifiers(value: Option<&Value>) -> Vec<KeyModifier> {
    let Some(values) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    [
        ("Control", KeyModifier::Control),
        ("Alt", KeyModifier::Alt),
        ("Shift", KeyModifier::Shift),
    ]
    .into_iter()
    .filter_map(|(name, modifier)| {
        values
            .iter()
            .any(|value| value.as_str() == Some(name))
            .then_some(modifier)
    })
    .collect()
}

pub fn sanitize_modifier_list(value: Vec<KeyModifier>) -> Vec<KeyModifier> {
    [KeyModifier::Control, KeyModifier::Alt, KeyModifier::Shift]
        .into_iter()
        .filter(|modifier| value.contains(modifier))
        .collect()
}

pub fn virtual_key_code(key: &str) -> Option<u16> {
    let bytes = key.as_bytes();
    if bytes.len() == 1 && (bytes[0].is_ascii_uppercase() || bytes[0].is_ascii_digit()) {
        return Some(bytes[0] as u16);
    }

    match key {
        "Backspace" => Some(0x08),
        "Tab" => Some(0x09),
        "Enter" => Some(0x0d),
        "Esc" => Some(0x1b),
        "Space" => Some(0x20),
        "PageUp" => Some(0x21),
        "PageDown" => Some(0x22),
        "End" => Some(0x23),
        "Home" => Some(0x24),
        "ArrowLeft" => Some(0x25),
        "ArrowUp" => Some(0x26),
        "ArrowRight" => Some(0x27),
        "ArrowDown" => Some(0x28),
        "Insert" => Some(0x2d),
        "Delete" => Some(0x2e),
        _ => key
            .strip_prefix('F')
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|value| (1..=24).contains(value))
            .map(|value| 0x70 + value - 1),
    }
}

pub fn format_key_step(key: &str, modifiers: &[KeyModifier]) -> String {
    modifiers
        .iter()
        .map(|modifier| modifier.label().to_owned())
        .chain(std::iter::once(key.to_owned()))
        .collect::<Vec<_>>()
        .join("+")
}

pub fn key_step_conflicts_with_hotkey(
    key: &str,
    modifiers: &[KeyModifier],
    hotkeys: &Hotkeys,
) -> bool {
    let accelerator = modifiers
        .iter()
        .map(|modifier| modifier.accelerator_label().to_owned())
        .chain(std::iter::once(key.to_owned()))
        .collect::<Vec<_>>()
        .join("+")
        .to_lowercase();
    [
        EMERGENCY_STOP_HOTKEY,
        &hotkeys.capture,
        &hotkeys.start,
        &hotkeys.stop,
    ]
    .iter()
    .any(|hotkey| hotkey.to_lowercase().replace("commandorcontrol", "control") == accelerator)
}

pub fn validate_hotkeys(hotkeys: &Hotkeys) -> Vec<String> {
    let entries = [
        ("capture", &hotkeys.capture),
        ("start", &hotkeys.start),
        ("stop", &hotkeys.stop),
    ];
    let mut errors = Vec::new();
    let mut seen: Vec<(String, &str)> = Vec::new();

    for (name, accelerator) in entries {
        if accelerator.is_empty() {
            errors.push(format!("热键不能为空：{name}"));
            continue;
        }
        let key = accelerator.to_lowercase();
        if key == EMERGENCY_STOP_HOTKEY.to_lowercase() {
            errors.push(format!("热键 {accelerator} 已保留为紧急停止"));
            continue;
        }
        if let Some((_, previous_name)) = seen.iter().find(|(value, _)| value == &key) {
            errors.push(format!(
                "热键重复：{previous_name} 与 {name} 都使用 {accelerator}"
            ));
            continue;
        }
        seen.push((key, name));
    }
    errors
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn legacy_json_is_loaded_and_sanitized() {
        let raw = json!({
            "activeProfileId": "legacy",
            "profiles": [{
                "id": "legacy",
                "name": "  旧方案  ",
                "points": [
                    {
                        "id": "click-1",
                        "label": "坐标",
                        "action": "click",
                        "x": "120.7",
                        "y": -42.4,
                        "delaySeconds": 99999,
                        "createdAt": 123
                    },
                    {
                        "action": "key",
                        "key": "Escape",
                        "modifiers": ["Shift", "Control", "Shift"],
                        "delaySeconds": 0
                    },
                    { "action": "click", "x": "not-a-number", "y": 1 }
                ],
                "settings": {
                    "clickIntervalSeconds": 0,
                    "loopIntervalSeconds": -10,
                    "startDelaySeconds": 80,
                    "loopMode": "infinite",
                    "loopCount": 12000,
                    "hotkeys": {
                        "capture": " Ctrl + Alt + Q ",
                        "start": "Ctrl+Alt+P",
                        "stop": "Ctrl+Alt+O"
                    }
                },
                "createdAt": 100,
                "updatedAt": 200
            }]
        });

        let store = sanitize_persisted(&raw);
        assert_eq!(store.active_profile_id, "legacy");
        assert_eq!(store.profiles.len(), 1);
        let profile = &store.profiles[0];
        assert_eq!(profile.name, "旧方案");
        assert_eq!(profile.points.len(), 2);
        assert!(profile.points[0].enabled);
        assert!(profile.points[1].enabled);
        assert_eq!(profile.points[0].x, 121);
        assert_eq!(profile.points[0].y, -42);
        assert_eq!(profile.points[0].delay_seconds, 3600.0);
        assert_eq!(profile.points[1].key, "Esc");
        assert_eq!(
            profile.points[1].modifiers,
            vec![KeyModifier::Control, KeyModifier::Shift]
        );
        assert_eq!(profile.settings.click_interval_seconds, 0.1);
        assert_eq!(profile.settings.loop_interval_seconds, 0.0);
        assert_eq!(profile.settings.start_delay_seconds, 60.0);
        assert_eq!(profile.settings.loop_count, 9999);
        assert_eq!(profile.settings.hotkeys.capture, "CommandOrControl+Alt+Q");
        assert_eq!(store.appearance, default_appearance());
    }

    #[test]
    fn point_actions_and_enabled_state_are_compatible_and_round_trip() {
        let double_click = sanitize_point(
            &json!({
                "id": "double-click",
                "label": "双击",
                "action": "doubleClick",
                "enabled": false,
                "x": -320,
                "y": 240,
                "delaySeconds": 0.5,
                "createdAt": 123
            }),
            0,
        )
        .expect("valid double-click point");
        assert_eq!(double_click.action, PointAction::DoubleClick);
        assert!(!double_click.enabled);
        assert_eq!((double_click.x, double_click.y), (-320, 240));

        let serialized = serde_json::to_value(&double_click).expect("serialize point");
        assert_eq!(serialized["action"], "doubleClick");
        assert_eq!(serialized["enabled"], false);
        let deserialized: Point =
            serde_json::from_value(serialized).expect("deserialize serialized point");
        assert_eq!(deserialized, double_click);

        let legacy: Point = serde_json::from_value(json!({
            "id": "legacy",
            "label": "旧步骤",
            "action": "click",
            "x": 1,
            "y": 2,
            "key": "",
            "modifiers": [],
            "delaySeconds": 0.5,
            "createdAt": 123
        }))
        .expect("deserialize legacy point");
        assert!(legacy.enabled);

        let invalid_enabled = sanitize_point(
            &json!({
                "action": "click",
                "enabled": "false",
                "x": 1,
                "y": 2
            }),
            1,
        )
        .expect("sanitize click point");
        assert!(invalid_enabled.enabled);

        assert!(
            sanitize_point(
                &json!({ "action": "doubleClick", "x": "invalid", "y": 2 }),
                2
            )
            .is_none()
        );
    }

    #[test]
    fn invalid_store_falls_back_to_default_profile() {
        let store = sanitize_persisted(&json!({ "profiles": [null, 42] }));
        assert_eq!(store.profiles.len(), 1);
        assert_eq!(store.profiles[0].name, DEFAULT_PROFILE_NAME);
        assert_eq!(store.active_profile_id, store.profiles[0].id);
        assert_eq!(store.profiles[0].settings, default_settings());
        assert_eq!(store.appearance, default_appearance());
        assert!(!store.restricted_workspaces_unlocked);
        assert_eq!(store.mystery_code, None);
        assert_eq!(store.ai_base_url, DEFAULT_AI_BASE_URL);
        assert_eq!(store.ai_api_key, None);
    }

    #[test]
    fn restricted_workspace_access_defaults_to_locked_and_round_trips() {
        let locked = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }]
        }));
        assert!(!locked.restricted_workspaces_unlocked);

        let unlocked = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "restrictedWorkspacesUnlocked": true
        }));
        assert!(unlocked.restricted_workspaces_unlocked);

        let serialized = serde_json::to_value(&unlocked).expect("serialize store");
        assert_eq!(serialized["restrictedWorkspacesUnlocked"], true);
    }

    #[test]
    fn mystery_code_is_trimmed_and_persisted_globally() {
        let store = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "mysteryCode": "  shree  "
        }));
        assert_eq!(store.mystery_code.as_deref(), Some("shree"));

        let serialized = serde_json::to_value(&store).expect("serialize store");
        assert_eq!(serialized["mysteryCode"], "shree");
        let profile = serde_json::to_value(&store.profiles[0]).expect("serialize profile");
        assert!(profile.get("mysteryCode").is_none());
    }

    #[test]
    fn ai_base_url_defaults_and_is_persisted_globally() {
        let default_store = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }]
        }));
        assert_eq!(default_store.ai_base_url, DEFAULT_AI_BASE_URL);

        let store = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "aiBaseUrl": "  https://example.com/  "
        }));
        assert_eq!(store.ai_base_url, "https://example.com");
        let serialized = serde_json::to_value(&store).expect("serialize store");
        assert_eq!(serialized["aiBaseUrl"], "https://example.com");
    }

    #[test]
    fn ai_key_is_trimmed_and_legacy_model_is_discarded() {
        let store = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "aiApiKey": "  sk-custom  ",
            "aiModel": "  vision-custom  "
        }));
        assert_eq!(store.ai_api_key.as_deref(), Some("sk-custom"));
        let serialized = serde_json::to_value(&store).expect("serialize store");
        assert!(serialized.get("aiModel").is_none());
    }

    #[test]
    fn appearance_is_loaded_and_unknown_themes_fall_back_to_longyin() {
        let valid = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "appearance": { "themeId": " default ", "cleanMode": true }
        }));
        assert_eq!(valid.appearance.theme_id, "default");
        assert!(valid.appearance.clean_mode);

        let chaoguang = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "appearance": { "themeId": " chaoguang ", "cleanMode": false }
        }));
        assert_eq!(chaoguang.appearance.theme_id, "chaoguang");
        assert!(!chaoguang.appearance.clean_mode);

        let xuehe = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "appearance": { "themeId": " xuehe ", "cleanMode": false }
        }));
        assert_eq!(xuehe.appearance.theme_id, "xuehe");
        assert!(!xuehe.appearance.clean_mode);

        let jiuling = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "appearance": { "themeId": " jiuling ", "cleanMode": false }
        }));
        assert_eq!(jiuling.appearance.theme_id, "jiuling");
        assert!(!jiuling.appearance.clean_mode);

        let suwen = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "appearance": { "themeId": " suwen ", "cleanMode": false }
        }));
        assert_eq!(suwen.appearance.theme_id, "suwen");
        assert!(!suwen.appearance.clean_mode);

        let shenxiang = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "appearance": { "themeId": " shenxiang ", "cleanMode": false }
        }));
        assert_eq!(shenxiang.appearance.theme_id, "shenxiang");
        assert!(!shenxiang.appearance.clean_mode);

        let unknown = sanitize_persisted(&json!({
            "profiles": [{ "id": "profile", "name": "方案" }],
            "appearance": { "themeId": "future-theme", "cleanMode": true }
        }));
        assert_eq!(unknown.appearance.theme_id, DEFAULT_THEME_ID);
        assert!(unknown.appearance.clean_mode);

        let jiuling_patch = patch_appearance(
            &valid.appearance,
            &AppearancePatch {
                theme_id: Some(" jiuling ".into()),
                clean_mode: None,
            },
        );
        assert_eq!(jiuling_patch.theme_id, "jiuling");
        assert!(jiuling_patch.clean_mode);

        let suwen_patch = patch_appearance(
            &valid.appearance,
            &AppearancePatch {
                theme_id: Some(" suwen ".into()),
                clean_mode: None,
            },
        );
        assert_eq!(suwen_patch.theme_id, "suwen");
        assert!(suwen_patch.clean_mode);

        let shenxiang_patch = patch_appearance(
            &valid.appearance,
            &AppearancePatch {
                theme_id: Some(" shenxiang ".into()),
                clean_mode: None,
            },
        );
        assert_eq!(shenxiang_patch.theme_id, "shenxiang");
        assert!(shenxiang_patch.clean_mode);

        let clean_mode_patch = patch_appearance(
            &shenxiang_patch,
            &AppearancePatch {
                theme_id: None,
                clean_mode: Some(false),
            },
        );
        assert_eq!(clean_mode_patch.theme_id, "shenxiang");
        assert!(!clean_mode_patch.clean_mode);

        let unknown_patch = patch_appearance(
            &shenxiang_patch,
            &AppearancePatch {
                theme_id: Some(" future-theme ".into()),
                clean_mode: None,
            },
        );
        assert_eq!(unknown_patch.theme_id, DEFAULT_THEME_ID);
        assert!(unknown_patch.clean_mode);

        let patched = patch_appearance(
            &unknown.appearance,
            &AppearancePatch {
                theme_id: Some(String::new()),
                clean_mode: None,
            },
        );
        assert_eq!(patched.theme_id, DEFAULT_THEME_ID);
        assert!(patched.clean_mode);

        assert_eq!(sanitize_theme_id(Some("   ")), DEFAULT_THEME_ID);
    }

    #[test]
    fn profile_serialization_and_import_ignore_global_appearance() {
        let store = create_default_profile_store();
        let exported = serde_json::to_value(&store.profiles[0]).expect("serialize profile");
        assert!(exported.get("appearance").is_none());

        let mut value = exported;
        value["appearance"] = json!({ "themeId": "default", "cleanMode": true });
        let imported = sanitize_profile(&value, "导入方案").expect("sanitize profile");
        let reexported = serde_json::to_value(imported).expect("serialize imported profile");
        assert!(reexported.get("appearance").is_none());
    }

    #[test]
    fn hotkey_conflicts_and_reserved_shortcut_are_detected() {
        let duplicate = Hotkeys {
            capture: "CommandOrControl+Alt+Q".into(),
            start: "CommandOrControl+Alt+Q".into(),
            stop: EMERGENCY_STOP_HOTKEY.into(),
        };
        let errors = validate_hotkeys(&duplicate);
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().any(|error| error.contains("热键重复")));
        assert!(errors.iter().any(|error| error.contains("紧急停止")));

        assert!(key_step_conflicts_with_hotkey(
            "Q",
            &[KeyModifier::Control, KeyModifier::Alt],
            &default_settings().hotkeys
        ));
    }

    #[test]
    fn virtual_key_mapping_matches_supported_keys() {
        assert_eq!(virtual_key_code("A"), Some(0x41));
        assert_eq!(virtual_key_code("9"), Some(0x39));
        assert_eq!(virtual_key_code("F24"), Some(0x87));
        assert_eq!(virtual_key_code("ArrowLeft"), Some(0x25));
        assert_eq!(virtual_key_code("F25"), None);
        assert_eq!(virtual_key_code("a"), None);
    }
}

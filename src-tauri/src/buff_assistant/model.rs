use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 10;
pub const MAX_LISTENERS: usize = 8;
pub const DEFAULT_CYCLE_MS: u64 = 20_000;
pub const DEFAULT_DEADLINE_GRACE_MS: u64 = 1_500;
const PREVIOUS_DEFAULT_CYCLE_MS: u64 = 20_180;
pub const DEFAULT_THRESHOLD: f32 = 0.95;
const LEGACY_DEFAULT_THRESHOLD: f32 = 0.86;
pub const DEFAULT_CONFIRM_FRAMES: u32 = 3;
pub const DEFAULT_MISSING_FRAMES: u32 = 5;
pub const DEFAULT_OVERLAY_WIDTH: u32 = 330;
pub const DEFAULT_OVERLAY_HEIGHT: u32 = 92;
pub const MIN_OVERLAY_WIDTH: u32 = 75;
pub const MIN_OVERLAY_HEIGHT: u32 = 30;
pub const MAX_OVERLAY_WIDTH: u32 = 800;
pub const MAX_OVERLAY_HEIGHT: u32 = 520;
pub const DEFAULT_MONITOR_HOTKEY: &str = "Ctrl+Alt+F10";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuffMatchMode {
    #[default]
    Pixel,
    BrightText,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl NormalizedRect {
    pub fn sanitized(self) -> Self {
        let x = finite_or(self.x, 0.0).clamp(0.0, 0.99);
        let y = finite_or(self.y, 0.0).clamp(0.0, 0.99);
        let width = finite_or(self.width, 1.0).clamp(0.01, 1.0 - x);
        let height = finite_or(self.height, 1.0).clamp(0.01, 1.0 - y);
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn pixel_bounds(self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let rect = self.sanitized();
        let start_x = (rect.x * f64::from(width)).floor() as u32;
        let start_y = (rect.y * f64::from(height)).floor() as u32;
        let end_x = ((rect.x + rect.width) * f64::from(width)).ceil() as u32;
        let end_y = ((rect.y + rect.height) * f64::from(height)).ceil() as u32;
        (
            start_x.min(width.saturating_sub(1)),
            start_y.min(height.saturating_sub(1)),
            end_x.clamp(start_x.saturating_add(1), width),
            end_y.clamp(start_y.saturating_add(1), height),
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffTarget {
    pub process_name: String,
    pub window_title: String,
    pub class_name: String,
    pub reference_width: u32,
    pub reference_height: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffTemplateSummary {
    pub id: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub crop: Option<NormalizedRect>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffTemplatePreview {
    pub image_data_url: String,
    pub mask_data_url: String,
    pub source_data_url: Option<String>,
    pub crop: Option<NormalizedRect>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuffSoundCue {
    Triggered,
    PrewarnThree,
    PrewarnTwo,
    PrewarnOne,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffSoundTemplateSummary {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffCustomSoundAsset {
    pub asset_id: String,
    pub file_name: String,
}

const DEFAULT_SOUND_TEMPLATE_ID: &str = "template-1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BuffSoundSource {
    Sine,
    Template {
        #[serde(rename = "templateId")]
        template_id: String,
    },
    Custom {
        #[serde(rename = "assetId")]
        asset_id: String,
        #[serde(rename = "fileName")]
        file_name: String,
    },
}

impl Default for BuffSoundSource {
    fn default() -> Self {
        Self::Template {
            template_id: DEFAULT_SOUND_TEMPLATE_ID.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffSoundSettings {
    pub trigger_enabled: bool,
    pub prewarn_three_enabled: bool,
    #[serde(default = "enabled_by_default")]
    pub prewarn_two_enabled: bool,
    pub prewarn_one_enabled: bool,
    #[serde(default)]
    pub trigger_source: BuffSoundSource,
    #[serde(default)]
    pub prewarn_three_source: BuffSoundSource,
    #[serde(default)]
    pub prewarn_two_source: BuffSoundSource,
    #[serde(default)]
    pub prewarn_one_source: BuffSoundSource,
    pub volume: f32,
}

impl Default for BuffSoundSettings {
    fn default() -> Self {
        Self {
            trigger_enabled: true,
            prewarn_three_enabled: true,
            prewarn_two_enabled: true,
            prewarn_one_enabled: true,
            trigger_source: BuffSoundSource::default(),
            prewarn_three_source: BuffSoundSource::default(),
            prewarn_two_source: BuffSoundSource::default(),
            prewarn_one_source: BuffSoundSource::default(),
            volume: 0.45,
        }
    }
}

impl BuffSoundSettings {
    pub fn source(&self, cue: BuffSoundCue) -> &BuffSoundSource {
        match cue {
            BuffSoundCue::Triggered => &self.trigger_source,
            BuffSoundCue::PrewarnThree => &self.prewarn_three_source,
            BuffSoundCue::PrewarnTwo => &self.prewarn_two_source,
            BuffSoundCue::PrewarnOne => &self.prewarn_one_source,
        }
    }

    pub fn source_mut(&mut self, cue: BuffSoundCue) -> &mut BuffSoundSource {
        match cue {
            BuffSoundCue::Triggered => &mut self.trigger_source,
            BuffSoundCue::PrewarnThree => &mut self.prewarn_three_source,
            BuffSoundCue::PrewarnTwo => &mut self.prewarn_two_source,
            BuffSoundCue::PrewarnOne => &mut self.prewarn_one_source,
        }
    }
}

const fn enabled_by_default() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffOverlaySettings {
    pub x: i32,
    pub y: i32,
    pub show_waiting_dot: bool,
    #[serde(default)]
    pub exclude_from_capture: bool,
    #[serde(default = "default_overlay_width")]
    pub width: u32,
    #[serde(default = "default_overlay_height")]
    pub height: u32,
    #[serde(default)]
    pub color_scheme: BuffOverlayColorScheme,
}

impl Default for BuffOverlaySettings {
    fn default() -> Self {
        Self {
            x: 40,
            y: 100,
            show_waiting_dot: false,
            exclude_from_capture: false,
            width: DEFAULT_OVERLAY_WIDTH,
            height: DEFAULT_OVERLAY_HEIGHT,
            color_scheme: BuffOverlayColorScheme::BlackWhite,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffCaptureSettings {
    #[serde(default = "enabled_by_default")]
    pub show_system_border: bool,
}

impl Default for BuffCaptureSettings {
    fn default() -> Self {
        Self {
            show_system_border: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuffOverlayColorScheme {
    Gold,
    #[default]
    BlackWhite,
}

const fn default_overlay_width() -> u32 {
    DEFAULT_OVERLAY_WIDTH
}

const fn default_overlay_height() -> u32 {
    DEFAULT_OVERLAY_HEIGHT
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffAssistantSettings {
    pub cycle_ms: u64,
    #[serde(default = "default_deadline_grace_ms")]
    pub deadline_grace_ms: u64,
    pub threshold: f32,
    pub confirm_frames: u32,
    pub missing_frames: u32,
    pub sound: BuffSoundSettings,
    pub overlay: BuffOverlaySettings,
    #[serde(default)]
    pub capture: BuffCaptureSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffListenerSettings {
    pub cycle_ms: u64,
    #[serde(default = "default_deadline_grace_ms")]
    pub deadline_grace_ms: u64,
    #[serde(default)]
    pub match_mode: BuffMatchMode,
    pub threshold: f32,
    pub confirm_frames: u32,
    pub missing_frames: u32,
    pub sound: BuffSoundSettings,
}

impl Default for BuffListenerSettings {
    fn default() -> Self {
        let settings = BuffAssistantSettings::default();
        Self {
            cycle_ms: settings.cycle_ms,
            deadline_grace_ms: settings.deadline_grace_ms,
            match_mode: BuffMatchMode::Pixel,
            threshold: settings.threshold,
            confirm_frames: settings.confirm_frames,
            missing_frames: settings.missing_frames,
            sound: settings.sound,
        }
    }
}

impl BuffListenerSettings {
    pub fn sanitize(&mut self) {
        let mut settings = BuffAssistantSettings {
            cycle_ms: self.cycle_ms,
            deadline_grace_ms: self.deadline_grace_ms,
            threshold: self.threshold,
            confirm_frames: self.confirm_frames,
            missing_frames: self.missing_frames,
            sound: self.sound.clone(),
            overlay: BuffOverlaySettings::default(),
            capture: BuffCaptureSettings::default(),
        };
        settings.sanitize();
        self.cycle_ms = settings.cycle_ms;
        self.deadline_grace_ms = settings.deadline_grace_ms;
        self.threshold = settings.threshold;
        self.confirm_frames = settings.confirm_frames;
        self.missing_frames = settings.missing_frames;
        self.sound = settings.sound;
    }
}

impl From<&BuffAssistantSettings> for BuffListenerSettings {
    fn from(settings: &BuffAssistantSettings) -> Self {
        Self {
            cycle_ms: settings.cycle_ms,
            deadline_grace_ms: settings.deadline_grace_ms,
            match_mode: BuffMatchMode::Pixel,
            threshold: settings.threshold,
            confirm_frames: settings.confirm_frames,
            missing_frames: settings.missing_frames,
            sound: settings.sound.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffGlobalSettings {
    pub overlay: BuffOverlaySettings,
    #[serde(default)]
    pub capture: BuffCaptureSettings,
    #[serde(default = "default_monitor_hotkey")]
    pub monitor_hotkey: Option<String>,
}

impl Default for BuffGlobalSettings {
    fn default() -> Self {
        Self {
            overlay: BuffOverlaySettings::default(),
            capture: BuffCaptureSettings::default(),
            monitor_hotkey: default_monitor_hotkey(),
        }
    }
}

impl BuffGlobalSettings {
    pub fn sanitize(&mut self) {
        self.overlay.width = self
            .overlay
            .width
            .clamp(MIN_OVERLAY_WIDTH, MAX_OVERLAY_WIDTH);
        self.overlay.height = self
            .overlay
            .height
            .clamp(MIN_OVERLAY_HEIGHT, MAX_OVERLAY_HEIGHT);
        self.monitor_hotkey = self
            .monitor_hotkey
            .as_deref()
            .map(str::trim)
            .filter(|shortcut| !shortcut.is_empty())
            .map(str::to_string);
    }
}

fn default_monitor_hotkey() -> Option<String> {
    Some(DEFAULT_MONITOR_HOTKEY.to_string())
}

impl From<&BuffAssistantSettings> for BuffGlobalSettings {
    fn from(settings: &BuffAssistantSettings) -> Self {
        Self {
            overlay: settings.overlay.clone(),
            capture: settings.capture.clone(),
            monitor_hotkey: default_monitor_hotkey(),
        }
    }
}

impl Default for BuffAssistantSettings {
    fn default() -> Self {
        Self {
            cycle_ms: DEFAULT_CYCLE_MS,
            deadline_grace_ms: DEFAULT_DEADLINE_GRACE_MS,
            threshold: DEFAULT_THRESHOLD,
            confirm_frames: DEFAULT_CONFIRM_FRAMES,
            missing_frames: DEFAULT_MISSING_FRAMES,
            sound: BuffSoundSettings::default(),
            overlay: BuffOverlaySettings::default(),
            capture: BuffCaptureSettings::default(),
        }
    }
}

impl BuffAssistantSettings {
    pub fn sanitize(&mut self) {
        self.cycle_ms = self.cycle_ms.clamp(5_000, 120_000);
        self.deadline_grace_ms = self.deadline_grace_ms.min(2_000);
        self.threshold = if self.threshold.is_finite() {
            self.threshold.clamp(0.5, 0.99)
        } else {
            DEFAULT_THRESHOLD
        };
        self.confirm_frames = self.confirm_frames.clamp(1, 12);
        self.missing_frames = self.missing_frames.clamp(1, 30);
        self.sound.volume = if self.sound.volume.is_finite() {
            self.sound.volume.clamp(0.0, 1.0)
        } else {
            BuffSoundSettings::default().volume
        };
        self.overlay.width = self
            .overlay
            .width
            .clamp(MIN_OVERLAY_WIDTH, MAX_OVERLAY_WIDTH);
        self.overlay.height = self
            .overlay
            .height
            .clamp(MIN_OVERLAY_HEIGHT, MAX_OVERLAY_HEIGHT);
    }
}

const fn default_deadline_grace_ms() -> u64 {
    DEFAULT_DEADLINE_GRACE_MS
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffAssistantConfig {
    pub schema_version: u32,
    pub target: Option<BuffTarget>,
    pub search_region: Option<NormalizedRect>,
    #[serde(default)]
    pub listeners: Vec<BuffListenerConfig>,
    #[serde(default)]
    pub settings: BuffGlobalSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffListenerConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    pub template: Option<BuffTemplateSummary>,
    pub settings: BuffListenerSettings,
}

impl BuffListenerConfig {
    pub fn sanitize(&mut self) {
        self.id = self
            .id
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
            .collect();
        self.name = self.name.trim().chars().take(20).collect();
        self.settings.sanitize();
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyBuffAssistantConfig {
    pub schema_version: u32,
    pub target: Option<BuffTarget>,
    pub search_region: Option<NormalizedRect>,
    pub template: Option<BuffTemplateSummary>,
    pub settings: BuffAssistantSettings,
}

impl Default for BuffAssistantConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            target: None,
            search_region: None,
            listeners: Vec::new(),
            settings: BuffGlobalSettings::default(),
        }
    }
}

impl BuffAssistantConfig {
    pub fn sanitize(&mut self) {
        self.schema_version = CONFIG_SCHEMA_VERSION;
        self.search_region = self.search_region.map(NormalizedRect::sanitized);
        self.settings.sanitize();
        self.listeners.truncate(MAX_LISTENERS);
        for listener in &mut self.listeners {
            listener.sanitize();
        }
        if let Some(target) = &mut self.target {
            target.process_name = target.process_name.trim().to_string();
            target.window_title = target.window_title.trim().to_string();
            target.class_name = target.class_name.trim().to_string();
            target.reference_width = target.reference_width.max(1);
            target.reference_height = target.reference_height.max(1);
        }
    }
}

impl LegacyBuffAssistantConfig {
    pub fn migrate(mut self) -> BuffAssistantConfig {
        if self.schema_version < 2
            && (self.settings.threshold - LEGACY_DEFAULT_THRESHOLD).abs() < 0.000_1
        {
            self.settings.threshold = DEFAULT_THRESHOLD;
        }
        if self.schema_version < 4 && self.settings.cycle_ms == PREVIOUS_DEFAULT_CYCLE_MS {
            self.settings.cycle_ms = DEFAULT_CYCLE_MS;
        }
        self.settings.sanitize();
        let listeners = self
            .template
            .map(|template| BuffListenerConfig {
                id: "jinzhoutian".into(),
                name: "金周天".into(),
                enabled: true,
                template: Some(template),
                settings: BuffListenerSettings::from(&self.settings),
            })
            .into_iter()
            .collect();
        let mut config = BuffAssistantConfig {
            schema_version: CONFIG_SCHEMA_VERSION,
            target: self.target,
            search_region: self.search_region,
            listeners,
            settings: BuffGlobalSettings::from(&self.settings),
        };
        config.sanitize();
        config
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuffAssistantActivity {
    Stopped,
    Waiting,
    Tracking,
    Prewarning,
    Confirming,
    Testing,
    TargetUnavailable,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffAssistantState {
    pub config: BuffAssistantConfig,
    pub activity: BuffAssistantActivity,
    pub is_monitoring: bool,
    pub listeners: Vec<BuffListenerRuntimeState>,
    pub last_error: Option<String>,
    pub capture_border_supported: bool,
    pub capture_border_notice: Option<String>,
    pub hotkey_registration_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffListenerRuntimeState {
    pub id: String,
    pub activity: BuffAssistantActivity,
    pub expected_at_unix_ms: Option<i64>,
    pub last_confidence: f32,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderlessCaptureAccessResult {
    Allowed,
    Unsupported,
    DeniedByUser,
    DeniedBySystem,
    NotDeclared,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureWindowCandidate {
    pub id: String,
    pub process_name: String,
    pub window_title: String,
    pub class_name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePreview {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub target: BuffTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuffOverlayMode {
    Hidden,
    Waiting,
    Triggered,
    Countdown,
    Confirming,
    Reset,
    TargetUnavailable,
    Editing,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffOverlayState {
    pub mode: BuffOverlayMode,
    pub message: String,
    #[serde(default)]
    pub items: Vec<BuffOverlayItem>,
    pub emitted_at_unix_ms: i64,
    pub editable: bool,
    pub color_scheme: BuffOverlayColorScheme,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffOverlayItem {
    pub listener_id: String,
    pub name: String,
    pub mode: BuffOverlayMode,
    pub expected_at_unix_ms: Option<i64>,
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_deadline_grace_is_1500_ms() {
        assert_eq!(BuffAssistantSettings::default().deadline_grace_ms, 1_500);
    }

    #[test]
    fn listener_match_mode_defaults_to_pixel_when_missing() {
        let mut value = serde_json::to_value(BuffListenerSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("matchMode");

        let settings: BuffListenerSettings = serde_json::from_value(value).unwrap();

        assert_eq!(settings.match_mode, BuffMatchMode::Pixel);
    }

    #[test]
    fn monitor_hotkey_defaults_when_missing_and_allows_null() {
        let mut value = serde_json::to_value(BuffGlobalSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("monitorHotkey");
        let settings: BuffGlobalSettings = serde_json::from_value(value).unwrap();
        assert_eq!(
            settings.monitor_hotkey.as_deref(),
            Some(DEFAULT_MONITOR_HOTKEY)
        );

        let mut value = serde_json::to_value(BuffGlobalSettings::default()).unwrap();
        value["monitorHotkey"] = serde_json::Value::Null;
        let settings: BuffGlobalSettings = serde_json::from_value(value).unwrap();
        assert_eq!(settings.monitor_hotkey, None);
    }

    #[test]
    fn bright_text_match_mode_serializes_as_camel_case() {
        let mut settings = BuffListenerSettings::default();
        settings.match_mode = BuffMatchMode::BrightText;

        let value = serde_json::to_value(&settings).unwrap();
        let restored: BuffListenerSettings = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(value["matchMode"], "brightText");
        assert_eq!(restored.match_mode, BuffMatchMode::BrightText);
    }

    #[test]
    fn normalized_rect_is_clamped_inside_frame() {
        let rect = NormalizedRect {
            x: -1.0,
            y: 0.9,
            width: 2.0,
            height: 0.5,
        }
        .sanitized();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.9);
        assert_eq!(rect.width, 1.0);
        assert!((rect.height - 0.1).abs() < f64::EPSILON * 4.0);
        assert_eq!(rect.pixel_bounds(100, 100), (0, 90, 100, 100));
    }

    #[test]
    fn settings_are_sanitized() {
        let mut settings = BuffAssistantSettings {
            cycle_ms: 1,
            deadline_grace_ms: 9_000,
            threshold: f32::NAN,
            confirm_frames: 0,
            missing_frames: 100,
            sound: BuffSoundSettings {
                volume: 3.0,
                ..BuffSoundSettings::default()
            },
            overlay: BuffOverlaySettings::default(),
            capture: BuffCaptureSettings::default(),
        };
        settings.sanitize();
        assert_eq!(settings.cycle_ms, 5_000);
        assert_eq!(settings.deadline_grace_ms, 2_000);
        assert_eq!(settings.threshold, DEFAULT_THRESHOLD);
        assert_eq!(settings.confirm_frames, 1);
        assert_eq!(settings.missing_frames, 30);
        assert_eq!(settings.sound.volume, 1.0);
        assert_eq!(settings.overlay.width, DEFAULT_OVERLAY_WIDTH);
        assert_eq!(settings.overlay.height, DEFAULT_OVERLAY_HEIGHT);
    }

    #[test]
    fn legacy_default_threshold_is_migrated_to_ninety_five_percent() {
        let mut settings = BuffAssistantSettings::default();
        settings.threshold = LEGACY_DEFAULT_THRESHOLD;
        let config = legacy_config(1, settings).migrate();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.listeners[0].settings.threshold, DEFAULT_THRESHOLD);
    }

    #[test]
    fn legacy_custom_threshold_is_preserved() {
        let mut settings = BuffAssistantSettings::default();
        settings.threshold = 0.9;
        let config = legacy_config(1, settings).migrate();
        assert_eq!(config.listeners[0].settings.threshold, 0.9);
    }

    #[test]
    fn previous_default_cycle_is_restored_to_twenty_seconds() {
        let mut settings = BuffAssistantSettings::default();
        settings.cycle_ms = PREVIOUS_DEFAULT_CYCLE_MS;
        let config = legacy_config(3, settings).migrate();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.listeners[0].settings.cycle_ms, DEFAULT_CYCLE_MS);
    }

    #[test]
    fn legacy_custom_cycle_is_preserved() {
        let mut settings = BuffAssistantSettings::default();
        settings.cycle_ms = 21_000;
        let config = legacy_config(3, settings).migrate();
        assert_eq!(config.listeners[0].settings.cycle_ms, 21_000);
    }

    fn legacy_config(
        schema_version: u32,
        settings: BuffAssistantSettings,
    ) -> LegacyBuffAssistantConfig {
        LegacyBuffAssistantConfig {
            schema_version,
            target: None,
            search_region: None,
            template: Some(BuffTemplateSummary {
                id: "legacy-template".into(),
                width: 32,
                height: 32,
                crop: None,
            }),
            settings,
        }
    }

    #[test]
    fn legacy_sound_settings_enable_the_two_second_warning() {
        let settings: BuffSoundSettings = serde_json::from_str(
            r#"{"triggerEnabled":true,"prewarnThreeEnabled":true,"prewarnOneEnabled":true,"volume":0.45}"#,
        )
        .unwrap();
        assert!(settings.prewarn_two_enabled);
        let template_one = BuffSoundSource::Template {
            template_id: DEFAULT_SOUND_TEMPLATE_ID.into(),
        };
        assert_eq!(settings.trigger_source, template_one);
        assert_eq!(settings.prewarn_three_source, template_one);
        assert_eq!(settings.prewarn_two_source, template_one);
        assert_eq!(settings.prewarn_one_source, template_one);
    }

    #[test]
    fn sound_sources_use_the_frontend_discriminated_union_shape() {
        let template = serde_json::to_value(BuffSoundSource::Template {
            template_id: "template-1".into(),
        })
        .unwrap();
        let custom = serde_json::to_value(BuffSoundSource::Custom {
            asset_id: "sound-1".into(),
            file_name: "提示.wav".into(),
        })
        .unwrap();
        assert_eq!(
            template,
            serde_json::json!({ "type": "template", "templateId": "template-1" })
        );
        assert_eq!(
            custom,
            serde_json::json!({
                "type": "custom",
                "assetId": "sound-1",
                "fileName": "提示.wav"
            })
        );
    }

    #[test]
    fn legacy_settings_use_the_default_deadline_grace() {
        let settings: BuffAssistantSettings = serde_json::from_str(
            r#"{
                "cycleMs": 20000,
                "threshold": 0.95,
                "confirmFrames": 3,
                "missingFrames": 5,
                "sound": {
                    "triggerEnabled": true,
                    "prewarnThreeEnabled": true,
                    "prewarnTwoEnabled": true,
                    "prewarnOneEnabled": true,
                    "volume": 0.45
                },
                "overlay": { "x": 40, "y": 100, "showWaitingDot": false }
            }"#,
        )
        .unwrap();

        assert_eq!(settings.deadline_grace_ms, DEFAULT_DEADLINE_GRACE_MS);
        assert_eq!(settings.overlay.width, DEFAULT_OVERLAY_WIDTH);
        assert_eq!(settings.overlay.height, DEFAULT_OVERLAY_HEIGHT);
        assert!(!settings.overlay.exclude_from_capture);
        assert!(settings.capture.show_system_border);
        assert_eq!(
            settings.overlay.color_scheme,
            BuffOverlayColorScheme::BlackWhite
        );
    }

    #[test]
    fn overlay_capture_exclusion_uses_the_frontend_field_shape() {
        let mut overlay = BuffOverlaySettings::default();
        overlay.exclude_from_capture = true;

        let value = serde_json::to_value(overlay).unwrap();

        assert_eq!(value["excludeFromCapture"], true);
    }

    #[test]
    fn legacy_settings_default_to_showing_the_system_capture_border() {
        let settings: BuffAssistantSettings = serde_json::from_str(
            r#"{
                "cycleMs": 20000,
                "threshold": 0.95,
                "confirmFrames": 3,
                "missingFrames": 5,
                "sound": {
                    "triggerEnabled": true,
                    "prewarnThreeEnabled": true,
                    "prewarnTwoEnabled": true,
                    "prewarnOneEnabled": true,
                    "volume": 0.45
                },
                "overlay": {
                    "x": 40,
                    "y": 100,
                    "showWaitingDot": false,
                    "showBorder": false
                }
            }"#,
        )
        .unwrap();

        assert!(settings.capture.show_system_border);
    }

    #[test]
    fn overlay_size_is_clamped_to_supported_bounds() {
        let mut settings = BuffAssistantSettings::default();
        settings.overlay.width = 1;
        settings.overlay.height = 1_000;
        settings.sanitize();
        assert_eq!(settings.overlay.width, MIN_OVERLAY_WIDTH);
        assert_eq!(settings.overlay.height, MAX_OVERLAY_HEIGHT);
    }
}

use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_CYCLE_MS: u64 = 20_000;
pub const DEFAULT_THRESHOLD: f32 = 0.86;
pub const DEFAULT_CONFIRM_FRAMES: u32 = 3;
pub const DEFAULT_MISSING_FRAMES: u32 = 5;

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
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffSoundSettings {
    pub trigger_enabled: bool,
    pub prewarn_three_enabled: bool,
    pub prewarn_one_enabled: bool,
    pub volume: f32,
}

impl Default for BuffSoundSettings {
    fn default() -> Self {
        Self {
            trigger_enabled: true,
            prewarn_three_enabled: true,
            prewarn_one_enabled: true,
            volume: 0.45,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffOverlaySettings {
    pub x: i32,
    pub y: i32,
    pub show_waiting_dot: bool,
}

impl Default for BuffOverlaySettings {
    fn default() -> Self {
        Self {
            x: 40,
            y: 100,
            show_waiting_dot: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffAssistantSettings {
    pub cycle_ms: u64,
    pub threshold: f32,
    pub confirm_frames: u32,
    pub missing_frames: u32,
    pub sound: BuffSoundSettings,
    pub overlay: BuffOverlaySettings,
}

impl Default for BuffAssistantSettings {
    fn default() -> Self {
        Self {
            cycle_ms: DEFAULT_CYCLE_MS,
            threshold: DEFAULT_THRESHOLD,
            confirm_frames: DEFAULT_CONFIRM_FRAMES,
            missing_frames: DEFAULT_MISSING_FRAMES,
            sound: BuffSoundSettings::default(),
            overlay: BuffOverlaySettings::default(),
        }
    }
}

impl BuffAssistantSettings {
    pub fn sanitize(&mut self) {
        self.cycle_ms = self.cycle_ms.clamp(5_000, 120_000);
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
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffAssistantConfig {
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
            template: None,
            settings: BuffAssistantSettings::default(),
        }
    }
}

impl BuffAssistantConfig {
    pub fn sanitize(&mut self) {
        self.schema_version = CONFIG_SCHEMA_VERSION;
        self.search_region = self.search_region.map(NormalizedRect::sanitized);
        self.settings.sanitize();
        if let Some(target) = &mut self.target {
            target.process_name = target.process_name.trim().to_string();
            target.window_title = target.window_title.trim().to_string();
            target.class_name = target.class_name.trim().to_string();
            target.reference_width = target.reference_width.max(1);
            target.reference_height = target.reference_height.max(1);
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuffAssistantActivity {
    Stopped,
    Waiting,
    Tracking,
    Prewarning,
    CapturingSamples,
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
    pub expected_at_unix_ms: Option<i64>,
    pub last_confidence: f32,
    pub sample_count: usize,
    pub last_error: Option<String>,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleFrameSummary {
    pub id: u64,
    pub captured_at_unix_ms: i64,
    pub width: u32,
    pub height: u32,
    pub thumbnail_data_url: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuffOverlayMode {
    Hidden,
    Waiting,
    Triggered,
    Countdown,
    Reset,
    TargetUnavailable,
    Editing,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffOverlayState {
    pub mode: BuffOverlayMode,
    pub message: String,
    pub expected_at_unix_ms: Option<i64>,
    pub emitted_at_unix_ms: i64,
    pub editable: bool,
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            threshold: f32::NAN,
            confirm_frames: 0,
            missing_frames: 100,
            sound: BuffSoundSettings {
                volume: 3.0,
                ..BuffSoundSettings::default()
            },
            overlay: BuffOverlaySettings::default(),
        };
        settings.sanitize();
        assert_eq!(settings.cycle_ms, 5_000);
        assert_eq!(settings.threshold, DEFAULT_THRESHOLD);
        assert_eq!(settings.confirm_frames, 1);
        assert_eq!(settings.missing_frames, 30);
        assert_eq!(settings.sound.volume, 1.0);
    }
}

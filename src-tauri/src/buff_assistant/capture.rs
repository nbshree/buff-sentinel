use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender as FrameSender, TrySendError, bounded};
use tauri::AppHandle;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl},
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

use super::{
    detector::{
        StablePresenceDetector, TemplateData, bright_text_feature, bright_text_radius,
        match_template, rgba_to_gray_with_buffer,
    },
    model::{BuffMatchMode, NormalizedRect},
};

#[derive(Clone, Default)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapturePurpose {
    Monitor,
    Test,
}

pub type RuntimeCaptureControl = CaptureControl<RuntimeCaptureHandler, String>;

pub struct CaptureOutcome<T> {
    pub value: T,
    pub used_border_fallback: bool,
}

struct SnapshotFlags {
    sender: Sender<Result<CapturedImage, String>>,
}

struct SnapshotHandler {
    sender: Sender<Result<CapturedImage, String>>,
    sent: bool,
}

impl GraphicsCaptureApiHandler for SnapshotHandler {
    type Flags = SnapshotFlags;
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            sender: ctx.flags.sender,
            sent: false,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.sent {
            return Ok(());
        }
        self.sent = true;
        let result = copy_frame(frame, None);
        let _ = self.sender.send(result);
        capture_control.stop();
        Ok(())
    }
}

#[derive(Clone)]
pub struct RuntimeCaptureFlags {
    pub app: AppHandle,
    pub purpose: CapturePurpose,
    /// Deduplicated search regions cropped out of every captured frame, in
    /// capture order. Each listener references one of these by index.
    pub regions: Vec<NormalizedRect>,
    pub listeners: Vec<RuntimeListenerFlags>,
    pub reference_width: u32,
    pub reference_height: u32,
    pub show_system_border: bool,
}

#[derive(Clone)]
pub struct RuntimeListenerFlags {
    pub id: String,
    pub region_index: usize,
    pub template: TemplateData,
    pub match_mode: BuffMatchMode,
    pub threshold: f32,
    pub confirm_frames: u32,
    pub missing_frames: u32,
}

/// Groups the search regions watched by one capture session so that listeners
/// sharing a region are cropped and converted to gray only once per frame.
///
/// Returns the deduplicated regions in first-seen order plus the index of each
/// input region inside that list; the input order is preserved.
pub fn group_regions(regions: Vec<NormalizedRect>) -> (Vec<NormalizedRect>, Vec<usize>) {
    let mut unique: Vec<NormalizedRect> = Vec::new();
    let mut indexes = Vec::with_capacity(regions.len());
    for region in regions {
        let index = match unique.iter().position(|existing| *existing == region) {
            Some(index) => index,
            None => {
                unique.push(region);
                unique.len() - 1
            }
        };
        indexes.push(index);
    }
    (unique, indexes)
}

pub struct RuntimeDetection {
    pub listener_id: String,
    pub confidence: f32,
    pub present: bool,
    pub absence_confirmed: bool,
    pub detected_at: Option<Instant>,
}

pub struct RuntimeCaptureHandler {
    sender: FrameSender<RuntimeFrame>,
    discard_receiver: Receiver<RuntimeFrame>,
    recycle_sender: FrameSender<Vec<CapturedImage>>,
    recycle_receiver: Receiver<Vec<CapturedImage>>,
    padding_buffer: Vec<u8>,
    regions: Vec<NormalizedRect>,
    app: AppHandle,
    purpose: CapturePurpose,
    minimum_frame_interval: Duration,
    last_enqueued_at: Instant,
}

struct RuntimeFrame {
    frame_width: u32,
    frame_height: u32,
    captured_at: Instant,
    /// One cropped image per entry in `RuntimeCaptureFlags::regions`.
    images: Vec<CapturedImage>,
}

struct RuntimeCaptureProcessor {
    flags: RuntimeCaptureFlags,
    listeners: Vec<RuntimeListenerProcessor>,
    detections: Vec<RuntimeDetection>,
    gray_buffers: Vec<Vec<u8>>,
    last_metric_at: Instant,
}

struct RuntimeListenerProcessor {
    flags: RuntimeListenerFlags,
    detector: StablePresenceDetector,
    prepared_template: Option<(f32, TemplateData)>,
    match_started_at: Option<Instant>,
}

impl GraphicsCaptureApiHandler for RuntimeCaptureHandler {
    type Flags = RuntimeCaptureFlags;
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (sender, receiver) = bounded(1);
        let discard_receiver = receiver.clone();
        let (recycle_sender, recycle_receiver) = bounded(2);
        let processor_recycle_sender = recycle_sender.clone();
        let regions = ctx.flags.regions.clone();
        let app = ctx.flags.app.clone();
        let purpose = ctx.flags.purpose;
        let minimum_frame_interval = Duration::from_millis(83);
        let mut processor = RuntimeCaptureProcessor::new(ctx.flags);
        thread::spawn(move || {
            while let Ok(frame) = receiver.recv() {
                let result = processor.process(&frame);
                let _ = processor_recycle_sender.try_send(frame.images);
                if let Err(error) = result {
                    super::handle_capture_error(
                        &processor.flags.app,
                        processor.flags.purpose,
                        error,
                    );
                    return;
                }
            }
        });
        Ok(Self {
            sender,
            discard_receiver,
            recycle_sender,
            recycle_receiver,
            padding_buffer: Vec::new(),
            regions,
            app,
            purpose,
            minimum_frame_interval,
            last_enqueued_at: Instant::now() - minimum_frame_interval,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.last_enqueued_at.elapsed() < self.minimum_frame_interval {
            return Ok(());
        }
        self.last_enqueued_at = Instant::now();
        let mut images = self.recycle_receiver.try_recv().unwrap_or_default();
        images.resize_with(self.regions.len(), CapturedImage::default);
        for (image, region) in images.iter_mut().zip(&self.regions) {
            copy_frame_into(frame, Some(*region), &mut self.padding_buffer, image)?;
        }
        let runtime_frame = RuntimeFrame {
            frame_width: frame.width(),
            frame_height: frame.height(),
            captured_at: Instant::now(),
            images,
        };
        match self.sender.try_send(runtime_frame) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(latest_frame)) => {
                if let Ok(discarded) = self.discard_receiver.try_recv() {
                    let _ = self.recycle_sender.try_send(discarded.images);
                }
                match self.sender.try_send(latest_frame) {
                    Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
                    Err(TrySendError::Disconnected(_)) => {
                        Err("Buff 识别线程已停止，请重新开始监控".into())
                    }
                }
            }
            Err(TrySendError::Disconnected(_)) => Err("Buff 识别线程已停止，请重新开始监控".into()),
        }
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        super::handle_capture_closed(&self.app, self.purpose);
        Ok(())
    }
}

impl RuntimeCaptureProcessor {
    fn new(flags: RuntimeCaptureFlags) -> Self {
        let detections = flags
            .listeners
            .iter()
            .map(|listener| RuntimeDetection {
                listener_id: listener.id.clone(),
                confidence: 0.0,
                present: false,
                absence_confirmed: false,
                detected_at: None,
            })
            .collect();
        let listeners = flags
            .listeners
            .iter()
            .cloned()
            .map(|listener| RuntimeListenerProcessor {
                detector: StablePresenceDetector::new(
                    listener.confirm_frames,
                    listener.missing_frames,
                ),
                flags: listener,
                prepared_template: None,
                match_started_at: None,
            })
            .collect();
        let region_count = flags.regions.len();
        Self {
            flags,
            listeners,
            detections,
            gray_buffers: vec![Vec::new(); region_count],
            last_metric_at: Instant::now() - Duration::from_secs(1),
        }
    }

    fn process(&mut self, frame: &RuntimeFrame) -> Result<(), String> {
        super::handle_capture_frame(&self.flags.app, self.flags.purpose);
        let region_count = self.flags.regions.len();
        for region_index in 0..region_count {
            let region = self.flags.regions[region_index];
            let image = frame
                .images
                .get(region_index)
                .ok_or_else(|| "游戏画面搜索区域数据缺失，请重新开始监控".to_string())?;
            let scale = reference_scale(
                frame.frame_width,
                frame.frame_height,
                self.flags.reference_width,
                self.flags.reference_height,
                region,
            )?;
            let gray_buffer = std::mem::take(&mut self.gray_buffers[region_index]);
            let gray =
                rgba_to_gray_with_buffer(image.width, image.height, &image.rgba, gray_buffer)?;
            let bright_text = self
                .listeners
                .iter()
                .any(|listener| {
                    listener.flags.region_index == region_index
                        && listener.flags.match_mode == BuffMatchMode::BrightText
                })
                .then(|| bright_text_feature(&gray, bright_text_radius(scale)));
            for (listener, detection) in self.listeners.iter_mut().zip(&mut self.detections) {
                if listener.flags.region_index != region_index {
                    continue;
                }
                let threshold = listener.flags.threshold;
                let match_mode = listener.flags.match_mode;
                let template = listener.template_for_scale(scale)?;
                let search = match match_mode {
                    BuffMatchMode::Pixel => &gray,
                    BuffMatchMode::BrightText => bright_text
                        .as_ref()
                        .expect("bright-text listeners create a shared feature image"),
                };
                let confidence = match_template(search, template);
                let matched = confidence >= threshold;
                if matched {
                    listener.match_started_at.get_or_insert(frame.captured_at);
                } else {
                    listener.match_started_at = None;
                }
                let present = listener.detector.update(matched);
                detection.confidence = confidence;
                detection.present = present;
                detection.absence_confirmed = listener.detector.absence_confirmed();
                detection.detected_at = present.then_some(listener.match_started_at).flatten();
            }
            self.gray_buffers[region_index] = gray.into_raw();
        }
        let should_emit_metric = self.last_metric_at.elapsed() >= Duration::from_millis(200);
        if should_emit_metric {
            self.last_metric_at = Instant::now();
        }
        super::handle_detection_batch(
            &self.flags.app,
            self.flags.purpose,
            &self.detections,
            should_emit_metric,
        );
        Ok(())
    }
}

impl RuntimeListenerProcessor {
    fn template_for_scale(&mut self, scale: f32) -> Result<&TemplateData, String> {
        let rebuild = self
            .prepared_template
            .as_ref()
            .is_none_or(|(current, _)| (current - scale).abs() > 0.01);
        if rebuild {
            let template = match self.flags.match_mode {
                BuffMatchMode::Pixel => self.flags.template.scaled(scale),
                BuffMatchMode::BrightText => self.flags.template.bright_text_scaled(scale)?,
            };
            self.prepared_template = Some((scale, template));
        }
        Ok(&self.prepared_template.as_ref().unwrap().1)
    }
}

fn reference_scale(
    frame_width: u32,
    frame_height: u32,
    reference_width: u32,
    reference_height: u32,
    region: NormalizedRect,
) -> Result<f32, String> {
    let width_scale = frame_width as f32 / reference_width.max(1) as f32;
    let height_scale = frame_height as f32 / reference_height.max(1) as f32;
    if scales_match(f64::from(width_scale), f64::from(height_scale)) {
        return Ok((width_scale + height_scale) / 2.0);
    }

    // Older templates stored cropped search-region dimensions as the full-window reference.
    let region = region.sanitized();
    let legacy_width = f64::from(reference_width) / region.width;
    let legacy_height = f64::from(reference_height) / region.height;
    let legacy_width_scale = f64::from(frame_width) / legacy_width.max(1.0);
    let legacy_height_scale = f64::from(frame_height) / legacy_height.max(1.0);
    if scales_match(legacy_width_scale, legacy_height_scale) {
        return Ok(((legacy_width_scale + legacy_height_scale) / 2.0) as f32);
    }

    Err("游戏窗口宽高比变化过大，请重新捕获预览并配置模板".into())
}

fn scales_match(width_scale: f64, height_scale: f64) -> bool {
    (width_scale - height_scale).abs() / width_scale.max(height_scale).max(f64::EPSILON) <= 0.15
}

pub fn capture_border_supported() -> bool {
    let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
    let supported = GraphicsCaptureApi::is_border_settings_supported().unwrap_or(false);
    if initialized {
        unsafe { RoUninitialize() };
    }
    supported
}

fn requested_border_setting(show_system_border: bool) -> DrawBorderSettings {
    border_setting_for(show_system_border, capture_border_supported())
}

fn border_setting_for(
    show_system_border: bool,
    border_settings_supported: bool,
) -> DrawBorderSettings {
    if show_system_border || !border_settings_supported {
        DrawBorderSettings::Default
    } else {
        DrawBorderSettings::WithoutBorder
    }
}

pub fn capture_snapshot(
    window: Window,
    show_system_border: bool,
) -> Result<CaptureOutcome<CapturedImage>, String> {
    let (sender, receiver) = mpsc::channel();
    let requested_border = requested_border_setting(show_system_border);
    let first_result = start_snapshot_capture(window, requested_border, sender.clone());
    let (control, used_border_fallback) = match first_result {
        Ok(control) => (control, false),
        Err(borderless_error) if requested_border == DrawBorderSettings::WithoutBorder => (
            start_snapshot_capture(window, DrawBorderSettings::Default, sender).map_err(
                |error| {
                    format!(
                        "隐藏系统捕获边框失败：{borderless_error}；使用默认边框重试也失败：{error}"
                    )
                },
            )?,
            true,
        ),
        Err(error) => return Err(format!("启动窗口捕获失败：{error}")),
    };
    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => {
            let _ = control.wait();
            result.map(|value| CaptureOutcome {
                value,
                used_border_fallback,
            })
        }
        Err(_) => {
            let _ = control.stop();
            Err("等待游戏窗口画面超时，请确认游戏没有最小化并使用无边框窗口".into())
        }
    }
}

fn start_snapshot_capture(
    window: Window,
    draw_border: DrawBorderSettings,
    sender: Sender<Result<CapturedImage, String>>,
) -> Result<
    CaptureControl<SnapshotHandler, String>,
    windows_capture::capture::GraphicsCaptureApiError<String>,
> {
    SnapshotHandler::start_free_threaded(Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        draw_border,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        SnapshotFlags { sender },
    ))
}

pub fn start_runtime_capture(
    window: Window,
    flags: RuntimeCaptureFlags,
) -> Result<CaptureOutcome<RuntimeCaptureControl>, String> {
    let show_system_border = flags.show_system_border;
    let requested_border = requested_border_setting(show_system_border);
    match start_runtime_capture_with_border(window, flags.clone(), requested_border) {
        Ok(value) => Ok(CaptureOutcome {
            value,
            used_border_fallback: false,
        }),
        Err(borderless_error) if requested_border == DrawBorderSettings::WithoutBorder => {
            let value = start_runtime_capture_with_border(
                window,
                flags,
                DrawBorderSettings::Default,
            )
            .map_err(|error| {
                format!("隐藏系统捕获边框失败：{borderless_error}；使用默认边框重试也失败：{error}")
            })?;
            Ok(CaptureOutcome {
                value,
                used_border_fallback: true,
            })
        }
        Err(error) => Err(format!("启动游戏窗口捕获失败：{error}")),
    }
}

fn start_runtime_capture_with_border(
    window: Window,
    flags: RuntimeCaptureFlags,
    draw_border: DrawBorderSettings,
) -> Result<RuntimeCaptureControl, windows_capture::capture::GraphicsCaptureApiError<String>> {
    RuntimeCaptureHandler::start_free_threaded(Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        draw_border,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        flags,
    ))
}

fn copy_frame(frame: &mut Frame, region: Option<NormalizedRect>) -> Result<CapturedImage, String> {
    let mut padding_buffer = Vec::new();
    let mut image = CapturedImage::default();
    copy_frame_into(frame, region, &mut padding_buffer, &mut image)?;
    Ok(image)
}

fn copy_frame_into(
    frame: &mut Frame,
    region: Option<NormalizedRect>,
    padding_buffer: &mut Vec<u8>,
    image: &mut CapturedImage,
) -> Result<(), String> {
    let buffer = if let Some(region) = region {
        let (start_x, start_y, end_x, end_y) = region.pixel_bounds(frame.width(), frame.height());
        frame
            .buffer_crop(start_x, start_y, end_x, end_y)
            .map_err(|error| format!("裁剪游戏画面失败：{error}"))?
    } else {
        frame
            .buffer()
            .map_err(|error| format!("读取游戏画面失败：{error}"))?
    };
    image.width = buffer.width();
    image.height = buffer.height();
    image.rgba.clear();
    image
        .rgba
        .extend_from_slice(buffer.as_nopadding_buffer(padding_buffer));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_REGION: NormalizedRect = NormalizedRect {
        x: 0.5,
        y: 0.0,
        width: 0.4,
        height: 0.2,
    };

    #[test]
    fn showing_the_system_border_uses_the_windows_default() {
        assert_eq!(border_setting_for(true, true), DrawBorderSettings::Default);
    }

    #[test]
    fn hiding_the_system_border_uses_borderless_capture_when_supported() {
        assert_eq!(
            border_setting_for(false, true),
            DrawBorderSettings::WithoutBorder
        );
    }

    #[test]
    fn hiding_the_system_border_falls_back_when_unsupported() {
        assert_eq!(
            border_setting_for(false, false),
            DrawBorderSettings::Default
        );
    }

    #[test]
    fn reference_scale_accepts_full_window_dimensions() {
        let scale = reference_scale(1920, 1080, 1920, 1080, TEST_REGION).unwrap();
        assert!((scale - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn reference_scale_recovers_legacy_cropped_dimensions() {
        let scale = reference_scale(1920, 1080, 768, 216, TEST_REGION).unwrap();
        assert!((scale - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn reference_scale_rejects_a_real_aspect_ratio_change() {
        assert!(reference_scale(1280, 1024, 1920, 1080, TEST_REGION).is_err());
    }

    #[test]
    fn identical_search_regions_are_cropped_once() {
        let (regions, indexes) = group_regions(vec![TEST_REGION, TEST_REGION]);

        assert_eq!(regions, vec![TEST_REGION]);
        assert_eq!(indexes, vec![0, 0]);
    }

    #[test]
    fn separate_search_regions_produce_one_crop_each() {
        let skill_region = NormalizedRect {
            x: 0.1,
            y: 0.7,
            width: 0.12,
            height: 0.12,
        };

        let (regions, indexes) = group_regions(vec![TEST_REGION, skill_region]);

        assert_eq!(regions, vec![TEST_REGION, skill_region]);
        assert_eq!(indexes, vec![0, 1]);
    }

    #[test]
    fn repeated_search_regions_reuse_the_first_index() {
        let skill_region = NormalizedRect {
            x: 0.1,
            y: 0.7,
            width: 0.12,
            height: 0.12,
        };

        let (regions, indexes) = group_regions(vec![TEST_REGION, skill_region, TEST_REGION]);

        assert_eq!(regions, vec![TEST_REGION, skill_region]);
        assert_eq!(indexes, vec![0, 1, 0]);
    }
}

use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender as FrameSender, TrySendError, bounded};
use tauri::AppHandle;
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

use super::{
    detector::{StablePresenceDetector, TemplateData, match_template, rgba_to_gray},
    model::NormalizedRect,
};

#[derive(Clone)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapturePurpose {
    Samples,
    Monitor,
    Test,
}

pub type RuntimeCaptureControl = CaptureControl<RuntimeCaptureHandler, String>;

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
    pub region: NormalizedRect,
    pub template: Option<TemplateData>,
    pub reference_width: u32,
    pub reference_height: u32,
    pub threshold: f32,
    pub confirm_frames: u32,
    pub missing_frames: u32,
}

pub struct RuntimeCaptureHandler {
    sender: FrameSender<RuntimeFrame>,
    discard_receiver: Receiver<RuntimeFrame>,
    region: NormalizedRect,
    app: AppHandle,
    purpose: CapturePurpose,
    minimum_frame_interval: Duration,
    last_enqueued_at: Instant,
}

struct RuntimeFrame {
    frame_width: u32,
    frame_height: u32,
    image: CapturedImage,
}

struct RuntimeCaptureProcessor {
    flags: RuntimeCaptureFlags,
    detector: StablePresenceDetector,
    prepared_template: Option<(f32, TemplateData)>,
    last_metric_at: Instant,
}

impl GraphicsCaptureApiHandler for RuntimeCaptureHandler {
    type Flags = RuntimeCaptureFlags;
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (sender, receiver) = bounded(1);
        let discard_receiver = receiver.clone();
        let region = ctx.flags.region;
        let app = ctx.flags.app.clone();
        let purpose = ctx.flags.purpose;
        let minimum_frame_interval = capture_interval(purpose);
        let mut processor = RuntimeCaptureProcessor::new(ctx.flags);
        thread::spawn(move || {
            while let Ok(frame) = receiver.recv() {
                if let Err(error) = processor.process(frame) {
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
            region,
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
        let runtime_frame = RuntimeFrame {
            frame_width: frame.width(),
            frame_height: frame.height(),
            image: copy_frame(frame, Some(self.region))?,
        };
        match self.sender.try_send(runtime_frame) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(latest_frame)) => {
                let _ = self.discard_receiver.try_recv();
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
        let detector = StablePresenceDetector::new(flags.confirm_frames, flags.missing_frames);
        Self {
            flags,
            detector,
            prepared_template: None,
            last_metric_at: Instant::now() - Duration::from_secs(1),
        }
    }

    fn process(&mut self, frame: RuntimeFrame) -> Result<(), String> {
        super::handle_capture_frame(&self.flags.app, self.flags.purpose);
        match self.flags.purpose {
            CapturePurpose::Samples => {
                super::handle_sample_frame(&self.flags.app, frame.image);
            }
            CapturePurpose::Monitor | CapturePurpose::Test => {
                let gray = rgba_to_gray(frame.image.width, frame.image.height, &frame.image.rgba)?;
                let template = self.template_for_frame(frame.frame_width, frame.frame_height)?;
                let confidence = match_template(&gray, template);
                let present = self.detector.update(confidence >= self.flags.threshold);
                let should_emit_metric = self.flags.purpose == CapturePurpose::Test
                    && self.last_metric_at.elapsed() >= Duration::from_millis(200);
                if should_emit_metric {
                    self.last_metric_at = Instant::now();
                }
                super::handle_detection_frame(
                    &self.flags.app,
                    self.flags.purpose,
                    confidence,
                    present,
                    should_emit_metric,
                );
            }
        }
        Ok(())
    }

    fn template_for_frame(
        &mut self,
        frame_width: u32,
        frame_height: u32,
    ) -> Result<&TemplateData, String> {
        let original = self
            .flags
            .template
            .as_ref()
            .ok_or_else(|| "尚未配置 Buff 图标模板".to_string())?;
        let width_scale = frame_width as f32 / self.flags.reference_width.max(1) as f32;
        let height_scale = frame_height as f32 / self.flags.reference_height.max(1) as f32;
        if (width_scale - height_scale).abs() > 0.15 {
            return Err("游戏窗口宽高比变化过大，请重新采集模板".into());
        }
        let scale = (width_scale + height_scale) / 2.0;
        let rebuild = self
            .prepared_template
            .as_ref()
            .is_none_or(|(current, _)| (current - scale).abs() > 0.01);
        if rebuild {
            self.prepared_template = Some((scale, original.scaled(scale)));
        }
        Ok(&self.prepared_template.as_ref().unwrap().1)
    }
}

pub fn capture_snapshot(window: Window) -> Result<CapturedImage, String> {
    let (sender, receiver) = mpsc::channel();
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::Default,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        SnapshotFlags { sender },
    );
    let control = SnapshotHandler::start_free_threaded(settings)
        .map_err(|error| format!("启动窗口捕获失败：{error}"))?;
    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => {
            let _ = control.wait();
            result
        }
        Err(_) => {
            let _ = control.stop();
            Err("等待游戏窗口画面超时，请确认游戏没有最小化并使用无边框窗口".into())
        }
    }
}

pub fn start_runtime_capture(
    window: Window,
    flags: RuntimeCaptureFlags,
) -> Result<RuntimeCaptureControl, String> {
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::Default,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        flags,
    );
    RuntimeCaptureHandler::start_free_threaded(settings)
        .map_err(|error| format!("启动游戏窗口捕获失败：{error}"))
}

const fn capture_interval(purpose: CapturePurpose) -> Duration {
    match purpose {
        CapturePurpose::Samples => Duration::from_millis(333),
        CapturePurpose::Monitor | CapturePurpose::Test => Duration::from_millis(83),
    }
}

fn copy_frame(frame: &mut Frame, region: Option<NormalizedRect>) -> Result<CapturedImage, String> {
    let mut padding_buffer = Vec::new();
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
    let width = buffer.width();
    let height = buffer.height();
    let rgba = buffer.as_nopadding_buffer(&mut padding_buffer).to_vec();
    Ok(CapturedImage {
        width,
        height,
        rgba,
    })
}

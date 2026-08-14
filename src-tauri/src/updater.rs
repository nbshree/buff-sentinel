use std::{
    ffi::OsStr,
    fs, io,
    path::Path,
    sync::{Mutex, MutexGuard},
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Manager, State, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};
use time::format_description::well_known::Rfc3339;

use crate::buff_assistant::BuffAssistant;

const INSTALLER_CLEANUP_RETRY_DELAY: Duration = Duration::from_secs(2);
const INSTALLER_CLEANUP_ATTEMPTS: usize = 10;

pub fn schedule_installer_cleanup(app_name: String) {
    thread::spawn(move || {
        let temp_dir = std::env::temp_dir();
        for attempt in 0..INSTALLER_CLEANUP_ATTEMPTS {
            if attempt > 0 {
                thread::sleep(INSTALLER_CLEANUP_RETRY_DELAY);
            }
            let _ = cleanup_installer_directories(&temp_dir, &app_name);
        }
    });
}

fn cleanup_installer_directories(temp_dir: &Path, app_name: &str) -> io::Result<usize> {
    let mut removed = 0;
    for entry in fs::read_dir(temp_dir)? {
        let Ok(entry) = entry else { continue };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir()
            || file_type.is_symlink()
            || !is_installer_directory_name(&entry.file_name(), app_name)
        {
            continue;
        }
        if fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

fn is_installer_directory_name(name: &OsStr, app_name: &str) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(remainder) = name
        .strip_prefix(app_name)
        .and_then(|remainder| remainder.strip_prefix('-'))
    else {
        return false;
    };
    let Some((version, random_suffix)) = remainder.rsplit_once("-updater-") else {
        return false;
    };
    !version.is_empty()
        && (6..=16).contains(&random_suffix.len())
        && random_suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCheckResult {
    current_version: String,
    update: Option<AppUpdateInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    version: String,
    notes: String,
    published_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppUpdateErrorCode {
    CheckFailed,
    MetadataFailed,
    NoPendingUpdate,
    InstallInProgress,
    MonitorBusy,
    DownloadFailed,
    InstallFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateError {
    code: AppUpdateErrorCode,
    message: String,
}

impl AppUpdateError {
    fn new(code: AppUpdateErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppUpdateDownloadEventKind {
    Started,
    Progress,
    Finished,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateDownloadEvent {
    event: AppUpdateDownloadEventKind,
    downloaded: u64,
    total: Option<u64>,
}

enum PendingState<T> {
    Empty,
    Available(T),
    Installing,
}

pub type PendingUpdate = PendingUpdateState<Update>;

pub struct PendingUpdateState<T> {
    inner: Mutex<PendingState<T>>,
}

impl<T> Default for PendingUpdateState<T> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(PendingState::Empty),
        }
    }
}

impl<T> PendingUpdateState<T> {
    fn lock(&self) -> MutexGuard<'_, PendingState<T>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn replace(&self, update: Option<T>) -> Result<(), AppUpdateError> {
        let mut state = self.lock();
        if matches!(*state, PendingState::Installing) {
            return Err(install_in_progress_error());
        }
        *state = update.map_or(PendingState::Empty, PendingState::Available);
        Ok(())
    }

    fn begin_install(&self) -> Result<T, AppUpdateError> {
        let mut state = self.lock();
        match std::mem::replace(&mut *state, PendingState::Installing) {
            PendingState::Available(update) => Ok(update),
            PendingState::Empty => {
                *state = PendingState::Empty;
                Err(AppUpdateError::new(
                    AppUpdateErrorCode::NoPendingUpdate,
                    "没有可安装的更新，请先检查更新。",
                ))
            }
            PendingState::Installing => {
                *state = PendingState::Installing;
                Err(install_in_progress_error())
            }
        }
    }

    fn restore_after_failure(&self, update: T) {
        let mut state = self.lock();
        if matches!(*state, PendingState::Installing) {
            *state = PendingState::Available(update);
        }
    }

    fn finish_install(&self) {
        let mut state = self.lock();
        if matches!(*state, PendingState::Installing) {
            *state = PendingState::Empty;
        }
    }
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<AppUpdateCheckResult, AppUpdateError> {
    let current_version = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|error| check_error(error.to_string()))?
        .check()
        .await
        .map_err(|error| check_error(error.to_string()))?;
    let info = update.as_ref().map(update_info).transpose()?;
    pending.replace(update)?;
    Ok(AppUpdateCheckResult {
        current_version,
        update: info,
    })
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<(), AppUpdateError> {
    ensure_monitor_idle(&app)?;
    let update = pending.begin_install()?;
    match download_and_install(&app, &update, &on_event).await {
        Ok(()) => {
            pending.finish_install();
            Ok(())
        }
        Err(error) => {
            pending.restore_after_failure(update);
            Err(error)
        }
    }
}

async fn download_and_install(
    app: &AppHandle,
    update: &Update,
    on_event: &Channel<AppUpdateDownloadEvent>,
) -> Result<(), AppUpdateError> {
    let progress = Mutex::new(DownloadProgress::default());
    send_download_event(on_event, AppUpdateDownloadEventKind::Started, 0, None);
    let bytes = update
        .download(
            |chunk_length, content_length| {
                let (downloaded, total) = {
                    let mut progress = progress
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if content_length.is_some() {
                        progress.total = content_length;
                    }
                    progress.downloaded = progress.downloaded.saturating_add(chunk_length as u64);
                    (progress.downloaded, progress.total)
                };
                send_download_event(
                    on_event,
                    AppUpdateDownloadEventKind::Progress,
                    downloaded,
                    total,
                );
            },
            || {},
        )
        .await
        .map_err(|error| {
            AppUpdateError::new(
                AppUpdateErrorCode::DownloadFailed,
                format!("更新下载或签名校验失败：{error}"),
            )
        })?;
    let progress = progress
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    send_download_event(
        on_event,
        AppUpdateDownloadEventKind::Finished,
        progress.downloaded,
        progress.total,
    );
    drop(progress);
    ensure_monitor_idle(app)?;
    update.install(bytes).map_err(|error| {
        AppUpdateError::new(
            AppUpdateErrorCode::InstallFailed,
            format!("启动更新安装程序失败：{error}"),
        )
    })
}

#[derive(Default)]
struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
}

fn update_info(update: &Update) -> Result<AppUpdateInfo, AppUpdateError> {
    let published_at = update
        .date
        .map(|date| date.format(&Rfc3339))
        .transpose()
        .map_err(|error| {
            AppUpdateError::new(
                AppUpdateErrorCode::MetadataFailed,
                format!("更新发布日期格式无效：{error}"),
            )
        })?;
    Ok(AppUpdateInfo {
        version: update.version.clone(),
        notes: update
            .body
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        published_at,
    })
}

fn ensure_monitor_idle(app: &AppHandle) -> Result<(), AppUpdateError> {
    if app.state::<BuffAssistant>().snapshot().is_monitoring {
        Err(AppUpdateError::new(
            AppUpdateErrorCode::MonitorBusy,
            "Buff 监控正在运行，不能安装更新，请先停止监控。",
        ))
    } else {
        Ok(())
    }
}

fn send_download_event(
    channel: &Channel<AppUpdateDownloadEvent>,
    event: AppUpdateDownloadEventKind,
    downloaded: u64,
    total: Option<u64>,
) {
    let _ = channel.send(AppUpdateDownloadEvent {
        event,
        downloaded,
        total,
    });
}

fn check_error(details: String) -> AppUpdateError {
    AppUpdateError::new(
        AppUpdateErrorCode::CheckFailed,
        format!("无法获取更新信息：{details}"),
    )
}

fn install_in_progress_error() -> AppUpdateError {
    AppUpdateError::new(
        AppUpdateErrorCode::InstallInProgress,
        "更新已在下载或安装中，请勿重复操作。",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_matches_owned_tauri_updater_directories() {
        assert!(is_installer_directory_name(
            OsStr::new("BuffFlow-0.2.0-updater-a1B2c3"),
            "BuffFlow"
        ));
        assert!(!is_installer_directory_name(
            OsStr::new("其他应用-0.2.0-updater-a1B2c3"),
            "BuffFlow"
        ));
        assert!(!is_installer_directory_name(
            OsStr::new("BuffFlow-0.2.0-updater-../../bad"),
            "BuffFlow"
        ));
    }
}

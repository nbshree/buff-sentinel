use windows_capture::window::Window;
use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;

use super::model::{BuffTarget, CaptureWindowCandidate};

pub fn enumerate_candidates() -> Result<Vec<CaptureWindowCandidate>, String> {
    let mut candidates = Window::enumerate()
        .map_err(|error| format!("枚举窗口失败：{error}"))?
        .into_iter()
        .filter_map(|window| candidate_from_window(window).ok())
        .filter(|candidate| {
            !candidate.window_title.trim().is_empty()
                && candidate.width >= 320
                && candidate.height >= 240
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.process_name
            .to_lowercase()
            .cmp(&right.process_name.to_lowercase())
            .then_with(|| left.window_title.cmp(&right.window_title))
    });
    Ok(candidates)
}

pub fn resolve_window(id: &str) -> Result<(Window, CaptureWindowCandidate), String> {
    let requested = id
        .parse::<usize>()
        .map_err(|_| "游戏窗口标识无效，请刷新窗口列表".to_string())?;
    for window in Window::enumerate().map_err(|error| format!("枚举窗口失败：{error}"))? {
        if window.as_raw_hwnd() as usize == requested {
            let candidate = candidate_from_window(window)?;
            return Ok((window, candidate));
        }
    }
    Err("游戏窗口已经关闭，请刷新窗口列表".into())
}

pub fn find_target(target: &BuffTarget) -> Result<Option<Window>, String> {
    let windows = Window::enumerate().map_err(|error| format!("枚举窗口失败：{error}"))?;
    let mut fallback = None;
    for window in windows {
        let Ok(candidate) = candidate_from_window(window) else {
            continue;
        };
        if !candidate
            .process_name
            .eq_ignore_ascii_case(&target.process_name)
        {
            continue;
        }
        let class_matches = target.class_name.is_empty()
            || candidate
                .class_name
                .eq_ignore_ascii_case(&target.class_name);
        let title_matches = target.window_title.is_empty()
            || candidate
                .window_title
                .eq_ignore_ascii_case(&target.window_title);
        if class_matches && title_matches {
            return Ok(Some(window));
        }
        if fallback.is_none() && class_matches {
            fallback = Some(window);
        }
    }
    Ok(fallback)
}

pub fn target_from_candidate(candidate: &CaptureWindowCandidate) -> BuffTarget {
    BuffTarget {
        process_name: candidate.process_name.clone(),
        window_title: candidate.window_title.clone(),
        class_name: candidate.class_name.clone(),
        reference_width: candidate.width,
        reference_height: candidate.height,
    }
}

fn candidate_from_window(window: Window) -> Result<CaptureWindowCandidate, String> {
    let width = window
        .width()
        .map_err(|error| format!("读取窗口宽度失败：{error}"))?;
    let height = window
        .height()
        .map_err(|error| format!("读取窗口高度失败：{error}"))?;
    Ok(CaptureWindowCandidate {
        id: (window.as_raw_hwnd() as usize).to_string(),
        process_name: window.process_name().unwrap_or_default(),
        window_title: window.title().unwrap_or_default(),
        class_name: class_name(window),
        width: width.max(0) as u32,
        height: height.max(0) as u32,
    })
}

fn class_name(window: Window) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe {
        GetClassNameW(
            window.as_raw_hwnd().cast(),
            buffer.as_mut_ptr(),
            buffer.len() as i32,
        )
    };
    if length <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

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
    Ok(resolve_target(target)?.map(|(window, _)| window))
}

pub fn resolve_target(
    target: &BuffTarget,
) -> Result<Option<(Window, CaptureWindowCandidate)>, String> {
    let windows = Window::enumerate().map_err(|error| format!("枚举窗口失败：{error}"))?;
    let mut best_match = None;
    for window in windows {
        let Ok(candidate) = candidate_from_window(window) else {
            continue;
        };
        let Some(rank) = target_match_rank(&candidate, target) else {
            continue;
        };
        if best_match
            .as_ref()
            .is_none_or(|(best_rank, _, _)| rank < *best_rank)
        {
            best_match = Some((rank, window, candidate));
        }
    }
    Ok(best_match.map(|(_, window, candidate)| (window, candidate)))
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

fn target_match_rank(candidate: &CaptureWindowCandidate, target: &BuffTarget) -> Option<u8> {
    let process_matches = field_matches(&candidate.process_name, &target.process_name);
    let class_matches = field_matches(&candidate.class_name, &target.class_name);
    let title_matches = field_matches(&candidate.window_title, &target.window_title);
    match (process_matches, class_matches, title_matches) {
        (true, true, true) => Some(0),
        (true, true, false) => Some(1),
        (true, false, true) => Some(2),
        (false, true, true) => Some(3),
        _ => None,
    }
}

fn field_matches(actual: &str, expected: &str) -> bool {
    !expected.is_empty() && actual.eq_ignore_ascii_case(expected)
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

#[cfg(test)]
mod tests {
    use super::{CaptureWindowCandidate, target_match_rank};
    use crate::buff_assistant::model::BuffTarget;

    fn candidate() -> CaptureWindowCandidate {
        CaptureWindowCandidate {
            id: "42".into(),
            process_name: "game.exe".into(),
            window_title: "Game".into(),
            class_name: "GameWindow".into(),
            width: 1920,
            height: 1080,
        }
    }

    fn target() -> BuffTarget {
        BuffTarget {
            process_name: "game.exe".into(),
            window_title: "Game".into(),
            class_name: "GameWindow".into(),
            reference_width: 1920,
            reference_height: 1080,
        }
    }

    #[test]
    fn exact_identity_has_highest_priority() {
        assert_eq!(target_match_rank(&candidate(), &target()), Some(0));
    }

    #[test]
    fn changed_title_can_match_by_process_and_class() {
        let mut target = target();
        target.window_title = "Previous title".into();
        assert_eq!(target_match_rank(&candidate(), &target), Some(1));
    }

    #[test]
    fn changed_process_can_match_by_class_and_title() {
        let mut target = target();
        target.process_name = "old-game.exe".into();
        assert_eq!(target_match_rank(&candidate(), &target), Some(3));
    }

    #[test]
    fn changed_class_can_match_by_process_and_title() {
        let mut target = target();
        target.class_name = "OldWindowClass".into();
        assert_eq!(target_match_rank(&candidate(), &target), Some(2));
    }

    #[test]
    fn empty_target_fields_are_not_wildcards() {
        let target = BuffTarget {
            process_name: String::new(),
            window_title: String::new(),
            class_name: String::new(),
            reference_width: 1920,
            reference_height: 1080,
        };
        assert_eq!(target_match_rank(&candidate(), &target), None);
    }
}

//! Global shortcuts are registered on one dedicated Windows message thread.
//! Reconfiguration reserves the new binding before releasing the previous one.
use player_application::{AppError, AppResult};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use tauri::{AppHandle, Emitter, Manager};

pub(super) struct GlobalSearch {
    send: mpsc::Sender<Request>,
}
struct Request {
    enabled: bool,
    binding: String,
    result: mpsc::SyncSender<AppResult<()>>,
}

impl GlobalSearch {
    pub fn spawn(app: AppHandle, running: Arc<AtomicBool>) -> Self {
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || run(app, running, receive));
        Self { send }
    }
    pub fn configure(&self, enabled: bool, binding: String) -> AppResult<()> {
        let (send, receive) = mpsc::sync_channel(1);
        self.send
            .send(Request {
                enabled,
                binding,
                result: send,
            })
            .map_err(|e| AppError::new("global_shortcut_error", e))?;
        receive
            .recv_timeout(std::time::Duration::from_secs(3))
            .map_err(|e| AppError::new("global_shortcut_error", e))?
    }
}

fn parse_binding(binding: &str) -> AppResult<(u32, u32)> {
    let mut parts = binding.split('+').collect::<Vec<_>>();
    let key = parts.pop().unwrap_or_default();
    let mut modifiers = 0_u32;
    for part in parts {
        let bit = match part {
            "Alt" => 1,
            "Ctrl" => 2,
            "Shift" => 4,
            _ => 0,
        };
        if bit == 0 || modifiers & bit != 0 {
            return Err(AppError::new(
                "invalid_input",
                "Unsupported global shortcut modifier",
            ));
        }
        modifiers |= bit;
    }
    if modifiers & 3 == 0 {
        return Err(AppError::new(
            "invalid_input",
            "Global shortcuts require Ctrl or Alt",
        ));
    }
    let vk = match key {
        "Space" => 0x20,
        "Enter" => 0x0D,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "ArrowLeft" => 0x25,
        "ArrowUp" => 0x26,
        "ArrowRight" => 0x27,
        "ArrowDown" => 0x28,
        _ if key.len() == 4 && key.starts_with("Key") && key.as_bytes()[3].is_ascii_uppercase() => {
            key.as_bytes()[3] as u32
        }
        _ if key.len() == 6 && key.starts_with("Digit") && key.as_bytes()[5].is_ascii_digit() => {
            key.as_bytes()[5] as u32
        }
        _ if key.starts_with('F') => key[1..]
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=24).contains(n))
            .map(|n| 0x70 + n - 1)
            .unwrap_or(0),
        _ => 0,
    };
    if vk == 0
        || (modifiers == 2 && vk == b'F' as u32)
        || (modifiers == 1 && (vk == 0x73 || vk == 0x20))
    {
        return Err(AppError::new(
            "invalid_input",
            "Choose a dedicated global shortcut, for example Ctrl+Alt+Space",
        ));
    }
    Ok((modifiers | 0x4000, vk)) // MOD_NOREPEAT
}

#[cfg(not(windows))]
fn run(_app: AppHandle, running: Arc<AtomicBool>, receive: mpsc::Receiver<Request>) {
    while running.load(Ordering::Relaxed) {
        if let Ok(request) = receive.recv_timeout(std::time::Duration::from_millis(100)) {
            let result = if request.enabled {
                Err(AppError::new(
                    "unsupported_platform",
                    "Global search shortcut is currently available on Windows",
                ))
            } else {
                Ok(())
            };
            let _ = request.result.send(result);
        }
    }
}

#[cfg(windows)]
fn run(app: AppHandle, running: Arc<AtomicBool>, receive: mpsc::Receiver<Request>) {
    use std::{ffi::c_void, ptr};
    #[repr(C)]
    #[derive(Default)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Message {
        hwnd: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        point: Point,
        private: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn RegisterHotKey(hwnd: *mut c_void, id: i32, modifiers: u32, key: u32) -> i32;
        fn UnregisterHotKey(hwnd: *mut c_void, id: i32) -> i32;
        fn PeekMessageW(
            message: *mut Message,
            hwnd: *mut c_void,
            min: u32,
            max: u32,
            remove: u32,
        ) -> i32;
    }
    let mut active: Option<(i32, (u32, u32))> = None;
    let mut next_id = 0x4150;
    while running.load(Ordering::Relaxed) {
        if let Ok(request) = receive.recv_timeout(std::time::Duration::from_millis(40)) {
            let result = if !request.enabled {
                if let Some((id, _)) = active.take() {
                    unsafe {
                        UnregisterHotKey(ptr::null_mut(), id);
                    }
                }
                Ok(())
            } else {
                parse_binding(&request.binding).and_then(|keys| {
                    if active.is_some_and(|(_, old)| old == keys) {
                        return Ok(());
                    }
                    next_id = if next_id == 0x4150 { 0x4151 } else { 0x4150 };
                    // SAFETY: Thread-owned registration, no HWND, validated values.
                    if unsafe { RegisterHotKey(ptr::null_mut(), next_id, keys.0, keys.1) } == 0 {
                        return Err(AppError::new(
                            "global_shortcut_conflict",
                            format!(
                                "The shortcut is unavailable or already used ({})",
                                std::io::Error::last_os_error()
                            ),
                        ));
                    }
                    if let Some((id, _)) = active.replace((next_id, keys)) {
                        unsafe {
                            UnregisterHotKey(ptr::null_mut(), id);
                        }
                    }
                    Ok(())
                })
            };
            let _ = request.result.send(result);
        }
        let mut message = Message::default();
        // SAFETY: Correct MSG layout, writable buffer on owning thread; only
        // WM_HOTKEY messages for this thread are removed from its message queue.
        while unsafe { PeekMessageW(&mut message, ptr::null_mut(), 0x0312, 0x0312, 1) } != 0 {
            if active.is_some_and(|(id, _)| message.wparam == id as usize) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
                let _ = app.emit("player://quick-search", ());
            }
        }
    }
    if let Some((id, _)) = active {
        unsafe {
            UnregisterHotKey(ptr::null_mut(), id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_require_dedicated_modifier_and_reject_plain_find() {
        assert_eq!(parse_binding("Ctrl+Alt+Space").unwrap(), (0x4003, 0x20));
        assert_eq!(parse_binding("Ctrl+Shift+KeyK").unwrap(), (0x4006, 75));
        assert!(parse_binding("Ctrl+KeyF").is_err());
        assert!(parse_binding("Space").is_err());
        assert!(parse_binding("Ctrl+Ctrl+Space").is_err());
    }
}

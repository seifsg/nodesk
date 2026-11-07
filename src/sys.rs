//! Thin Win32 helpers shared by the app: strings, clocks, idle time, boxes, process setup. 🧰
use std::{mem::size_of, ptr::null};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::{
        SystemInformation::{GetLocalTime, GetTickCount},
        Threading::CreateMutexW,
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        Shell::{
            QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
            SHQueryUserNotificationState,
        },
        WindowsAndMessaging::{
            MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MessageBoxW,
        },
    },
};

/// UTF-16 + NUL for Win32 `W` APIs.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Milliseconds since boot. Wraps every 49.7 days: always diff with `wrapping_sub`.
pub fn now_ms() -> u32 {
    unsafe { GetTickCount() }
}

/// Milliseconds since the last keyboard or mouse input, system-wide.
pub fn idle_ms() -> u32 {
    let mut li = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    assert!(
        unsafe { GetLastInputInfo(&mut li) } != 0,
        "GetLastInputInfo failed"
    );
    now_ms().wrapping_sub(li.dwTime)
}

/// Local date as yyyymmdd: the stats day key.
pub fn today() -> u32 {
    let mut t = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut t) };
    t.wYear as u32 * 10_000 + t.wMonth as u32 * 100 + t.wDay as u32
}

/// True while a fullscreen app, game or presentation runs: breaks wait for it.
pub fn fullscreen_busy() -> bool {
    let mut s = 0;
    (unsafe { SHQueryUserNotificationState(&mut s) }) == 0
        && matches!(
            s,
            QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
        )
}

/// Modal error box (never while app state is borrowed).
pub fn error_box(msg: &str) {
    message(msg, MB_ICONERROR);
}

/// Modal info box (never while app state is borrowed).
pub fn info_box(msg: &str) {
    message(msg, MB_ICONINFORMATION);
}

/// Modal box. Callers never hold app state across it (its loop dispatches our timers).
fn message(msg: &str, icon: u32) {
    let (text, title) = (wide(msg), wide("NoDesk"));
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_SETFOREGROUND | icon,
        )
    };
}

/// Takes a session-wide named mutex, kept for the process lifetime; false if NoDesk already runs.
pub fn first_instance() -> bool {
    let name = wide("Local\\NoDesk.SingleInstance");
    unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    (unsafe { GetLastError() }) != ERROR_ALREADY_EXISTS
}

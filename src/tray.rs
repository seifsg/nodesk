//! Tray icon: add, update tooltip and icon, warning balloon, popup menu. 🔔
use crate::sys::wide;
use std::{iter::repeat, mem::size_of, ptr::null};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Shell::{
            NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_WARNING, NIM_ADD,
            NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
            Shell_NotifyIconW,
        },
        WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, DestroyMenu, HICON, MF_CHECKED, MF_SEPARATOR, MF_STRING,
            PostMessageW, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
            TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_NULL,
        },
    },
};

/// Callback message the icon sends to the host window.
pub const MSG: u32 = WM_APP + 1;

/// Our icon's identity (window + id 1); each call fills the fields it changes.
fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut d: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    d.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    d.hWnd = hwnd;
    d.uID = 1;
    d
}

/// Copies `s` into a fixed UTF-16 field, truncated, always NUL-terminated.
fn put(dst: &mut [u16], s: &str) {
    let n = dst.len() - 1;
    dst.iter_mut()
        .zip(s.encode_utf16().take(n).chain(repeat(0)))
        .for_each(|(d, c)| *d = c);
}

/// Shows the icon with version-4 callbacks; also used to re-add it after Explorer restarts.
pub fn add(hwnd: HWND, icon: HICON, tip: &str) {
    let mut d = data(hwnd);
    d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    d.uCallbackMessage = MSG;
    d.hIcon = icon;
    put(&mut d.szTip, tip);
    d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &d);
        Shell_NotifyIconW(NIM_SETVERSION, &d);
    }
}

/// New icon and tooltip text.
pub fn update(hwnd: HWND, icon: HICON, tip: &str) {
    let mut d = data(hwnd);
    d.uFlags = NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    d.hIcon = icon;
    put(&mut d.szTip, tip);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
}

/// Non-modal warning balloon: safe to call while app state is borrowed.
pub fn warn(hwnd: HWND, text: &str) {
    let mut d = data(hwnd);
    d.uFlags = NIF_INFO;
    d.dwInfoFlags = NIIF_WARNING;
    put(&mut d.szInfoTitle, "NoDesk");
    put(&mut d.szInfo, text);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
}

/// Takes the icon out of the tray on quit.
pub fn remove(hwnd: HWND) {
    unsafe { Shell_NotifyIconW(NIM_DELETE, &data(hwnd)) };
}

/// One popup menu row.
pub enum Item {
    Info(String),
    Check(u32, &'static str, bool),
    Cmd(u32, &'static str),
    Sep,
}

/// Shows the menu at (x, y); returns the picked command id, 0 when dismissed.
/// Modal: callers never hold app state across it.
pub fn menu(hwnd: HWND, x: i32, y: i32, items: &[Item]) -> u32 {
    unsafe {
        let m = CreatePopupMenu();
        for it in items {
            let (flags, id, text) = match it {
                // Themed menus grey out any disabled row, so stats stay plain rows at full
                // contrast; their id 0 is the same answer as dismissing the menu.
                Item::Info(t) => (MF_STRING, 0, t.as_str()),
                Item::Check(id, t, on) => (MF_STRING | if *on { MF_CHECKED } else { 0 }, *id, *t),
                Item::Cmd(id, t) => (MF_STRING, *id, *t),
                Item::Sep => (MF_SEPARATOR, 0, ""),
            };
            AppendMenuW(m, flags, id as usize, wide(text).as_ptr());
        }
        SetForegroundWindow(hwnd);
        let flags = TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN;
        let cmd = TrackPopupMenu(m, flags, x, y, 0, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(m);
        cmd as u32
    }
}

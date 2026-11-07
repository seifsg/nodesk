//! Hidden host window: owns the tray icon, the 1 s tick and the message loop. 🏠
use crate::{
    app::{self, App},
    overlay,
    sys::{error_box, wide},
    tray,
};
use std::{
    ptr::{null, null_mut},
    sync::OnceLock,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::GetSystemMetricsForDpi,
        Shell::{NIN_SELECT, NINF_KEY},
        WindowsAndMessaging::*,
    },
};

/// Shell broadcast after Explorer (re)starts: the tray icon must be added again.
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

/// Creates the host window and app state, then pumps messages until Quit.
pub fn run() {
    let class = wide("NoDeskHost");
    let host = unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: GetModuleHandleW(null()),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            wc.hInstance,
            null(),
        )
    };
    assert!(!host.is_null(), "could not create the host window");
    TASKBAR_CREATED
        .get_or_init(|| unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) });
    app::init(App::new(
        host,
        overlay::create(),
        [icon(1), icon(2)],
        speed(),
    ));
    app::with(App::add_tray);
    unsafe {
        SetTimer(host, 1, 1000, None);
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Small icon from the exe's resources, sized for the system DPI.
fn icon(id: u16) -> HICON {
    unsafe {
        let px = GetSystemMetricsForDpi(
            SM_CXSMICON,
            windows_sys::Win32::UI::HiDpi::GetDpiForSystem(),
        );
        let h = LoadImageW(
            GetModuleHandleW(null()),
            id as usize as *const u16,
            IMAGE_ICON,
            px,
            px,
            LR_DEFAULTCOLOR,
        );
        assert!(!h.is_null(), "icon resource {id} missing");
        h
    }
}

/// `NODESK_SPEED=60` makes active time run 60x for demos; anything invalid is refused loudly.
fn speed() -> u32 {
    match std::env::var("NODESK_SPEED") {
        Err(_) => 1,
        Ok(v) => v
            .parse()
            .ok()
            .filter(|s| (1..=3600).contains(s))
            .unwrap_or_else(|| {
                error_box(&format!(
                    "NODESK_SPEED must be 1 to 3600, got {v:?}. Using real time."
                ));
                1
            }),
    }
}

const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const CMD_QUIT: u32 = 1;

/// Tray menu: built from app state, shown with no state borrowed, then the pick applied.
fn menu(h: HWND, x: i32, y: i32) {
    let items = [tray::Item::Cmd(CMD_QUIT, "Quit NoDesk")];
    if tray::menu(h, x, y, &items) == CMD_QUIT {
        unsafe { DestroyWindow(h) };
    }
}

unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_TIMER => app::with(App::tick),
        tray::MSG => {
            if let WM_CONTEXTMENU | NIN_SELECT | NIN_KEYSELECT = l as u32 & 0xFFFF {
                // Version-4 icons send the anchor point in wParam.
                menu(h, w as u16 as i16 as i32, (w >> 16) as u16 as i16 as i32);
            }
        }
        WM_DESTROY => unsafe {
            tray::remove(h);
            PostQuitMessage(0);
        },
        _ if Some(&m) == TASKBAR_CREATED.get() => app::with(App::add_tray),
        _ => return unsafe { DefWindowProcW(h, m, w, l) },
    }
    0
}

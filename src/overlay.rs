//! Break screen window: one topmost layered popup over every monitor; fade-in, keys, clicks. 🌙
use crate::{
    app,
    paint::{self, BG, Layout, layout},
    sys::{now_ms, wide},
    timer::{Break, Outcome},
};
use std::{
    cell::Cell,
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::*,
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::{
            HiDpi::*,
            Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE},
            WindowsAndMessaging::*,
        },
    },
    core::BOOL,
};

/// Animation timer: fade-in, then the smoothly shrinking bar, while the screen shows.
const FRAME: usize = 2;
const FRAME_MS: u32 = 30;
const FADE_MS: u32 = 250;
const ALPHA: u8 = 245;
/// Keys and clicks this soon after a break appears were meant for the app you were typing in.
const GRACE_MS: u32 = 1_500;

thread_local! {
    /// What the screen shows and when it was last set; paint reads it, never app state.
    static VIEW: Cell<Option<(Break, u32)>> = const { Cell::new(None) };
    static SHOWN_AT: Cell<u32> = const { Cell::new(0) };
    /// Cursor position while it is over a button (hover look, hand cursor).
    static HOT: Cell<Option<POINT>> = const { Cell::new(None) };
}

/// Creates the hidden break window once; shown and hidden per break.
pub fn create() -> HWND {
    let class = wide("NoDeskBreak");
    let h = unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: GetModuleHandleW(null()),
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let ex = WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW;
        CreateWindowExW(
            ex,
            class.as_ptr(),
            wide("NoDesk break").as_ptr(),
            WS_POPUP,
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
    assert!(!h.is_null(), "could not create the break window");
    h
}

/// Covers all monitors, fades in, takes focus for Esc / P.
pub fn show(h: HWND, b: Break) {
    VIEW.set(Some((b, now_ms())));
    SHOWN_AT.set(now_ms());
    HOT.set(None);
    unsafe {
        SetLayeredWindowAttributes(h, 0, 0, LWA_ALPHA);
        cover(h);
        ShowWindow(h, SW_SHOW);
        SetTimer(h, FRAME, FRAME_MS, None);
    }
    focus(h);
}

/// Takes keyboard focus from the app in front, so typing pauses and Esc / P reach us.
/// Windows refuses SetForegroundWindow to background apps unless input queues are joined.
fn focus(h: HWND) {
    unsafe {
        let (me, front) = (
            GetCurrentThreadId(),
            GetWindowThreadProcessId(GetForegroundWindow(), null_mut()),
        );
        let joined = front != me && AttachThreadInput(me, front, 1) != 0;
        SetForegroundWindow(h);
        SetFocus(h);
        if joined {
            AttachThreadInput(me, front, 0);
        }
    }
}

/// New countdown second: repaints each monitor's content block.
pub fn update(h: HWND, b: Break) {
    VIEW.set(Some((b, now_ms())));
    invalidate(h, |l| l.block);
}

/// Hides the screen and stops its animation timer.
pub fn hide(h: HWND) {
    VIEW.set(None);
    unsafe {
        KillTimer(h, FRAME);
        ShowWindow(h, SW_HIDE);
    }
}

/// Marks one part of every monitor's layout for repaint.
fn invalidate(h: HWND, part: impl Fn(&Layout) -> RECT) {
    for (m, dpi) in monitors() {
        unsafe { InvalidateRect(h, &part(&layout(&m, dpi)), 0) };
    }
}

/// Break on screen plus its countdown interpolated since the last tick (for a smooth bar).
fn view() -> Option<(Break, u32)> {
    VIEW.get()
        .map(|(b, at)| (b, b.left.saturating_sub(now_ms().wrapping_sub(at))))
}

/// Sizes the window to the whole virtual screen (all monitors), topmost.
fn cover(h: HWND) {
    unsafe {
        let [x, y, w, ht] = [
            SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
            SM_CXVIRTUALSCREEN,
            SM_CYVIRTUALSCREEN,
        ]
        .map(|i| GetSystemMetrics(i));
        SetWindowPos(h, HWND_TOPMOST, x, y, w, ht, SWP_NOACTIVATE);
        InvalidateRect(h, null(), 0);
    }
}

/// Every monitor as (rect in this window's client coords, effective DPI).
fn monitors() -> Vec<(RECT, u32)> {
    unsafe extern "system" fn add(m: HMONITOR, _: HDC, r: *mut RECT, out: LPARAM) -> BOOL {
        let (mut dx, mut dy) = (96, 96);
        unsafe { GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
        unsafe { (*(out as *mut Vec<(RECT, u32)>)).push((*r, dx)) };
        1
    }
    let mut v: Vec<(RECT, u32)> = Vec::new();
    unsafe {
        EnumDisplayMonitors(null_mut(), null(), Some(add), &mut v as *mut _ as LPARAM);
        let (ox, oy) = (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
        );
        v.iter_mut().for_each(|(r, _)| {
            *r = RECT {
                left: r.left - ox,
                top: r.top - oy,
                right: r.right - ox,
                bottom: r.bottom - oy,
            }
        });
    }
    v
}

/// Content blocks first, then the dim background around them.
fn paint(h: HWND) {
    unsafe {
        let mut ps = std::mem::zeroed();
        let dc = BeginPaint(h, &mut ps);
        if let Some((b, left)) = view() {
            for (m, dpi) in monitors() {
                paint::draw(dc, &m, dpi, &b, left, HOT.get());
                let k = layout(&m, dpi).block;
                ExcludeClipRect(dc, k.left, k.top, k.right, k.bottom);
            }
        }
        let br = CreateSolidBrush(BG);
        FillRect(dc, &ps.rcPaint, br);
        DeleteObject(br);
        EndPaint(h, &ps);
    }
}

/// Which button sits under a client point, if any.
fn hit(p: POINT) -> Option<Outcome> {
    let inside = |r: &RECT| (r.left..r.right).contains(&p.x) && (r.top..r.bottom).contains(&p.y);
    monitors()
        .iter()
        .map(|(m, dpi)| layout(m, *dpi))
        .find_map(|l| {
            if inside(&l.skip) {
                Some(Outcome::Skipped)
            } else if inside(&l.later) {
                Some(Outcome::Postponed)
            } else {
                None
            }
        })
}

/// One animation frame: ease-out fade for the first 250 ms, then the bar moves on.
fn frame(h: HWND) {
    let t = now_ms().wrapping_sub(SHOWN_AT.get());
    if t <= FADE_MS + FRAME_MS {
        let p = 1.0 - (1.0 - (t.min(FADE_MS) as f32 / FADE_MS as f32)).powi(3);
        unsafe { SetLayeredWindowAttributes(h, 0, (ALPHA as f32 * p) as u8, LWA_ALPHA) };
    }
    let Some((b, left)) = view() else { return };
    unsafe {
        let dc = GetDC(h);
        for (m, dpi) in monitors() {
            paint::draw_bar(dc, &layout(&m, dpi), &b, left);
        }
        ReleaseDC(h, dc);
    }
}

/// Mouse moved: track which button it is over, repaint buttons when that changes.
fn hover(h: HWND, p: POINT) {
    let now = hit(p).map(|_| p);
    let changed = now.is_some() != HOT.get().is_some();
    HOT.set(now);
    if changed {
        invalidate(h, |l| l.skip);
        invalidate(h, |l| l.later);
    }
}

/// Client point packed in a mouse message's lParam.
fn point(l: LPARAM) -> POINT {
    POINT {
        x: l as i16 as i32,
        y: (l >> 16) as i16 as i32,
    }
}

/// Break window messages: paint, animation, hover, keys and clicks.
unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let pick = match m {
        WM_PAINT => {
            paint(h);
            None
        }
        WM_ERASEBKGND => return 1,
        WM_TIMER if w == FRAME => {
            frame(h);
            None
        }
        WM_MOUSEMOVE => {
            hover(h, point(l));
            None
        }
        WM_SETCURSOR if HOT.get().is_some() => {
            unsafe { SetCursor(LoadCursorW(null_mut(), IDC_HAND)) };
            return 1;
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            cover(h);
            None
        }
        WM_KEYDOWN | WM_LBUTTONUP | WM_CLOSE
            if now_ms().wrapping_sub(SHOWN_AT.get()) < GRACE_MS =>
        {
            None
        }
        WM_KEYDOWN if l >> 30 & 1 == 1 => None, // auto-repeat of a key held before the break
        WM_KEYDOWN if w == VK_ESCAPE as usize => Some(Outcome::Skipped),
        WM_KEYDOWN if w == b'P' as usize => Some(Outcome::Postponed),
        WM_CLOSE => Some(Outcome::Skipped),
        WM_LBUTTONUP => hit(point(l)),
        _ => return unsafe { DefWindowProcW(h, m, w, l) },
    };
    if let Some(o) = pick {
        app::with(|a| a.finish(o));
    }
    0
}

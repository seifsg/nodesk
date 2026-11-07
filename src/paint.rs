//! Break screen drawing ("Dusk" look): per-monitor layout and GDI paint. 🎨
use crate::{
    kinds::{Kind, in_mask, rgb},
    sys::wide,
    text::{clock, dur},
    timer::{Break, SNOOZE_MS},
};
use std::{ptr::null_mut, sync::OnceLock};
use windows_sys::Win32::{Foundation::RECT, Graphics::Gdi::*};

pub const BG: u32 = rgb(0x0E1116);
const TEXT: u32 = rgb(0xF5F7FA);
const DIM: u32 = rgb(0x9AA4B2);
const TRACK: u32 = rgb(0x232A33);
const BUTTON: u32 = rgb(0x1B212A);

/// Where each element sits on one monitor (client coords).
pub struct Layout {
    pub block: RECT,
    label: RECT,
    count: RECT,
    tips: RECT,
    bar: RECT,
    pub skip: RECT,
    pub later: RECT,
}

/// Centered column, sized by the monitor's DPI and clamped to its width.
pub fn layout(m: &RECT, dpi: u32) -> Layout {
    let s = |v: i32| v * dpi as i32 / 96;
    let (cx, cy) = ((m.left + m.right) / 2, (m.top + m.bottom) / 2);
    let half = s(440).min((m.right - m.left) / 2 - s(24));
    let top = cy - s(270);
    let row = |y, h, half| RECT {
        left: cx - half,
        top: top + s(y),
        right: cx + half,
        bottom: top + s(y + h),
    };
    let button = |x| RECT {
        left: cx + s(x),
        top: top + s(452),
        right: cx + s(x + 200),
        bottom: top + s(500),
    };
    Layout {
        block: row(0, 520, half),
        label: row(0, 28, half),
        count: row(36, 176, half),
        tips: row(222, 150, half),
        bar: row(400, 4, s(180).min(half)),
        skip: button(-208),
        later: button(8),
    }
}

/// Font faces: Windows 11 Segoe UI Variable named instances, else classic Segoe UI.
#[derive(Clone, Copy)]
enum Face {
    Light,
    Label,
    Text,
    Strong,
}

fn face(f: Face) -> &'static str {
    static VARIABLE: OnceLock<bool> = OnceLock::new();
    let v = *VARIABLE.get_or_init(|| has_face("Segoe UI Variable Text"));
    let (new, old) = match f {
        Face::Light => ("Segoe UI Variable Display Light", "Segoe UI Light"),
        Face::Label => ("Segoe UI Variable Display Semib", "Segoe UI Semibold"),
        Face::Text => ("Segoe UI Variable Text", "Segoe UI"),
        Face::Strong => ("Segoe UI Variable Text Semibold", "Segoe UI Semibold"),
    };
    if v { new } else { old }
}

fn font(f: &str, px: i32) -> HFONT {
    let name = wide(f);
    let q = CLEARTYPE_QUALITY as u32;
    unsafe {
        CreateFontW(
            -px,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            q,
            0,
            name.as_ptr(),
        )
    }
}

/// GDI silently swaps a missing face for another: ask which face it really picked.
fn has_face(f: &str) -> bool {
    unsafe {
        let dc = GetDC(null_mut());
        let font = font(f, 12);
        let old = SelectObject(dc, font);
        let mut got = [0u16; 32];
        let n = GetTextFaceW(dc, 32, got.as_mut_ptr());
        SelectObject(dc, old);
        DeleteObject(font);
        ReleaseDC(null_mut(), dc);
        n > 0 && got[..n as usize - 1] == *f.encode_utf16().collect::<Vec<_>>()
    }
}

fn fill(dc: HDC, r: &RECT, color: u32) {
    unsafe {
        let b = CreateSolidBrush(color);
        FillRect(dc, r, b);
        DeleteObject(b);
    }
}

fn text(dc: HDC, r: &RECT, s: &str, f: Face, px: i32, color: u32, flags: DRAW_TEXT_FORMAT) {
    unsafe {
        let font = font(face(f), px);
        let old = SelectObject(dc, font);
        SetTextColor(dc, color);
        let mut r = *r;
        DrawTextW(dc, wide(s).as_ptr(), -1, &mut r, flags | DT_NOPREFIX);
        SelectObject(dc, old);
        DeleteObject(font);
    }
}

/// Flat rounded button: label centered, key hint dimmed on the right.
fn button(dc: HDC, r: &RECT, label: &str, key: &str, s: impl Fn(i32) -> i32) {
    unsafe {
        let b = CreateSolidBrush(BUTTON);
        let (ob, op) = (
            SelectObject(dc, b),
            SelectObject(dc, GetStockObject(NULL_PEN)),
        );
        RoundRect(dc, r.left, r.top, r.right + 1, r.bottom + 1, s(12), s(12));
        SelectObject(dc, ob);
        SelectObject(dc, op);
        DeleteObject(b);
    }
    let center = DT_CENTER | DT_VCENTER | DT_SINGLELINE;
    text(dc, r, label, Face::Strong, s(16), TEXT, center);
    let hint = RECT {
        right: r.right - s(14),
        ..*r
    };
    text(
        dc,
        &hint,
        key,
        Face::Text,
        s(12),
        DIM,
        DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
    );
}

/// Paints one monitor's content block off-screen, then blits it: no flicker, small memory.
pub fn draw(hdc: HDC, m: &RECT, dpi: u32, b: &Break) {
    let l = layout(m, dpi);
    let s = |v: i32| v * dpi as i32 / 96;
    let (w, h) = (l.block.right - l.block.left, l.block.bottom - l.block.top);
    // Longest kind leads (label, accent, big tip); ties go to the later row (eye over micro).
    let mut order: Vec<&Kind> = in_mask(b.mask).map(|(_, k)| k).collect();
    if let Some(i) = (0..order.len()).max_by_key(|&i| (order[i].length_ms, i)) {
        let lead = order.remove(i);
        order.insert(0, lead);
    }
    let primary = order[0]; // a break always holds at least one kind
    let tip = |k: &Kind| k.tips[b.rot as usize % k.tips.len()];
    unsafe {
        let dc = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        let old = SelectObject(dc, bmp);
        SetViewportOrgEx(dc, -l.block.left, -l.block.top, null_mut());
        SetBkMode(dc, TRANSPARENT as i32);
        fill(dc, &l.block, BG);

        let names: Vec<_> = order.iter().map(|k| k.name.to_uppercase()).collect();
        SetTextCharacterExtra(dc, s(2));
        text(
            dc,
            &l.label,
            &names.join("  ·  "),
            Face::Label,
            s(14),
            primary.accent,
            DT_CENTER | DT_SINGLELINE,
        );
        SetTextCharacterExtra(dc, 0);
        text(
            dc,
            &l.count,
            &clock(b.left),
            Face::Light,
            s(150),
            TEXT,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );

        let line = DT_CENTER | DT_SINGLELINE | DT_END_ELLIPSIS;
        text(dc, &l.tips, tip(primary), Face::Text, s(28), TEXT, line);
        for (n, k) in order.iter().skip(1).enumerate() {
            let r = RECT {
                top: l.tips.top + s(56 + 32 * n as i32),
                ..l.tips
            };
            text(dc, &r, tip(k), Face::Text, s(19), DIM, line);
        }

        fill(dc, &l.bar, TRACK);
        let done = (l.bar.right - l.bar.left) as i64 * b.left as i64 / b.total.max(1) as i64;
        fill(
            dc,
            &RECT {
                right: l.bar.left + done as i32,
                ..l.bar
            },
            primary.accent,
        );
        button(dc, &l.skip, "Skip", "Esc", s);
        button(
            dc,
            &l.later,
            &format!("Postpone {}", dur(SNOOZE_MS)),
            "P",
            s,
        );

        BitBlt(
            hdc,
            l.block.left,
            l.block.top,
            w,
            h,
            dc,
            l.block.left,
            l.block.top,
            SRCCOPY,
        );
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
    }
}

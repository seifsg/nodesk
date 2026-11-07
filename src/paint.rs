//! Break screen drawing ("Dusk" look): per-monitor layout and GDI paint. 🎨
use crate::{
    kinds::{Kind, in_mask, rgb},
    sys::wide,
    text::{clock, dur},
    timer::{Break, SNOOZE_MS},
};
use std::{ptr::null_mut, sync::OnceLock};
use windows_sys::Win32::{
    Foundation::{POINT, RECT, SIZE},
    Graphics::Gdi::*,
};

pub const BG: u32 = rgb(0x0E1116);
const TEXT: u32 = rgb(0xF5F7FA);
const DIM: u32 = rgb(0x9AA4B2);
const TRACK: u32 = rgb(0x262E39);
const BUTTON: u32 = rgb(0x1B212A);
const BUTTON_HOT: u32 = rgb(0x262F3B);
const EDGE: u32 = rgb(0x2E3744);
/// Letter spacing of the kind label, in logical px.
const TRACKING: i32 = 2;

/// Logical px -> device px for one monitor: its DPI, plus up to 1.4x on tall screens
/// so the screen reads the same on a 1440p or 4K panel as on 1080p.
pub fn scale(m: &RECT, dpi: u32) -> impl Fn(i32) -> i32 {
    let logical_height = (m.bottom - m.top) as f32 * 96.0 / dpi as f32;
    let f = dpi as f32 / 96.0 * (logical_height / 1080.0).clamp(1.0, 1.4);
    move |v| (v as f32 * f).round() as i32
}

/// Where each element sits on one monitor (client coords).
pub struct Layout {
    pub block: RECT,
    label: RECT,
    count: RECT,
    pub bar: RECT,
    pub skip: RECT,
    pub later: RECT,
}

/// Centered column, scaled per monitor and clamped to its width.
pub fn layout(m: &RECT, dpi: u32) -> Layout {
    let s = scale(m, dpi);
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
        bar: row(400, 6, s(200).min(half)),
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

/// GDI face name for `f`, checked once for Windows 11's variable font.
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

/// ClearType font of `px` device pixels; the caller deletes it.
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

/// Runs `f` with `face` at `px` selected into `dc`, then frees the font.
fn with_font<R>(dc: HDC, f: Face, px: i32, run: impl FnOnce() -> R) -> R {
    unsafe {
        let font = font(face(f), px);
        let old = SelectObject(dc, font);
        let r = run();
        SelectObject(dc, old);
        DeleteObject(font);
        r
    }
}

/// Text in a rect with DrawText flags (centering, ellipsis).
fn text(dc: HDC, r: &RECT, s: &str, f: Face, px: i32, color: u32, flags: DRAW_TEXT_FORMAT) {
    with_font(dc, f, px, || unsafe {
        SetTextColor(dc, color);
        let mut r = *r;
        DrawTextW(dc, wide(s).as_ptr(), -1, &mut r, flags | DT_NOPREFIX);
    });
}

/// Rounded rect in `fill`, with a 1 px `edge` border when given.
fn rounded(dc: HDC, r: &RECT, radius: i32, fill: u32, edge: Option<u32>) {
    unsafe {
        let brush = CreateSolidBrush(fill);
        let pen = edge.map_or(GetStockObject(NULL_PEN), |c| CreatePen(PS_SOLID, 1, c));
        let (ob, op) = (SelectObject(dc, brush), SelectObject(dc, pen));
        let grow = edge.is_none() as i32; // NULL_PEN draws one pixel short
        RoundRect(
            dc,
            r.left,
            r.top,
            r.right + grow,
            r.bottom + grow,
            radius,
            radius,
        );
        SelectObject(dc, ob);
        SelectObject(dc, op);
        DeleteObject(brush);
        if edge.is_some() {
            DeleteObject(pen);
        }
    }
}

/// Draws `paint` into an off-screen copy of `r`, then blits it in one go: no flicker.
fn offscreen(hdc: HDC, r: &RECT, paint: impl FnOnce(HDC)) {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    unsafe {
        let dc = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        let old = SelectObject(dc, bmp);
        SetViewportOrgEx(dc, -r.left, -r.top, null_mut());
        SetBkMode(dc, TRANSPARENT as i32);
        paint(dc);
        BitBlt(hdc, r.left, r.top, w, h, dc, r.left, r.top, SRCCOPY);
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
    }
}

/// Capsule progress bar: dim track, accent fill that shrinks with `left` of `total`.
fn bar(dc: HDC, r: &RECT, left: u32, total: u32, accent: u32) {
    let radius = r.bottom - r.top;
    rounded(dc, r, radius, TRACK, None);
    let w = ((r.right - r.left) as u64 * left as u64 / total.max(1) as u64) as i32;
    if w > 0 {
        rounded(
            dc,
            &RECT {
                right: r.left + w,
                ..*r
            },
            radius,
            accent,
            None,
        );
    }
}

/// Repaints just the bar, e.g. 30 times a second for smooth motion.
pub fn draw_bar(hdc: HDC, l: &Layout, b: &Break, left: u32) {
    let pad = RECT {
        left: l.bar.left - 2,
        top: l.bar.top - 2,
        right: l.bar.right + 2,
        bottom: l.bar.bottom + 2,
    };
    offscreen(hdc, &pad, |dc| {
        fill(dc, &pad, BG);
        bar(dc, &l.bar, left, b.total, lead(b)[0].accent);
    });
}

/// Solid rectangle.
fn fill(dc: HDC, r: &RECT, color: u32) {
    rounded(dc, r, 0, color, None);
}

/// Kinds in `b`, longest first (it leads label color, big tip and bar); ties go to the
/// later row, so the eye break leads a micro-break. A break always holds at least one kind.
fn lead(b: &Break) -> Vec<&'static Kind> {
    let mut order: Vec<&Kind> = in_mask(b.mask).map(|(_, k)| k).collect();
    if let Some(i) = (0..order.len()).max_by_key(|&i| (order[i].length_ms, i)) {
        let first = order.remove(i);
        order.insert(0, first);
    }
    order
}

/// Kind names in their own accents, dot-separated, letter-spaced, truly centered.
fn label(dc: HDC, r: &RECT, kinds: &[&Kind], s: &impl Fn(i32) -> i32) {
    let extra = s(TRACKING);
    let mut parts = Vec::new();
    for (i, k) in kinds.iter().enumerate() {
        if i > 0 {
            parts.push(("  ·  ".to_string(), DIM));
        }
        parts.push((k.name.to_uppercase(), k.accent));
    }
    with_font(dc, Face::Label, s(14), || unsafe {
        SetTextCharacterExtra(dc, extra);
        let widths: Vec<i32> = parts
            .iter()
            .map(|(t, _)| {
                let mut size = SIZE { cx: 0, cy: 0 };
                let w = wide(t);
                GetTextExtentPoint32W(dc, w.as_ptr(), w.len() as i32 - 1, &mut size);
                size.cx
            })
            .collect();
        // The last letter's trailing spacing is not part of the visible word.
        let mut x = (r.left + r.right - widths.iter().sum::<i32>() + extra) / 2;
        for ((t, color), w) in parts.iter().zip(widths) {
            SetTextColor(dc, *color);
            let txt = wide(t);
            TextOutW(dc, x, r.top, txt.as_ptr(), txt.len() as i32 - 1);
            x += w;
        }
        SetTextCharacterExtra(dc, 0);
    });
}

/// Flat rounded button with a 1 px edge, lighter under the mouse, key hint in a keycap.
fn button(dc: HDC, r: &RECT, name: &str, key: &str, hot: bool, s: &impl Fn(i32) -> i32) {
    rounded(
        dc,
        r,
        s(12),
        if hot { BUTTON_HOT } else { BUTTON },
        Some(EDGE),
    );
    text(
        dc,
        r,
        name,
        Face::Strong,
        s(16),
        TEXT,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    let h = s(22);
    let cap = RECT {
        left: r.right - s(12) - s(14) - s(8) * key.len() as i32,
        top: (r.top + r.bottom - h) / 2,
        right: r.right - s(12),
        bottom: (r.top + r.bottom + h) / 2,
    };
    rounded(dc, &cap, s(6), BUTTON, Some(EDGE));
    text(
        dc,
        &cap,
        key,
        Face::Text,
        s(12),
        DIM,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

/// Paints one monitor's content block off-screen, then blits it.
/// `left` is the smooth countdown for the bar; `hot` is the cursor when it is over a button.
pub fn draw(hdc: HDC, m: &RECT, dpi: u32, b: &Break, left: u32, hot: Option<POINT>) {
    let l = layout(m, dpi);
    let s = scale(m, dpi);
    let order = lead(b);
    let tip = |k: &Kind| k.tips[b.rot as usize % k.tips.len()];
    let over = |r: &RECT| {
        hot.is_some_and(|p| (r.left..r.right).contains(&p.x) && (r.top..r.bottom).contains(&p.y))
    };
    offscreen(hdc, &l.block, |dc| {
        fill(dc, &l.block, BG);
        label(dc, &l.label, &order, &s);
        text(
            dc,
            &l.count,
            &clock(b.left),
            Face::Light,
            s(150),
            TEXT,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );

        // Tips sit centered between the countdown and the bar: one big, the rest small and dim.
        let height = s(44) + s(32) * (order.len() as i32 - 1);
        let mut y = (l.count.bottom + l.bar.top - height) / 2;
        let line = DT_CENTER | DT_SINGLELINE | DT_END_ELLIPSIS;
        for (i, k) in order.iter().enumerate() {
            let (px, color, h) = if i == 0 {
                (s(28), TEXT, s(44))
            } else {
                (s(19), DIM, s(32))
            };
            let r = RECT {
                top: y,
                bottom: y + h,
                ..l.count
            };
            text(dc, &r, tip(k), Face::Text, px, color, line);
            y += h;
        }

        bar(dc, &l.bar, left, b.total, order[0].accent);
        button(dc, &l.skip, "Skip", "Esc", over(&l.skip), &s);
        button(
            dc,
            &l.later,
            &format!("Postpone {}", dur(SNOOZE_MS)),
            "P",
            over(&l.later),
            &s,
        );
    });
}

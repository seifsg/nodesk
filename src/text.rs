//! Short human strings for the tray tooltip, menu and break screen. ✍️
use crate::{kinds::KINDS, stats::Day};

/// Rounded-up duration: "40 s", "7 min", "2 h 14 min".
pub fn dur(ms: u32) -> String {
    match ms.div_ceil(1000) {
        s @ 0..60 => format!("{s} s"),
        s @ 60..3600 => format!("{} min", s.div_ceil(60)),
        s => format!("{} h {} min", s / 3600, s / 60 % 60),
    }
}

/// Tray menu stats: today's active time, then one row per kind.
pub fn stats_lines(day: &Day) -> Vec<String> {
    let head = format!("Today: {} active", dur(day.active_ms));
    let rows = KINDS
        .iter()
        .zip(&day.counts)
        .map(|(k, [t, s, p])| format!("{}s\t{t} taken · {s} skipped · {p} postponed", k.name));
    std::iter::once(head).chain(rows).collect()
}

/// Countdown clock: "4:59", "0:20".
pub fn clock(ms: u32) -> String {
    let s = ms.div_ceil(1000);
    format!("{}:{:02}", s / 60, s % 60)
}

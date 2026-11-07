//! Short human strings for the tray tooltip, menu and break screen. ✍️

/// Rounded-up duration: "40 s", "7 min", "2 h 14 min".
pub fn dur(ms: u32) -> String {
    match ms.div_ceil(1000) {
        s @ 0..60 => format!("{s} s"),
        s @ 60..3600 => format!("{} min", s.div_ceil(60)),
        s => format!("{} h {} min", s / 3600, s / 60 % 60),
    }
}

/// Countdown clock: "4:59", "0:20".
pub fn clock(ms: u32) -> String {
    let s = ms.div_ceil(1000);
    format!("{}:{:02}", s / 60, s % 60)
}

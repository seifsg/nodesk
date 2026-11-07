//! NoDesk: a tray break reminder timed to real keyboard and mouse activity. 🚀
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod host;
mod kinds;
mod overlay;
mod paint;
mod stats;
mod store;
mod sys;
mod text;
mod timer;
mod tray;

fn main() {
    // Loud bugs: show the panic from a side thread so our own windows never re-enter mid-panic.
    std::panic::set_hook(Box::new(|p| {
        let msg = format!("NoDesk hit a bug and has to close.\n\n{p}");
        let _ = std::thread::spawn(move || sys::error_box(&msg)).join();
    }));
    if !sys::first_instance() {
        return sys::info_box("NoDesk is already running. Find it in your tray.");
    }
    host::run();
}

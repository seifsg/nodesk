//! App state and what happens each second: timers, today's stats, tray text. 🧠
use crate::{
    kinds::KINDS,
    stats::Day,
    sys::{fullscreen_busy, idle_ms, now_ms, today},
    text::dur,
    timer::{ACTIVE_MS, Timer},
    tray,
};
use std::cell::RefCell;
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::HICON};

/// Longest gap one tick may count as work: sleep or a stalled loop never counts as active time.
const DT_CAP_MS: u32 = 2_000;

pub struct App {
    pub host: HWND,
    icons: [HICON; 2],
    timer: Timer,
    pub day: Day,
    pub paused: bool,
    held: bool,
    last: u32,
    speed: u32,
    shown: (bool, String),
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Installs the app state for this (UI) thread.
pub fn init(app: App) {
    APP.set(Some(app));
}

/// Runs `f` on the app state. Never show modal UI (menus, message boxes) inside `f`.
pub fn with<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with_borrow_mut(|a| f(a.as_mut().expect("app state used before init")))
}

impl App {
    /// `icons`: [normal, paused]; `speed`: demo multiplier for active time (1 = real).
    pub fn new(host: HWND, icons: [HICON; 2], speed: u32) -> Self {
        Self {
            host,
            icons,
            timer: Timer::default(),
            day: Day::new(today()),
            paused: false,
            held: false,
            last: now_ms(),
            speed,
            shown: (false, String::new()),
        }
    }

    /// Once a second: roll the day, advance timers on real activity, refresh the tray.
    pub fn tick(&mut self) {
        let now = now_ms();
        let dt = now.wrapping_sub(self.last).min(DT_CAP_MS);
        self.last = now;
        self.roll_day();
        if !self.paused {
            let idle = idle_ms();
            if idle < ACTIVE_MS {
                self.day.active_ms += dt;
            }
            let due = self.timer.tick(dt * self.speed, idle);
            self.held = due != 0 && fullscreen_busy();
            #[cfg(debug_assertions)]
            eprintln!(
                "idle {idle} ms, due {due:04b}, next {:?}",
                self.timer.next()
            );
        }
        self.refresh_tray();
    }

    /// A new local date starts a fresh day of stats.
    fn roll_day(&mut self) {
        let d = today();
        if d != self.day.date {
            self.day = Day::new(d);
        }
    }

    /// Tooltip + icon, pushed to the shell only when they change.
    pub fn refresh_tray(&mut self) {
        let quiet = self.paused || self.held;
        let state = if self.paused {
            "Paused".into()
        } else if self.held {
            "Break waits for your fullscreen app".into()
        } else {
            let (i, left) = self.timer.next();
            format!("{} in {}", KINDS[i].name, dur(left / self.speed))
        };
        let tip = format!("NoDesk\n{state}\nToday: {} active", dur(self.day.active_ms));
        if self.shown.0 != quiet || self.shown.1 != tip {
            tray::update(self.host, self.icons[quiet as usize], &tip);
            #[cfg(debug_assertions)]
            eprintln!("tray: {}", tip.replace('\n', " | "));
            self.shown = (quiet, tip);
        }
    }

    /// Puts the icon back, e.g. after Explorer restarted.
    pub fn add_tray(&mut self) {
        tray::add(self.host, self.icons[0], "NoDesk");
        self.shown = (false, String::new());
        self.refresh_tray();
    }
}

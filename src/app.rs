//! App state and what happens each second: timers, today's stats, tray text. 🧠
use crate::{
    kinds::{KINDS, Mask},
    overlay,
    stats::Day,
    store,
    sys::{fullscreen_busy, idle_ms, now_ms, today},
    text::{clock, dur},
    timer::{ACTIVE_MS, Break, Outcome, Timer, longest},
    tray,
};
use std::cell::RefCell;
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::HICON};

/// Longest gap one tick may count as work: sleep or a stalled loop never counts as active time.
const DT_CAP_MS: u32 = 2_000;
/// Active time between stats saves (outcomes, pause, quit and a new day save at once).
const SAVE_EVERY_MS: u32 = 5 * 60_000;

pub struct App {
    pub host: HWND,
    overlay: HWND,
    icons: [HICON; 2],
    timer: Timer,
    pub day: Day,
    /// Earlier days as stored; None when the file could not be read, so it is never overwritten.
    history: Option<String>,
    unsaved: u32,
    save_failed: bool,
    pub paused: bool,
    held: bool,
    brk: Option<Break>,
    rot: u32,
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
    /// `icons`: [normal, paused]; `speed`: demo multiplier for active time (1 = real);
    /// `stored`: what `store::load` returned (history None = do not save).
    pub fn new(
        host: HWND,
        overlay: HWND,
        icons: [HICON; 2],
        speed: u32,
        stored: (Option<String>, Day),
    ) -> Self {
        Self {
            host,
            overlay,
            icons,
            timer: Timer::default(),
            history: stored.0,
            day: stored.1,
            unsaved: 0,
            save_failed: false,
            paused: false,
            held: false,
            brk: None,
            rot: now_ms(),
            last: now_ms(),
            speed,
            shown: (false, String::new()),
        }
    }

    /// Once a second: roll the day, run the break countdown or the timers, refresh the tray.
    pub fn tick(&mut self) {
        let now = now_ms();
        let real = now.wrapping_sub(self.last);
        self.last = now;
        self.roll_day();
        if let Some(b) = &mut self.brk {
            // Real time, uncapped: a break slept through is a break taken.
            b.left = b.left.saturating_sub(real);
            match *b {
                Break { left: 0, .. } => self.finish(Outcome::Taken),
                b => overlay::update(self.overlay, b),
            }
        } else if !self.paused {
            let (idle, dt) = (idle_ms(), real.min(DT_CAP_MS));
            if idle < ACTIVE_MS {
                self.day.active_ms = self.day.active_ms.saturating_add(dt);
                self.unsaved = self.unsaved.saturating_add(dt);
            }
            if self.unsaved >= SAVE_EVERY_MS {
                self.save();
            }
            let due = self.timer.tick(dt * self.speed, idle);
            self.held = due != 0 && fullscreen_busy();
            if due != 0 && !self.held {
                self.start(due);
            }
            #[cfg(debug_assertions)]
            eprintln!(
                "idle {idle} ms, due {due:04b}, next {:?}",
                self.timer.next()
            );
        }
        self.refresh_tray();
    }

    /// Shows one screen for every kind due now.
    fn start(&mut self, due: Mask) {
        let total = longest(due);
        let b = Break {
            mask: due,
            left: total,
            total,
            rot: self.rot,
        };
        self.rot = self.rot.wrapping_add(1);
        self.brk = Some(b);
        #[cfg(debug_assertions)]
        eprintln!("start mask {due:04b}");
        overlay::show(self.overlay, b);
    }

    /// Ends the break on screen with its outcome: timers reset or snooze, today's counts grow.
    pub fn finish(&mut self, o: Outcome) {
        if let Some(b) = self.brk.take() {
            #[cfg(debug_assertions)]
            eprintln!("finish {o:?} mask {:04b} left {} ms", b.mask, b.left);
            self.timer.finish(b.mask, o);
            self.day.add(b.mask, o);
            overlay::hide(self.overlay);
            self.save();
            self.refresh_tray();
        }
    }

    /// A new local date files yesterday's row and starts today at zero.
    fn roll_day(&mut self) {
        let d = today();
        if d != self.day.date {
            if let Some(h) = &mut self.history {
                *h += &self.day.line();
                h.push('\n');
            }
            self.day = Day::new(d);
            self.save();
        }
    }

    /// Writes stats now. A failure warns once (balloon) until a save works again.
    pub fn save(&mut self) {
        self.unsaved = 0;
        let Some(h) = &self.history else { return };
        match store::save(h, &self.day) {
            Ok(()) => self.save_failed = false,
            Err(e) if !self.save_failed => {
                self.save_failed = true;
                tray::warn(self.host, &format!("Could not save today's stats: {e}"));
            }
            Err(_) => {}
        }
    }

    /// Tooltip + icon, pushed to the shell only when they change.
    pub fn refresh_tray(&mut self) {
        let quiet = self.paused; // grey icon means paused, nothing else
        let state = if let Some(b) = &self.brk {
            format!("Break now, {} left", clock(b.left))
        } else if self.paused {
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

    /// Pause stops every timer (and today's active time) until Resume.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.held = false;
        self.save();
        self.refresh_tray();
    }

    /// Puts the icon back, e.g. after Explorer restarted.
    pub fn add_tray(&mut self) {
        tray::add(self.host, self.icons[0], "NoDesk");
        self.shown = (false, String::new());
        self.refresh_tray();
    }
}

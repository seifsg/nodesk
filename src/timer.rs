//! Break timers on active time. Pure logic, no OS calls, so it is unit-tested. ⏱️
use crate::kinds::{KINDS, Mask, in_mask};

/// Input this recent means you are working.
pub const ACTIVE_MS: u32 = 10_000;
/// Postpone pushes a break this far out.
pub const SNOOZE_MS: u32 = 5 * 60_000;

/// What happened to a shown break. Order = stats column order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Taken,
    Skipped,
    Postponed,
}

/// Active ms counted toward each kind since its last break.
#[derive(Default)]
pub struct Timer {
    elapsed: [u32; KINDS.len()],
}

impl Timer {
    /// Adds `dt` while input is recent; idling a natural break's length resets it. Returns due kinds.
    pub fn tick(&mut self, dt: u32, idle: u32) -> Mask {
        let mut due = 0;
        for (i, (k, e)) in KINDS.iter().zip(&mut self.elapsed).enumerate() {
            if k.natural && idle >= k.length_ms {
                *e = 0;
            } else if idle < ACTIVE_MS {
                *e = e.saturating_add(dt);
            }
            if *e >= k.every_ms {
                due |= 1 << i;
            }
        }
        due
    }

    /// Applies a shown break's outcome. Taken also covers natural kinds strictly shorter than it,
    /// so a rest break counts as micro + eye, but a micro-break never hides the eye break.
    pub fn finish(&mut self, mask: Mask, o: Outcome) {
        let len = longest(mask);
        for (i, (k, e)) in KINDS.iter().zip(&mut self.elapsed).enumerate() {
            let shown = mask >> i & 1 == 1;
            match o {
                Outcome::Postponed if shown => *e = k.every_ms.saturating_sub(SNOOZE_MS),
                Outcome::Skipped if shown => *e = 0,
                Outcome::Taken if shown || k.natural && k.length_ms < len => *e = 0,
                _ => {}
            }
        }
    }

    /// Soonest kind and the active ms left until it is due.
    pub fn next(&self) -> (usize, u32) {
        (0..KINDS.len())
            .map(|i| (i, KINDS[i].every_ms.saturating_sub(self.elapsed[i])))
            .min_by_key(|&(_, left)| left)
            .expect("KINDS is never empty")
    }
}

/// Longest length among `mask`: a merged break lasts this long.
pub fn longest(mask: Mask) -> u32 {
    in_mask(mask).map(|(_, k)| k.length_ms).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    const MIN: u32 = 60_000;
    const MICRO: Mask = 1;
    const EYE: Mask = 2;
    const REST: Mask = 4;
    const WATER: Mask = 8;

    /// `ms` of steady work in 1 s ticks; returns the last due mask.
    fn work(t: &mut Timer, ms: u32) -> Mask {
        (0..ms / 1000).map(|_| t.tick(1000, 0)).last().unwrap_or(0)
    }

    #[test]
    fn kinds_hold_invariants() {
        for k in &KINDS {
            assert!(k.every_ms > SNOOZE_MS && k.every_ms > k.length_ms && !k.tips.is_empty());
            assert!(
                !k.natural || k.length_ms > ACTIVE_MS,
                "natural reset must outlast the active window"
            );
        }
    }

    #[test]
    fn only_activity_counts_and_idle_resets() {
        let mut t = Timer::default();
        assert_eq!(work(&mut t, 10 * MIN - 1000), 0);
        assert_eq!(t.tick(1000, ACTIVE_MS), 0, "idle 10 s pauses");
        assert_eq!(t.tick(1000, 0), MICRO);
        t.tick(1000, 20_000);
        assert_eq!(
            t.next(),
            (0, 10 * MIN),
            "20 s idle = natural micro + eye break"
        );
        assert_eq!(
            work(&mut t, 35 * MIN) & WATER,
            WATER,
            "water keeps counting through idle"
        );
    }

    #[test]
    fn merged_breaks_reset_by_length() {
        let mut t = Timer::default();
        assert_eq!(work(&mut t, 10 * MIN), MICRO);
        t.finish(MICRO, Outcome::Taken);
        assert_eq!(
            work(&mut t, 10 * MIN),
            MICRO | EYE,
            "micro never hides the eye break"
        );
        t.finish(MICRO | EYE, Outcome::Taken);
        assert_eq!(work(&mut t, 40 * MIN), MICRO | EYE | REST | WATER);
        t.finish(REST, Outcome::Taken);
        assert_eq!(t.tick(0, 0), WATER, "rest covers micro + eye, never water");
    }

    #[test]
    fn skip_and_postpone() {
        let mut t = Timer::default();
        assert_eq!(work(&mut t, 45 * MIN) & WATER, WATER);
        t.finish(WATER, Outcome::Postponed);
        assert_eq!(t.tick(0, 0) & WATER, 0);
        assert_eq!(
            work(&mut t, SNOOZE_MS) & WATER,
            WATER,
            "postponed comes back after the snooze"
        );
        t.finish(WATER, Outcome::Skipped);
        assert_eq!(t.tick(0, 0) & WATER, 0);
    }
}

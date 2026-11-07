//! One day's numbers and the TSV line they are stored as. Pure, so it is unit-tested. 📊
use crate::{
    kinds::{KINDS, Mask, in_mask},
    timer::Outcome,
};

/// Today's active time and, per kind, [taken, skipped, postponed] counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub date: u32,
    pub active_ms: u32,
    pub counts: [[u32; 3]; KINDS.len()],
}

/// Column names, e.g. `date  active_s  micro_taken  micro_skipped  micro_postponed ...`.
pub fn header() -> String {
    KINDS.iter().fold("date\tactive_s".into(), |h, k| {
        format!("{h}\t{0}_taken\t{0}_skipped\t{0}_postponed", k.key)
    })
}

impl Day {
    pub fn new(date: u32) -> Self {
        Self {
            date,
            active_ms: 0,
            counts: [[0; 3]; KINDS.len()],
        }
    }

    /// Counts one outcome for every kind the break showed.
    pub fn add(&mut self, mask: Mask, o: Outcome) {
        in_mask(mask).for_each(|(i, _)| self.counts[i][o as usize] += 1);
    }

    /// `2026-11-07  3600  8  1  0 ...`: one TSV row.
    pub fn line(&self) -> String {
        let d = self.date;
        let head = format!(
            "{:04}-{:02}-{:02}\t{}",
            d / 10_000,
            d / 100 % 100,
            d % 100,
            self.active_ms / 1000
        );
        self.counts
            .iter()
            .flatten()
            .fold(head, |l, n| format!("{l}\t{n}"))
    }

    /// Reads a row written by `line`; None for anything else (header, corrupt, other layout).
    pub fn parse(row: &str) -> Option<Self> {
        let mut f = row.split('\t');
        let date = f
            .next()
            .filter(|d| d.len() == 10)?
            .replace('-', "")
            .parse()
            .ok()?;
        let active_s: u32 = f.next()?.parse().ok()?;
        let mut day = Self {
            date,
            active_ms: active_s.checked_mul(1000)?,
            ..Self::new(date)
        };
        for n in day.counts.iter_mut().flatten() {
            *n = f.next()?.parse().ok()?;
        }
        f.next().is_none().then_some(day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_round_trips_and_counts_outcomes() {
        let mut d = Day::new(20261107);
        d.active_ms = 3_600_500;
        d.add(0b0011, Outcome::Taken);
        d.add(0b1000, Outcome::Postponed);
        let line = d.line();
        assert!(line.starts_with("2026-11-07\t3600\t1\t0\t0\t1\t0\t0\t0\t0\t0\t0\t0\t1"));
        assert_eq!(
            Day::parse(&line),
            Some(Day {
                active_ms: 3_600_000,
                ..d
            })
        );
        assert_eq!(header().split('\t').count(), line.split('\t').count());
    }

    #[test]
    fn rejects_anything_else() {
        for bad in [
            "",
            &header(),
            "2026-11-07\t12",
            "2026-11-07\tx\t0",
            "garbage",
            &format!("{}\t9", Day::new(20261107).line()),
        ] {
            assert_eq!(Day::parse(bad), None, "{bad:?}");
        }
    }
}

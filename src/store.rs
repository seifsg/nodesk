//! Stats file: `%APPDATA%\NoDesk\stats.tsv`, a header plus one row per day, today's row last. 💾
use crate::stats::{Day, header};
use std::{fs, io, path::PathBuf};

/// The stats file path; Err when APPDATA is missing, so stats switch off and breaks keep working.
fn path() -> io::Result<PathBuf> {
    let base = std::env::var_os("APPDATA").ok_or_else(|| io::Error::other("APPDATA is not set"))?;
    Ok(PathBuf::from(base).join("NoDesk").join("stats.tsv"))
}

/// Past rows (kept verbatim, even odd ones) and today's counters.
/// Missing file: fresh start. Unreadable file: Err, so the caller never overwrites it.
pub fn load(today: u32) -> io::Result<(String, Day)> {
    let text = match fs::read_to_string(path()?) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        r => r?,
    };
    let mut rows: Vec<&str> = text.lines().collect();
    let day = rows
        .last()
        .and_then(|r| Day::parse(r))
        .filter(|d| d.date == today);
    if day.is_some() {
        rows.pop();
    }
    let mut history = if rows.is_empty() {
        header()
    } else {
        rows.join("\n")
    };
    history.push('\n');
    Ok((history, day.unwrap_or_else(|| Day::new(today))))
}

/// Writes history + today's row through a temp file and a rename: a crash never truncates history.
pub fn save(history: &str, day: &Day) -> io::Result<()> {
    let p = path()?;
    fs::create_dir_all(p.parent().expect("stats path has a folder"))?;
    let tmp = p.with_extension("tmp");
    fs::write(&tmp, format!("{history}{}\n", day.line()))?;
    fs::rename(tmp, p)
}

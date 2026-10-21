//! Loading and validating the day_NN/loads files.

use std::fs;
use std::path::Path;

use crate::error::LoadError;

pub const EXPECTED_DAYS: usize = 32;
pub const EXPECTED_PROGRAMS_PER_DAY: usize = 128;

/// One day's snapshot: every program's request, in bits.
#[derive(Debug, Clone)]
pub struct Day {
    pub number: u32,
    pub requests: Vec<u64>,
}

/// Everything loaded, plus warnings about counts that differ from the spec.
/// Warnings are returned instead of printed so the caller decides how loudly
/// to report them, and so tests can check them.
#[derive(Debug)]
pub struct Workloads {
    pub days: Vec<Day>,
    pub warnings: Vec<String>,
}

impl Workloads {
    pub fn all_requests(&self) -> impl Iterator<Item = u64> + '_ {
        self.days.iter().flat_map(|d| d.requests.iter().copied())
    }
}

/// Parses the text of one loads file. `path` is only used in error messages.
/// Line numbers are 1 based to match what an editor shows.
pub fn parse_loads(path: &Path, contents: &str) -> Result<Vec<u64>, LoadError> {
    let mut requests = Vec::new();
    for (index, raw) in contents.lines().enumerate() {
        let line = index + 1;
        // trim() also strips a trailing '\r', so Windows line endings still parse.
        let text = raw.trim();
        if text.is_empty() {
            return Err(LoadError::EmptyLine {
                path: path.to_path_buf(),
                line,
            });
        }
        // Parse as signed first so "-5" is reported as NotPositive, not as garbage.
        let value: i64 = text.parse().map_err(|_| LoadError::NotAnInteger {
            path: path.to_path_buf(),
            line,
            text: text.to_string(),
        })?;
        if value <= 0 {
            return Err(LoadError::NotPositive {
                path: path.to_path_buf(),
                line,
                value,
            });
        }
        // Cannot fail: value is positive, so it fits in u64.
        requests.push(value as u64);
    }
    Ok(requests)
}

pub fn load_day_file(path: &Path) -> Result<Vec<u64>, LoadError> {
    let contents =
        fs::read_to_string(path).map_err(|e| LoadError::from_io(path.to_path_buf(), e))?;
    parse_loads(path, &contents)
}

/// Loads every day_NN/loads under `dir`. Hard errors (missing loads file, bad
/// line) stop everything. Count mismatches only produce warnings.
pub fn load_workloads(dir: &Path) -> Result<Workloads, LoadError> {
    let mut warnings = Vec::new();
    let mut day_dirs = Vec::new();

    let entries = fs::read_dir(dir).map_err(|e| LoadError::from_io(dir.to_path_buf(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| LoadError::from_io(dir.to_path_buf(), e))?;
        let path = entry.path();
        // Skip anything that is not a day_NN folder (.DS_Store, __MACOSX, ...).
        let Some(suffix) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.strip_prefix("day_").map(str::to_owned))
        else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let Ok(number) = suffix.parse::<u32>() else {
            warnings.push(format!("ignoring {}: not a day_NN name", path.display()));
            continue;
        };
        day_dirs.push((number, path));
    }

    if day_dirs.is_empty() {
        return Err(LoadError::NoDays {
            path: dir.to_path_buf(),
        });
    }
    // read_dir order is unspecified. Sort by number BEFORE loading so that when
    // several files are bad, the reported error is always the lowest day.
    day_dirs.sort_by_key(|(number, _)| *number);

    let mut days = Vec::new();
    for (number, path) in day_dirs {
        let requests = load_day_file(&path.join("loads"))?;
        days.push(Day { number, requests });
    }

    if days.len() != EXPECTED_DAYS {
        warnings.push(format!(
            "expected {EXPECTED_DAYS} day folders, found {}",
            days.len()
        ));
    }
    for day in &days {
        if day.requests.len() != EXPECTED_PROGRAMS_PER_DAY {
            warnings.push(format!(
                "day_{:02}: expected {EXPECTED_PROGRAMS_PER_DAY} programs, found {}",
                day.number,
                day.requests.len()
            ));
        }
    }

    Ok(Workloads { days, warnings })
}

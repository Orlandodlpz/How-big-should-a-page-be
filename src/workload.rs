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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::path::PathBuf;

    // LoadError cannot derive PartialEq (io::Error does not implement it), so
    // these tests check variants with matches! instead of assert_eq!.

    fn fake() -> &'static Path {
        Path::new("day_99/loads")
    }

    #[test]
    fn parses_valid_lines_and_keeps_duplicates() {
        let got = parse_loads(fake(), "5\n6\n6\n7\n");
        assert_eq!(got.ok(), Some(vec![5, 6, 6, 7]));
    }

    #[test]
    fn missing_final_newline_and_crlf_are_fine() {
        assert_eq!(parse_loads(fake(), "5\n6").ok(), Some(vec![5, 6]));
        assert_eq!(parse_loads(fake(), "5\r\n6\r\n").ok(), Some(vec![5, 6]));
    }

    #[test]
    fn empty_file_is_ok_but_empty() {
        // Not an error at this level. load_workloads warns about the count.
        assert_eq!(parse_loads(fake(), "").ok(), Some(vec![]));
    }

    #[test]
    fn blank_line_is_empty_line_error_with_line_number() {
        let err = parse_loads(fake(), "5\n\n7\n").unwrap_err();
        assert!(
            matches!(err, LoadError::EmptyLine { line: 2, .. }),
            "{err:?}"
        );
        // Whitespace only counts as blank too.
        let err = parse_loads(fake(), "5\n   \n").unwrap_err();
        assert!(
            matches!(err, LoadError::EmptyLine { line: 2, .. }),
            "{err:?}"
        );
        // An extra blank line at the end of the file is still a blank line.
        let err = parse_loads(fake(), "5\n6\n\n").unwrap_err();
        assert!(
            matches!(err, LoadError::EmptyLine { line: 3, .. }),
            "{err:?}"
        );
    }

    #[test]
    fn non_integers_are_not_an_integer_errors() {
        for (input, bad) in [("5\n12.5\n", "12.5"), ("abc\n", "abc"), ("1e6\n", "1e6")] {
            let err = parse_loads(fake(), input).unwrap_err();
            assert!(
                matches!(&err, LoadError::NotAnInteger { text, .. } if text == bad),
                "{err:?}"
            );
        }
        // Too big for 64 bits also lands here.
        let err = parse_loads(fake(), "99999999999999999999\n").unwrap_err();
        assert!(
            matches!(err, LoadError::NotAnInteger { line: 1, .. }),
            "{err:?}"
        );
    }

    #[test]
    fn zero_and_negative_are_not_positive_errors() {
        let err = parse_loads(fake(), "0\n").unwrap_err();
        assert!(
            matches!(
                err,
                LoadError::NotPositive {
                    line: 1,
                    value: 0,
                    ..
                }
            ),
            "{err:?}"
        );
        let err = parse_loads(fake(), "5\n-3\n").unwrap_err();
        assert!(
            matches!(
                err,
                LoadError::NotPositive {
                    line: 2,
                    value: -3,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn errors_carry_the_path_and_display_it() {
        let err = parse_loads(fake(), "5\nabc\n").unwrap_err();
        assert!(matches!(&err, LoadError::NotAnInteger { path, .. } if path == fake()));
        assert_eq!(
            err.to_string(),
            "day_99/loads:2: \"abc\" is not a whole number of bits"
        );
    }

    #[test]
    fn missing_file_is_missing_file_error() {
        let path = Path::new("definitely/not/here/loads");
        let err = load_day_file(path).unwrap_err();
        assert!(
            matches!(&err, LoadError::MissingFile { path: p } if p == path),
            "{err:?}"
        );
    }

    /// A scratch directory under the system temp folder, deleted on drop.
    /// The name includes the process id and test name so parallel tests
    /// never share a folder.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Result<Self, Box<dyn Error>> {
            let dir =
                std::env::temp_dir().join(format!("page_size_test_{}_{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir)?;
            Ok(Scratch(dir))
        }

        fn day(&self, number: u32, contents: Option<&str>) -> Result<(), Box<dyn Error>> {
            let day = self.0.join(format!("day_{number:02}"));
            fs::create_dir_all(&day)?;
            if let Some(text) = contents {
                fs::write(day.join("loads"), text)?;
            }
            Ok(())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_directory_is_missing_file_error() {
        let err = load_workloads(Path::new("no/such/workloads")).unwrap_err();
        assert!(matches!(err, LoadError::MissingFile { .. }), "{err:?}");
    }

    #[test]
    fn directory_without_days_is_no_days_error() -> Result<(), Box<dyn Error>> {
        let s = Scratch::new("no_days")?;
        fs::write(s.0.join(".DS_Store"), "junk")?;
        let err = load_workloads(&s.0).unwrap_err();
        assert!(matches!(err, LoadError::NoDays { .. }), "{err:?}");
        Ok(())
    }

    #[test]
    fn day_folder_without_loads_is_missing_file_error() -> Result<(), Box<dyn Error>> {
        let s = Scratch::new("no_loads")?;
        s.day(1, Some("5\n"))?;
        s.day(2, None)?;
        let err = load_workloads(&s.0).unwrap_err();
        assert!(
            matches!(&err, LoadError::MissingFile { path } if path.ends_with("day_02/loads")),
            "{err:?}"
        );
        Ok(())
    }

    #[test]
    fn lowest_bad_day_is_reported_first() -> Result<(), Box<dyn Error>> {
        // Regression test for the Phase 0 read_dir ordering bug.
        let s = Scratch::new("ordering")?;
        for n in [3, 1, 2] {
            s.day(n, Some("5\nbad\n"))?;
        }
        let err = load_workloads(&s.0).unwrap_err();
        assert!(
            matches!(&err, LoadError::NotAnInteger { path, line: 2, .. } if path.ends_with("day_01/loads")),
            "{err:?}"
        );
        Ok(())
    }

    #[test]
    fn wrong_counts_warn_but_still_load() -> Result<(), Box<dyn Error>> {
        let s = Scratch::new("counts")?;
        s.day(2, Some("7\n8\n"))?;
        s.day(1, Some("5\n"))?;
        let w = load_workloads(&s.0)?;
        // Sorted by day number regardless of creation order.
        let numbers: Vec<u32> = w.days.iter().map(|d| d.number).collect();
        assert_eq!(numbers, vec![1, 2]);
        assert_eq!(w.warnings.len(), 3, "{:?}", w.warnings); // 1 day count + 2 program counts
        assert!(w.warnings[0].contains("expected 32 day folders, found 2"));
        Ok(())
    }
}

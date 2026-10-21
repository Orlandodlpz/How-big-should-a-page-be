//! Writing results to CSV files and formatting numbers for the terminal.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::AppError;
use crate::model::Cost;
use crate::sweep::page_size_at;

/// Mean daily score as text. With 32 days the division is exact in 5
/// decimals (1/32 = 0.03125), so nothing is rounded away.
pub fn mean_text(total: u64, days: usize) -> String {
    format!("{:.5}", total as f64 / days as f64)
}

/// 1234567 -> "1,234,567", for readable terminal output only.
pub fn commas(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Opens `path` for buffered writing, creating its folder if needed.
/// BufWriter batches many small writes into few system calls, which matters
/// for 131,072 rows.
fn create(path: &Path) -> Result<BufWriter<File>, AppError> {
    let wrap = |source| AppError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(wrap)?;
    }
    File::create(path).map(BufWriter::new).map_err(wrap)
}

/// Writes rows with `write_rows`, then flushes. Flushing explicitly matters:
/// BufWriter flushes on drop too, but there it has to ignore any error.
fn write_csv<F>(path: &Path, header: &str, write_rows: F) -> Result<(), AppError>
where
    F: FnOnce(&mut BufWriter<File>) -> std::io::Result<()>,
{
    let mut out = create(path)?;
    let result = writeln!(out, "{header}")
        .and_then(|()| write_rows(&mut out))
        .and_then(|()| out.flush());
    result.map_err(|source| AppError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Every page size from 1 to `last` (or the end of the sweep).
pub fn write_sweep_csv(
    path: &Path,
    totals: &[u64],
    last: u64,
    days: usize,
) -> Result<(), AppError> {
    let rows = totals
        .iter()
        .enumerate()
        .take_while(|&(i, _)| page_size_at(i) <= last);
    write_csv(
        path,
        "page_size_bits,total_penalty_bits,mean_penalty_bits",
        |out| {
            for (i, &t) in rows {
                writeln!(out, "{},{t},{}", page_size_at(i), mean_text(t, days))?;
            }
            Ok(())
        },
    )
}

/// One row per block of the tail: its range and its best page size.
pub fn write_tail_csv(
    path: &Path,
    blocks: &[(u64, u64, u64, u64)],
    days: usize,
) -> Result<(), AppError> {
    write_csv(
        path,
        "block_first_bits,block_last_bits,best_page_size_bits,best_total_penalty_bits,best_mean_penalty_bits",
        |out| {
            for &(first, last, best_p, best_t) in blocks {
                writeln!(
                    out,
                    "{first},{last},{best_p},{best_t},{}",
                    mean_text(best_t, days)
                )?;
            }
            Ok(())
        },
    )
}

/// One day's line in the per day table at the chosen page size.
pub struct DayRow {
    pub day: u32,
    pub programs: usize,
    pub cost: Cost,
}

pub fn write_per_day_csv(path: &Path, rows: &[DayRow]) -> Result<(), AppError> {
    write_csv(
        path,
        "day,programs,pages,bookkeeping_bits,unused_bits,total_penalty_bits",
        |out| {
            for r in rows {
                writeln!(
                    out,
                    "{},{},{},{},{},{}",
                    r.day,
                    r.programs,
                    r.cost.pages,
                    r.cost.bookkeeping_bits,
                    r.cost.unused_bits,
                    r.cost.penalty
                )?;
            }
            Ok(())
        },
    )
}

/// EXPLORATION ONLY: the best page size if this one day could choose alone.
pub struct DayBest {
    pub day: u32,
    pub best_sizes: Vec<u64>,
    pub best_total: u64,
    pub at_overall_winner: u64,
}

pub fn write_exploration_csv(path: &Path, rows: &[DayBest]) -> Result<(), AppError> {
    write_csv(
        path,
        "day,best_page_sizes_bits,best_day_penalty_bits,day_penalty_at_overall_winner_bits,extra_bits_from_shared_size",
        |out| {
            for r in rows {
                let sizes: Vec<String> = r.best_sizes.iter().map(u64::to_string).collect();
                writeln!(
                    out,
                    "{},{},{},{},{}",
                    r.day,
                    sizes.join(" "),
                    r.best_total,
                    r.at_overall_winner,
                    r.at_overall_winner - r.best_total
                )?;
            }
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commas_groups_thousands() {
        assert_eq!(commas(0), "0");
        assert_eq!(commas(999), "999");
        assert_eq!(commas(1000), "1,000");
        assert_eq!(commas(34_392_541_729), "34,392,541,729");
    }

    #[test]
    fn mean_is_exact_for_32_days() {
        // 117,304,799 / 32 = 3,665,774.96875 exactly.
        assert_eq!(mean_text(117_304_799, 32), "3665774.96875");
    }
}

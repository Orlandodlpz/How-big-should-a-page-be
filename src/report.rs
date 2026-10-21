//! Writing results to CSV files and formatting numbers for the terminal.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::AppError;
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

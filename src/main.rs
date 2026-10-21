use std::path::PathBuf;
use std::process::ExitCode;

use page_size::error::LoadError;
use page_size::workload::{self, EXPECTED_PROGRAMS_PER_DAY};

const DEFAULT_DATA_DIR: &str = "data/workloads";

/// main only translates the Result into an exit code and a readable message.
/// Returning Result from main directly would print the Debug form, which is
/// noisier than our Display messages.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), LoadError> {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));

    let loaded = workload::load_workloads(&dir)?;
    for w in &loaded.warnings {
        eprintln!("WARNING: {w}");
    }

    let total_programs: usize = loaded.days.iter().map(|d| d.requests.len()).sum();
    // Every loads file could be empty (a warning, not an error), so min/max
    // can be None. Fall back to 0 rather than unwrap.
    let min = loaded.all_requests().min().unwrap_or(0);
    let max = loaded.all_requests().max().unwrap_or(0);
    let sum: u64 = loaded.all_requests().sum();

    println!("data directory:   {}", dir.display());
    println!("days:             {}", loaded.days.len());
    if loaded
        .days
        .iter()
        .all(|d| d.requests.len() == EXPECTED_PROGRAMS_PER_DAY)
    {
        println!("programs per day: {EXPECTED_PROGRAMS_PER_DAY} on every day");
    } else {
        for d in &loaded.days {
            println!("  day_{:02}: {} programs", d.number, d.requests.len());
        }
    }
    println!("total programs:   {total_programs}");
    println!("total requested:  {sum} bits");
    println!("min request:      {min} bits");
    println!("max request:      {max} bits");
    if total_programs > 0 {
        println!(
            "mean request:     {:.2} bits",
            sum as f64 / total_programs as f64
        );
    }
    if let Some(first) = loaded.days.first() {
        let head: Vec<String> = first.requests.iter().take(3).map(u64::to_string).collect();
        println!("day_{:02} first 3:  {}", first.number, head.join(", "));
    }
    Ok(())
}

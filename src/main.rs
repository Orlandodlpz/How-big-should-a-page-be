use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::Instant;

use page_size::error::AppError;
use page_size::report::{self, commas, mean_text};
use page_size::sweep::{self, total_at};
use page_size::workload::{self, EXPECTED_PROGRAMS_PER_DAY, Workloads};

const DEFAULT_DATA_DIR: &str = "data/workloads";
const USAGE: &str = "usage: page_size [summary|sweep] [--data DIR] [--check P]...";

/// Dense CSV covers every size up to here (2^17, four times the continuous
/// estimate). Beyond it, sweep_tail.csv keeps the best size per block.
const DENSE_CSV_LAST: u64 = 131_072;
const TAIL_BLOCK: usize = 4096;

/// main only translates the Result into an exit code and a readable message.
/// Returning Result from main directly would print the Debug form, which is
/// noisier than our Display messages.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            if matches!(e, AppError::Usage(_)) {
                eprintln!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}

enum Command {
    Summary,
    Sweep,
}

struct Args {
    command: Command,
    data_dir: PathBuf,
    checks: Vec<NonZeroU64>,
}

/// Bad arguments are user input, so they get a Result like bad data does.
fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, AppError> {
    let mut parsed = Args {
        command: Command::Summary,
        data_dir: PathBuf::from(DEFAULT_DATA_DIR),
        checks: Vec::new(),
    };
    let mut first = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "summary" if first => parsed.command = Command::Summary,
            "sweep" if first => parsed.command = Command::Sweep,
            "--data" => {
                let dir = args
                    .next()
                    .ok_or_else(|| AppError::Usage("--data needs a directory".into()))?;
                parsed.data_dir = PathBuf::from(dir);
            }
            "--check" => {
                let text = args
                    .next()
                    .ok_or_else(|| AppError::Usage("--check needs a page size".into()))?;
                let p = text
                    .parse::<u64>()
                    .ok()
                    .and_then(NonZeroU64::new)
                    .ok_or_else(|| {
                        AppError::Usage(format!("--check {text:?}: need a positive whole number"))
                    })?;
                parsed.checks.push(p);
            }
            other => return Err(AppError::Usage(format!("unexpected argument {other:?}"))),
        }
        first = false;
    }
    Ok(parsed)
}

fn run() -> Result<(), AppError> {
    let args = parse_args(std::env::args().skip(1))?;
    // `?` converts LoadError into AppError through the From impl.
    let loaded = workload::load_workloads(&args.data_dir)?;
    for w in &loaded.warnings {
        eprintln!("WARNING: {w}");
    }
    match args.command {
        Command::Summary => {
            summary(&args.data_dir, &loaded);
            Ok(())
        }
        Command::Sweep => run_sweep(&loaded, &args.checks),
    }
}

fn summary(dir: &Path, loaded: &Workloads) {
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
}

fn run_sweep(loaded: &Workloads, checks: &[NonZeroU64]) -> Result<(), AppError> {
    let requests: Vec<u64> = loaded.all_requests().collect();
    let days = loaded.days.len();
    let n = requests.len() as u64;
    let total_requested: u64 = requests.iter().sum();
    let max_request = requests
        .iter()
        .copied()
        .max()
        .and_then(NonZeroU64::new)
        .ok_or_else(|| AppError::Usage("no programs loaded, nothing to sweep".into()))?;
    // If the OS cannot tell us the core count, one thread is still correct.
    let threads = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);

    println!(
        "{} programs over {days} days, {} bits requested",
        commas(n),
        commas(total_requested)
    );
    println!(
        "sweeping P = 1..={} on {threads} threads",
        commas(max_request.get())
    );
    let start = Instant::now();
    let totals = sweep::sweep(&requests, max_request, threads);
    let elapsed = start.elapsed();
    println!(
        "sweep time: {:.2} s ({} cost evaluations)\n",
        elapsed.as_secs_f64(),
        commas(max_request.get() * n)
    );

    let Some((best, winners)) = sweep::winners(&totals) else {
        return Err(AppError::Usage("sweep produced no results".into()));
    };
    let row = |p: u64, t: u64| {
        let gap = t - best;
        let rank = sweep::rank_of(&totals, p).map_or("?".into(), |r| commas(r as u64));
        println!(
            "  {:>12}  {:>16}  {:>18}  {:>14}  {:>10}",
            commas(p),
            commas(t),
            mean_text(t, days),
            commas(gap),
            rank
        );
    };
    let header = |title: &str| {
        println!("{title}");
        println!(
            "  {:>12}  {:>16}  {:>18}  {:>14}  {:>10}",
            "page bits", "total bits", "mean per day", "over best", "rank"
        );
    };

    println!("BEST: {} page size(s) tie for the minimum", winners.len());
    header("");
    for &p in &winners {
        row(p, best);
    }

    header("\nTop 10");
    for (p, t) in sweep::top_k(&totals, 10) {
        row(p, t);
    }

    for &w in &winners {
        header(&format!("\nWindow: {} +/- 10", commas(w)));
        for p in w.saturating_sub(10).max(1)..=w + 10 {
            if let Some(t) = total_at(&totals, p) {
                row(p, t);
            }
        }
    }

    header("\nPowers of two");
    for e in 10..=16 {
        let p = 1u64 << e;
        if let Some(t) = total_at(&totals, p) {
            row(p, t);
        }
    }

    // Continuous estimate P* = sqrt(128 R / n), from Phase 2.
    let estimate = (128.0 * total_requested as f64 / n as f64).sqrt();
    header(&format!(
        "\nReference points (continuous estimate {estimate:.2})"
    ));
    let mut refs = vec![estimate.floor() as u64, estimate.ceil() as u64];
    refs.extend(checks.iter().map(|p| p.get()));
    refs.dedup();
    for p in refs {
        match total_at(&totals, p) {
            Some(t) => row(p, t),
            None => println!("  {:>12}  outside the sweep range", commas(p)),
        }
    }

    // Bound cross checks.
    println!("\nBound checks");
    println!(
        "  locked bound: every P >= {} gives one page per program, so the total grows by {} per extra bit. Swept through it.",
        commas(max_request.get()),
        commas(n)
    );
    let cutoff = sweep::lower_bound_cutoff(n, total_requested, best);
    println!(
        "  tighter bound: floor (P + 64) * n - R exceeds the best total for every P >= {}",
        commas(cutoff)
    );
    // Consistency check on the sweep itself: no total may fall below the floor.
    let floor_violations = totals
        .iter()
        .enumerate()
        .filter(|&(i, &t)| {
            let p = sweep::page_size_at(i);
            ((p + 64) * n).saturating_sub(total_requested) > t
        })
        .count();
    println!("  sizes whose total is below the floor (must be 0): {floor_violations}");
    let beyond_cutoff_wins = winners.iter().filter(|&&p| p >= cutoff).count();
    println!("  winners at or past the tighter cutoff (must be 0): {beyond_cutoff_wins}");

    let dense = Path::new("results/sweep.csv");
    let tail = Path::new("results/sweep_tail.csv");
    report::write_sweep_csv(dense, &totals, DENSE_CSV_LAST, days)?;
    let blocks = sweep::block_minima(&totals, DENSE_CSV_LAST + 1, TAIL_BLOCK);
    report::write_tail_csv(tail, &blocks, days)?;
    println!(
        "\nwrote {} (P = 1..={}) and {} ({} blocks of {TAIL_BLOCK})",
        dense.display(),
        commas(DENSE_CSV_LAST.min(max_request.get())),
        tail.display(),
        blocks.len()
    );
    Ok(())
}

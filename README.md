# How Big Should a Page Be?

## Answer

**Use a page size of 32,768 bits (2^15).**

| | bits |
|---|---|
| Overall score (mean of the 32 daily scores) | 3,173,294.96875 |
| Total penalty over all 32 days | 101,545,439 |
| Bookkeeping part of that total | 67,239,936 |
| Unused space part of that total | 34,305,503 |

It is the unique minimum: no other page size ties it. The runner up is 32,769 bits, which is 1,050,624 bits worse in total.

## How to run

Needs Rust 1.85 or newer (edition 2024). Standard library only, no crates to download.

The repo already contains the data in `data/workloads/`. To use a fresh copy of the downloaded archive instead:

```sh
unzip page-size-workloads.zip -d data      # gives data/workloads/day_01/loads ... day_32/loads
```

Then:

```sh
cargo test                          # 35 unit tests + 1 test on the real dataset
cargo run --release                 # data check: days, programs, min/max/mean request
cargo run --release -- sweep        # full sweep, about 18 s on a 10 core Apple M4
cargo run --release -- report       # per day table, independent check, exploration, about 38 s
```

Options for every command:

- `--data DIR` reads workloads from another folder (default `data/workloads`).
- `--check P` (sweep only, repeatable) also prints the score and rank of page size P.

Use `--release`. A debug build is about 13 times slower here (the full sweep would take an estimated 4 minutes on 10 cores, or about 21 minutes on one), because it does not inline the cost function or keep values in registers.

### Output files

| File | Contents |
|---|---|
| `results/sweep.csv` | Every page size from 1 to 131,072: `page_size_bits, total_penalty_bits, mean_penalty_bits` |
| `results/sweep_tail.csv` | Page sizes 131,073 to 25,165,824 in blocks of 4,096, with the best size and score in each block |
| `results/per_day.csv` | Each day at 32,768 bits: programs, pages, bookkeeping bits, unused bits, total penalty |
| `results/exploration_per_day_best.csv` | Exploration only: the size each day would pick on its own |

A CSV of all 25,165,824 sizes would be about 1 GB and too long for a spreadsheet, so the dense file stops at 131,072 (four times the winner). The program still scores every size; the tail file shows that nothing past 131,072 comes close.

## The model

For page size P bits, a program requesting r bits gets ceil(r / P) whole pages. Each page costs one 64 bit bookkeeping record, and each unused bit inside a program's pages costs 1 bit:

```
penalty(r, P) = 64 * ceil(r / P) + (ceil(r / P) * P - r)
```

A day's score is the sum over its programs. The overall score is the mean of the 32 daily scores. Everything is measured in bits.

## Search method

1. **Load and validate** all 32 files (4,096 programs). Bad data stops the run with the file and line number; wrong counts produce a warning.
2. **Score every page size from 1 to 25,165,824** (the largest request). That is about 103 billion program evaluations, split across all CPU cores with `std::thread::scope`.
3. **Rank by the exact integer total** over all 32 days. The overall score is total / 32, so ranking by total gives exactly the same order as ranking by the mean, without comparing floating point numbers. The mean is only computed for display (exact in 5 decimals, since 1/32 = 0.03125).
4. **Report every page size tied for the minimum.** There is exactly one.

All arithmetic is `u64`. The largest possible total (at P = 1) is about 2.2 * 10^12, far below the `u64` limit of 1.8 * 10^19. The release build keeps overflow checks on anyway, so an arithmetic bug would stop the program instead of silently wrapping around.

## Why the sweep can stop at the largest request

Let M = 25,165,824 bits, the biggest request in the data.

Take any page size P that is at least M. Every program then fits in a single page, so each program's penalty is 64 + (P - r): one record plus the empty part of its one page. Make the page one bit bigger and every program gets exactly one more unused bit, with nothing else changing. So above M, the total goes up by exactly 4,096 (one bit per program) for every extra bit of page size.

That means M beats every larger size, strictly, so no larger size can win or tie. Checking 1 through M therefore covers every page size that could possibly be the answer.

**A tighter bound (used only as a cross check).** Every program needs at least one page, so at any page size P the total is at least (P + 64) * n - R, where n = 4,096 programs and R = 34,392,541,729 requested bits. Once that floor is above the best total found, no larger P can win. Here that happens at P = 8,421,345, so about two thirds of the sweep could never have changed the answer. The program checks that every swept total is at or above this floor (0 violations).

## Why 32,768, and why the curve is jagged

The total can be rewritten exactly as

```
T(P) = (64 + P) * N(P) - R
```

where N(P) is the total number of pages. As long as N stays the same, each extra bit of page size adds one unused bit to every page, so T climbs by N per bit. When P crosses a value where some program fits in one fewer page, N drops and T falls off a cliff. The curve is a sawtooth, and the best size always sits at the bottom of a drop.

The data has two clear groups:

- 2,048 small requests, the largest of which is exactly 32,768 bits.
- 2,048 large requests, every one an exact multiple of 65,536 bits.

At 32,768 bits every small program fits in one page, and every large program fills its pages exactly with zero unused bits. Going down one bit to 32,767 forces all 2,064 requests that are multiples of 32,768 to take one extra page, a net cost of 66,712,560 bits. Halving to 16,384 doubles the records on the large programs (total 151,186,015). Doubling to 65,536 makes every small program waste at least half a page (total 135,099,871).

Selected scores (full list in `results/sweep.csv`):

| Page size (bits) | Total penalty | Rank |
|---|---|---|
| 1,024 | 2,150,627,807 | 706,808 |
| 16,384 | 151,186,015 | 22,400 |
| 32,767 | 168,257,999 | 35,140 |
| **32,768** | **101,545,439** | **1** |
| 32,769 | 102,596,063 | 2 |
| 32,783 | 117,304,799 | 16 |
| 65,536 | 135,099,871 | 390 |

**Continuous approximation.** Treating pages as r / P and unused space as P / 2 per program gives P = sqrt(128 * mean request) = 32,783.64 bits. That lands within 16 bits of the true answer, but 32,783 itself ranks 16th and is 15.5% worse, because on a sawtooth being close in page size does not mean being close in score. It also overestimates the best score by 32%, since it assumes every program wastes half a page while here half of them waste nothing.

## Verification

- **Unit tests** check the cost function against hand computed cases (for example r = 23 at P = 10, 6 and 15 give 199, 257 and 135), boundaries (P = 1, P at least r, exact multiples), and the definition of ceiling for every r and P from 1 to 300.
- **The threaded sweep** is tested against a plain loop with 1, 2, 3, 7, 16 and 300 threads, since chunk boundaries are where off by one errors hide.
- **Independent implementation** (`src/naive.rs`): hands out pages one at a time until a request fits, with no division, ceiling or multiplication. At 16 page sizes (the winner, its neighbours, powers of two, a prime, the largest request, and others) it matches the fast version on every day, for pages, bookkeeping, unused bits and penalty separately, and matches the sweep total.
- **Per day totals** at 32,768 add up to exactly the sweep total.
- **A third implementation** (a short awk script in a different language, with its own file reading) produced the same totals at 31,256, 32,783 and 32,784.

## Error handling

The assignment module is error handling, so the program never calls `unwrap`, `expect` or `panic!` outside tests. Errors travel up with `Result` and `?`.

`LoadError` (in `src/error.rs`) has one variant per way the data can be wrong, each carrying the file path and, where it applies, the line number:

| Variant | Example message |
|---|---|
| `MissingFile` | `data/workloads/day_03/loads: file or directory not found` |
| `Io` | any other read failure, with the operating system's reason |
| `NoDays` | `data/workloads: no day_NN folders found` |
| `EmptyLine` | `data/workloads/day_01/loads:2: empty line` |
| `NotAnInteger` | `data/workloads/day_01/loads:1: "12.5" is not a whole number of bits` |
| `NotPositive` | `data/workloads/day_02/loads:2: request must be positive, got -3` |

An enum was chosen over `Box<dyn Error>` so tests can check exactly which error came back, and so every variant is forced to carry a location.

`AppError` wraps `LoadError` together with output file errors, command line mistakes and failed cross checks. Its `From<LoadError>` impl is what lets `?` convert a load error automatically. `main` prints the readable message and exits with status 1.

Wrong counts (not 32 days, or not 128 programs in a day) print a `WARNING:` line and the run continues. Day folders are sorted before loading, so with several bad files the reported error is always the lowest numbered day, regardless of the order the operating system lists them.

The page size type is `NonZeroU64`, so a page size of 0 (which would divide by zero) cannot be passed to the cost function at all.

## What is established and what is only tested

**Established for this dataset and this model:**

- 32,768 bits is the unique best page size among all positive whole numbers. Sizes 1 through 25,165,824 were each scored exactly, and the bound argument above rules out every larger size.

**Tested, not proven:**

- That the code computes the model correctly. The evidence is the hand computed unit tests, the independent naive implementation agreeing at 16 sizes, and the awk check at 3 sizes. The naive check covers 16 of the 25 million sizes, not all of them.

**Not established:**

- **Other workloads.** The answer depends heavily on how this data is built: the largest small request is exactly 32,768 and every large request is a multiple of 65,536. A single small request of 32,769 bits, or large requests that are not multiples of 32,768, could move the answer. These 32 days are a sample, not a guarantee about future days.
- **Day to day stability.** As an exploration, 10 of the 32 days would choose a different size on their own (8 prefer 65,536, 2 prefer 21,846). Letting each day choose would save only 2.5% overall, but the machine uses one size for all days, so this is not the answer.
- **Other objectives.** The goal is the mean daily score. Minimizing the worst day, or weighting days differently, was not tested and could pick a different size.
- **Real hardware.** The model leaves out everything a real operating system cares about (alignment, address translation, records that grow with memory size, programs that grow over time). That 32,768 is a power of two comes from this data, not from anything in the model.

## Project layout

| Path | Purpose |
|---|---|
| `src/model.rs` | Cost of one program at one page size, plus totals |
| `src/workload.rs` | Loading and validating the day files |
| `src/error.rs` | `LoadError` and `AppError` |
| `src/sweep.rs` | Threaded sweep, winners, top 10, ranks, bounds |
| `src/naive.rs` | Independent page at a time implementation |
| `src/report.rs` | CSV output and number formatting |
| `src/main.rs` | Command line: `summary`, `sweep`, `report` |
| `tests/real_data.rs` | Checks the real dataset's counts and totals |
| `NOTES.md` | Running log of predictions, results, surprises and bugs |

# NOTES (raw running log)

## Phase 0: setup and data check (2026-10-21)

- zip found at ~/Downloads/page-size-workloads.zip. Same bytes as the copy already sitting in the repo root (checked with cmp). Extracted to data/.
- data/README.txt confirms: 32 snapshots, 128 positive integers per file, duplicates count, 4,096 total.
- Results: 32 days, 128 programs every day, 4,096 total. Total requested 34,392,541,729 bits. Min 29, max 25,165,824, mean 8,396,616.63 bits.
- WRONG ASSUMPTION in my prompt: I expected day_01 to start 132294, 24302, 313965. Real values are 32294, 4302, 13965. I had copied them with the editor line numbers (1, 2, 3) glued on the front.
- Surprise: max request is 25,165,824 bits (exactly 3 * 2^23). Way bigger than I'd guessed. Means the full sweep 1..=max is about 25M page sizes * 4,096 programs, roughly 10^11 divisions. Runtime may actually matter.
- Surprise: half the programs (2,048) are over 1,000,000 bits. Requests span 29 bits to 25M bits, so this is a very wide spread.
- Bound discussion: my bound (nothing above max_request can win) is correct. It can be tightened: every program needs at least 1 page, so T(P) >= (P + 64) * n - R. Once that floor is above the best total seen, no bigger P can win. Keeping the full sweep anyway (locked), using this as a cross-check.
- BUG hit: first loader version loaded each day as read_dir returned it. read_dir order is not guaranteed, so with several bad files the reported error depended on directory order (test reported day_02 when day_01 was also bad). Fix: collect day folders, sort by number, then load. Now the lowest bad day is always the one reported.
- Error enum LoadError: MissingFile, Io, NoDays, EmptyLine, NotAnInteger, NotPositive. Checked each by hand with scratch files, all report path:line. Parse as i64 first so "-3" becomes NotPositive instead of NotAnInteger.
- Known limit: a 20 digit number gets NotAnInteger ("not a whole number of bits"), which is technically true for 64 bit integers but the message doesn't say "too big".
- Gotcha for Phase 3: a full sweep.csv would be 25M rows, about 1 GB, past Excel's row limit.

## Phase 1: cost function and tests (2026-10-21)

- program_cost(request, page_size) in src/model.rs returns a Cost struct (pages, bookkeeping_bits, unused_bits, penalty). Struct not tuple: all four are u64, a tuple would let me swap two fields with no compile error.
- page_size is NonZeroU64, not u64. P = 0 would divide by zero. Type makes it impossible, no runtime check in the hot loop, no Result for something that's a code bug not a data problem.
- Ceiling with u64::div_ceil, not (r + P - 1) / P (can overflow near u64::MAX) and not f64 ceil (floats are the wrong tool for exact integer answers).
- All hand checked cases from the prompt passed first try: r=23 P=10 -> 199, P=6 -> 257, P=15 -> 135.
- Extra cases I added: one bit past a page boundary (r=11, P=10 costs 137 vs 64 for r=10, so one extra bit costs 73), and the dataset max 25,165,824 = 24 * 2^20, so P = 2^20 gives exactly 24 pages, 0 unused.
- Invariant test over every r, P in 1..=300 that checks "ceiling" by its definition (pages * P >= r and (pages - 1) * P < r) without calling div_ceil.
- Error tests: blank line (middle, whitespace only, extra at end), 12.5 / abc / 1e6 / 20 digit number, 0, -3, missing loads file, missing directory, folder with no days, wrong counts warn, lowest bad day reported first (regression test for the Phase 0 bug).
- Real data integration test pins the Phase 0 numbers so a bad re-extract gets caught.
- Result: 23 tests pass, clippy clean.
- Mutation check: swapped div_ceil for plain / (floor). 6 model tests failed, so the tests do catch it. The 3 that still passed were the exactly divisible ones, which makes sense since floor = ceil there.
- SURPRISE from the mutation check: the crash was "attempt to subtract with overflow" in pages * P - r. Debug builds panic on overflow, RELEASE BUILDS WRAP SILENTLY. Phase 3 runs in release, so a bug like that would give a giant wrong number instead of a crash. Need to decide about overflow-checks in release.

## Phase 2: predictions, before any sweep (2026-10-21)

Continuous approximation (computed with a quick awk script over data/, not the sweep):
- Assume pages ~ r/P (ignore rounding up), so bookkeeping ~ 64 * r / P per program, 64R/P total.
- Assume the last page is randomly filled, so unused ~ P/2 per program, nP/2 total.
- f(P) = 64R/P + nP/2. Small P: records blow up. Big P: waste grows. Set derivative to 0: 64R/P^2 = n/2, so P* = sqrt(128 * R / n) = sqrt(128 * mean request).
- R = 34,392,541,729, n = 4,096, mean 8,396,616.63. P* = 32,783.64 bits.
- Approx total at P*: 134,281,782 (mean per day 4,196,306).

Exact scores at a few hand picked sizes (single evaluations, NOT the sweep):
- P = 32,783: total 117,304,799, mean per day 3,665,774.97.
- P = 32,784: total 118,355,423, mean per day 3,698,606.97.
- The approximation came out 12% too high near P*.

Data structure found while checking the approximation's assumptions:
- Exactly 2,048 requests are below 32,784 bits, and 2,048 requests are exact multiples of 65,536 (2^16). The dataset is two populations, not one smooth spread.
- That breaks the "unused ~ P/2" assumption. For the big half, unused space is exactly 0 for any P that divides 65,536, and very non random otherwise.
- Adjacent sizes 32,783 and 32,784 already differ by over 1,000,000 bits, so the curve is not smooth.

My prediction, verbatim (given AFTER seeing the continuous estimate and the data structure, so it is not independent of them):
> then put my best size as 31256

- Single exact evaluation of P = 31,256 (still not the sweep): total 136,693,391, mean per day 4,271,668.47. Bookkeeping 70,557,824, unused 66,135,567.
- That is worse than P = 32,783 (117,304,799). Two reasons: 65,536 / 31,256 = 2.097, so a 65,536 bit program needs 3 pages instead of 2 and wastes 28,232 bits. Also 116 of the small programs are between 31,256 and 32,784 bits, so they spill into a second page.

## Phase 3: exhaustive sweep (2026-10-21)

Setup decisions:
- Threads: std::thread::scope + chunks_mut, one contiguous block of page sizes per core. Still std only, no rayon. Borrow checker proves threads write disjoint slices, no locks.
- overflow-checks = true in [profile.release] so an arithmetic bug crashes instead of wrapping.
- CSV: results/sweep.csv has every P from 1 to 131,072 (4 MB). results/sweep_tail.csv has the best P in each block of 4,096 after that (6,112 blocks). The program itself still scores all 25,165,824 sizes in memory (about 200 MB of u64).
- New AppError enum (Load, Write, Usage) with From<LoadError>, so ? lifts load errors automatically.

Runtime (Apple M4, 10 cores, 103,079,215,104 cost evaluations):
- release, 10 threads, overflow checks ON: 17.4 s (rerun 17.5 s)
- release, 10 threads, overflow checks OFF: 11.7 s. Checks cost about 50% here. Kept them on, same answer either way.
- 1 thread, measured on P = 1..=100,000 and scaled up: release about 97 s, debug about 1,243 s (21 min). Debug is about 13x slower: no inlining of program_cost, iterator chains not fused, values not kept in registers.
- 10 threads gave about 5.5x, not 10x. M4 is 4 performance + 6 efficiency cores.

RESULT: best page size 32,768 bits (2^15). Unique, no ties.
- Total 101,545,439 bits. Mean per day 3,173,294.96875 bits.
- Top 10 is just 32,768 through 32,777, each one 1,050,624 worse than the last.
- 32,767 (one bit less): 168,257,999. That's 66,712,560 worse, rank 35,140.
- Powers of two: 16,384 -> 151,186,015 (rank 22,400). 65,536 -> 135,099,871 (rank 390). 1,024 -> 2,150,627,807.

Why the curve is jagged (ceiling effect):
- Exact identity: T(P) = (64 + P) * N(P) - R, where N(P) is total pages.
- While N stays the same, every extra bit of P adds 1 unused bit to every page, so T rises by N per bit. At 32,768, N = 1,050,624, which is exactly the step in the top 10.
- When P crosses a point where a program fits in one fewer page, N drops and T falls off a cliff. So the curve is a sawtooth: slow climbs, sudden drops. The winner has to sit at the bottom of a drop.
- 32,767 vs 32,768: all 2,064 requests that are multiples of 32,768 need one extra page at 32,767. Change = 32,831 * 2,064 - 1,050,624 = 66,712,560. Matches the sweep exactly.

Why 2^15 specifically:
- Largest "small" request is exactly 32,768 (three of them). So at P = 32,768 every small program fits in one page.
- Every big request is a multiple of 65,536, so at P = 32,768 the big programs waste 0 bits.
- Go smaller (16,384): twice the records on the big programs. Go bigger (65,536): every small program wastes up to 65,536. 32,768 is the one size that gets both groups right.

Comparison to predictions:
- My prediction 31,256: rank 2,040, total 136,693,391, 34.6% worse than the best. 65,536 doesn't divide into it, so each 65,536 bit program needs 3 pages, and 116 small programs spill into a second page.
- Continuous estimate 32,783.64: within 16 bits of the winner (0.05%), but 32,783 ranks 16th and is 15.5% worse. 15 bits too far right times about 1.05M per bit = 15.76M. Close in P is not close in score on a sawtooth. Its predicted best score (134.3M) was 32% too high because it assumed P/2 waste on every program, but at 2^15 half the programs waste nothing.

Bound checks:
- Locked bound covers through 25,165,824.
- Tighter bound: (P + 64) * n - R is already above the best total for every P >= 8,421,345. So two thirds of the sweep provably couldn't win. Used only as a check.
- Every swept total is at or above that floor (0 violations), a consistency check on the sweep itself.

Small things:
- My shell test loop passed "sweep --check 0" as ONE argument (zsh does not word split $var). Looked like a CLI bug, was a test harness bug.
- Every mean ends in .96875 or similar fractions of 1/32 because the mean is total / 32, which is exact in 5 decimals.

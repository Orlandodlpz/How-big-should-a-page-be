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

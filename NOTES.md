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

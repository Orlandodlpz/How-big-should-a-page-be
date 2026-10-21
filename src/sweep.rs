//! Exhaustive sweep over page sizes, plus the summaries read off its result.
//!
//! Ranking uses the exact u64 total over all programs. The overall score is
//! the mean of the daily scores, which is total / number_of_days, so ordering
//! by total gives the same order as ordering by mean, without floats.

use std::num::{NonZeroU64, NonZeroUsize};
use std::thread;

use crate::model::program_cost;

/// Total penalty over all `requests` at page size `p`.
pub fn total_penalty(requests: &[u64], p: NonZeroU64) -> u64 {
    requests.iter().map(|&r| program_cost(r, p).penalty).sum()
}

/// Every page size from 1 to `max_page_size`, scored. Returns a Vec where
/// index i holds the total for page size i + 1.
///
/// The range is cut into one contiguous chunk per thread. chunks_mut hands
/// each thread its own disjoint &mut slice of the output, so the borrow
/// checker proves no two threads write the same slot (no locks needed).
/// thread::scope lets the threads borrow `requests` directly, and joins them
/// all before returning. If any thread panics (e.g. an overflow check), the
/// panic is re-raised here instead of being lost.
pub fn sweep(requests: &[u64], max_page_size: NonZeroU64, threads: NonZeroUsize) -> Vec<u64> {
    // u64 to usize is lossless on the 64 bit targets this runs on.
    let len = max_page_size.get() as usize;
    let mut totals = vec![0u64; len];
    let chunk = len.div_ceil(threads.get());

    thread::scope(|s| {
        for (i, out) in totals.chunks_mut(chunk).enumerate() {
            s.spawn(move || {
                let first_offset = (i * chunk) as u64;
                for (j, slot) in out.iter_mut().enumerate() {
                    // MIN is 1, so this is page size first_offset + j + 1,
                    // built as a NonZeroU64 with no Option to unwrap.
                    let p = NonZeroU64::MIN.saturating_add(first_offset + j as u64);
                    *slot = total_penalty(requests, p);
                }
            });
        }
    });
    totals
}

/// Page size for a 0 based index into the sweep result.
pub fn page_size_at(index: usize) -> u64 {
    index as u64 + 1
}

/// Total for page size `p` (1 based) from the sweep result.
pub fn total_at(totals: &[u64], p: u64) -> Option<u64> {
    let index = usize::try_from(p.checked_sub(1)?).ok()?;
    totals.get(index).copied()
}

/// The minimum total and EVERY page size that reaches it, in ascending order.
pub fn winners(totals: &[u64]) -> Option<(u64, Vec<u64>)> {
    let best = *totals.iter().min()?;
    let sizes = totals
        .iter()
        .enumerate()
        .filter(|&(_, &t)| t == best)
        .map(|(i, _)| page_size_at(i))
        .collect();
    Some((best, sizes))
}

/// The k best (page_size, total) pairs, best first. Ties are broken by the
/// smaller page size so the output is deterministic.
pub fn top_k(totals: &[u64], k: usize) -> Vec<(u64, u64)> {
    let mut top: Vec<(u64, u64)> = Vec::with_capacity(k + 1);
    for (i, &t) in totals.iter().enumerate() {
        if top.len() == k && top.last().is_some_and(|&(_, worst)| t >= worst) {
            continue;
        }
        // Insert after any equal totals: those have smaller page sizes.
        let pos = top.partition_point(|&(_, other)| other <= t);
        top.insert(pos, (page_size_at(i), t));
        top.truncate(k);
    }
    top
}

/// 1 based rank of page size `p`: one plus how many sizes score strictly
/// better. Tied sizes share a rank.
pub fn rank_of(totals: &[u64], p: u64) -> Option<usize> {
    let t = total_at(totals, p)?;
    Some(1 + totals.iter().filter(|&&other| other < t).count())
}

/// Best page size inside each block of `block` consecutive sizes, for every
/// size from `start` to the end. Returns (block_first, block_last,
/// best_size, best_total).
pub fn block_minima(totals: &[u64], start: u64, block: usize) -> Vec<(u64, u64, u64, u64)> {
    let Some(from) = start.checked_sub(1).and_then(|s| usize::try_from(s).ok()) else {
        return Vec::new();
    };
    let Some(rest) = totals.get(from..) else {
        return Vec::new();
    };
    rest.chunks(block)
        .enumerate()
        .filter_map(|(b, chunk)| {
            let base = from + b * block;
            let (offset, &best) = chunk.iter().enumerate().min_by_key(|&(j, &t)| (t, j))?;
            Some((
                page_size_at(base),
                page_size_at(base + chunk.len() - 1),
                page_size_at(base + offset),
                best,
            ))
        })
        .collect()
}

/// Smallest page size P at which the floor (P + 64) * n - R is already above
/// `best`. Every program needs at least one page, so at any P the total is at
/// least (P + 64) * n - R. Past this size nothing can beat `best`.
pub fn lower_bound_cutoff(n: u64, total_requested: u64, best: u64) -> u64 {
    // Solve (P + 64) * n > best + R for the smallest whole P.
    // P + 64 > (best + R) / n, so P = floor((best + R) / n) + 1 - 64, but
    // never below 1 when best is tiny.
    ((best + total_requested) / n + 1).saturating_sub(64).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nz(n: u64) -> NonZeroU64 {
        NonZeroU64::new(n).unwrap()
    }

    fn threads(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    #[test]
    fn total_is_sum_of_program_penalties() {
        // From the Phase 1 cases at P = 10: r=23 -> 199, r=30 -> 192, r=11 -> 137.
        assert_eq!(total_penalty(&[23, 30, 11], nz(10)), 199 + 192 + 137);
    }

    #[test]
    fn sweep_matches_direct_evaluation_for_any_thread_count() {
        // Chunk boundaries are a classic off by one spot. Compare the threaded
        // sweep to a plain loop with thread counts that do and do not divide
        // the range evenly, including more threads than page sizes.
        let requests = [23, 30, 11, 1, 97, 64, 65, 200];
        let max = 250;
        let expected: Vec<u64> = (1..=max).map(|p| total_penalty(&requests, nz(p))).collect();
        for t in [1, 2, 3, 7, 16, 300] {
            assert_eq!(
                sweep(&requests, nz(max), threads(t)),
                expected,
                "threads={t}"
            );
        }
    }

    #[test]
    fn winners_reports_every_tie() {
        let totals = [9, 4, 7, 4, 4, 8];
        assert_eq!(winners(&totals), Some((4, vec![2, 4, 5])));
        assert_eq!(winners(&[]), None);
    }

    #[test]
    fn top_k_orders_by_total_then_size() {
        let totals = [9, 4, 7, 4, 1, 8];
        // sizes:     1  2  3  4  5  6
        assert_eq!(top_k(&totals, 3), vec![(5, 1), (2, 4), (4, 4)]);
        assert_eq!(top_k(&totals, 10).len(), 6);
    }

    #[test]
    fn rank_counts_strictly_better_sizes() {
        let totals = [9, 4, 7, 4, 1, 8];
        assert_eq!(rank_of(&totals, 5), Some(1));
        assert_eq!(rank_of(&totals, 2), Some(2));
        assert_eq!(rank_of(&totals, 4), Some(2)); // tie shares the rank
        assert_eq!(rank_of(&totals, 3), Some(4));
        assert_eq!(rank_of(&totals, 0), None);
        assert_eq!(rank_of(&totals, 7), None);
    }

    #[test]
    fn block_minima_covers_the_tail() {
        let totals = [9, 4, 7, 4, 1, 8, 3];
        // From size 3 in blocks of 2: [3,4] -> (7,4) best size 4; [5,6] -> (1,8)
        // best size 5; [7] -> (3) best size 7.
        assert_eq!(
            block_minima(&totals, 3, 2),
            vec![(3, 4, 4, 4), (5, 6, 5, 1), (7, 7, 7, 3)]
        );
    }

    #[test]
    fn cutoff_is_first_size_whose_floor_beats_best() {
        // n = 2 programs, R = 100 bits, best = 50.
        // Floor at P: (P + 64) * 2 - 100. P = 12 -> 52 > 50, P = 11 -> 50, not above.
        assert_eq!(lower_bound_cutoff(2, 100, 50), 12);
    }
}

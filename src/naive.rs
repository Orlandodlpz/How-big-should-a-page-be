//! A deliberately naive second implementation, used only as a cross check.
//!
//! It shares no code or formula with model.rs: no division, no ceiling, no
//! multiplication. It hands out pages one at a time until the request fits,
//! which is the machine description read literally. That makes it slow
//! (one loop step per page), and that is the point: if two very different
//! methods agree exactly, a shared mistake is unlikely.

use std::num::NonZeroU64;

/// Pages, bookkeeping bits and unused bits, summed over a group of programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NaiveTotals {
    pub pages: u64,
    pub bookkeeping_bits: u64,
    pub unused_bits: u64,
}

impl NaiveTotals {
    pub fn penalty(&self) -> u64 {
        self.bookkeeping_bits + self.unused_bits
    }
}

/// One program, one page at a time. NonZeroU64 keeps P = 0 out, which would
/// otherwise loop forever.
pub fn naive_program(request: u64, page_size: NonZeroU64) -> NaiveTotals {
    let mut out = NaiveTotals::default();
    let mut capacity = 0;
    while capacity < request {
        out.pages += 1;
        out.bookkeeping_bits += 64;
        capacity += page_size.get();
    }
    out.unused_bits = capacity - request;
    out
}

/// A whole day (or any group), accumulated with a plain loop.
pub fn naive_group(requests: &[u64], page_size: NonZeroU64) -> NaiveTotals {
    let mut total = NaiveTotals::default();
    for &r in requests {
        let one = naive_program(r, page_size);
        total.pages += one.pages;
        total.bookkeeping_bits += one.bookkeeping_bits;
        total.unused_bits += one.unused_bits;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::program_cost;

    fn p(n: u64) -> NonZeroU64 {
        NonZeroU64::new(n).unwrap()
    }

    #[test]
    fn hand_checked_cases() {
        // Same hand computed cases as model.rs: 199, 257, 135.
        assert_eq!(naive_program(23, p(10)).penalty(), 199);
        assert_eq!(naive_program(23, p(6)).penalty(), 257);
        assert_eq!(naive_program(23, p(15)).penalty(), 135);
    }

    #[test]
    fn agrees_with_model_on_small_grid() {
        for r in 1..=300 {
            for page in 1..=300 {
                let n = naive_program(r, p(page));
                let m = program_cost(r, p(page));
                assert_eq!(
                    (n.pages, n.bookkeeping_bits, n.unused_bits, n.penalty()),
                    (m.pages, m.bookkeeping_bits, m.unused_bits, m.penalty),
                    "r={r} P={page}"
                );
            }
        }
    }

    #[test]
    fn group_sums_programs() {
        // P = 10: pages 3 + 3 + 2 = 8, unused 7 + 0 + 9 = 16, penalty 528.
        let g = naive_group(&[23, 30, 11], p(10));
        assert_eq!((g.pages, g.unused_bits, g.penalty()), (8, 16, 528));
    }
}

//! The machine model: what one program costs at one page size.
//!
//! All integer math, all in bits. No floats: integer div_ceil is exact by
//! construction, so there is no rounding argument to make.

use std::num::NonZeroU64;

/// Each allocated page needs one bookkeeping record of this many bits.
pub const RECORD_BITS: u64 = 64;

/// Cost breakdown for one program at one page size. A struct instead of a
/// 4-tuple because all four fields are u64, so a tuple would let two of them
/// be swapped without any compile error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    pub pages: u64,
    pub bookkeeping_bits: u64,
    pub unused_bits: u64,
    pub penalty: u64,
}

/// Cost of one program requesting `request` bits when pages are `page_size`
/// bits. Pure: same inputs, same output, no I/O.
///
/// `page_size` is NonZeroU64 because P = 0 would divide by zero. The caller
/// proves P > 0 once (NonZeroU64::new returns None for 0), so this function
/// needs neither a runtime check nor a Result for what would be a programming
/// bug rather than bad data.
pub fn program_cost(request: u64, page_size: NonZeroU64) -> Cost {
    let p = page_size.get();
    // Exact integer ceiling. The classic (r + p - 1) / p gives the same answer
    // but can overflow when r is close to u64::MAX.
    let pages = request.div_ceil(p);
    let bookkeeping_bits = RECORD_BITS * pages;
    // pages * p >= request by definition of the ceiling, so no underflow.
    let unused_bits = pages * p - request;
    Cost {
        pages,
        bookkeeping_bits,
        unused_bits,
        penalty: bookkeeping_bits + unused_bits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper. unwrap is fine in tests: a panic is just a failed test.
    fn p(n: u64) -> NonZeroU64 {
        NonZeroU64::new(n).unwrap()
    }

    fn cost(pages: u64, bookkeeping_bits: u64, unused_bits: u64, penalty: u64) -> Cost {
        Cost {
            pages,
            bookkeeping_bits,
            unused_bits,
            penalty,
        }
    }

    #[test]
    fn r23_p10() {
        // 23 / 10 = 2.3, ceiling 3 pages. 3 * 64 = 192 bookkeeping.
        // 3 pages hold 30 bits, 30 - 23 = 7 unused. 192 + 7 = 199.
        assert_eq!(program_cost(23, p(10)), cost(3, 192, 7, 199));
    }

    #[test]
    fn r23_p6() {
        // 23 / 6 = 3.83, ceiling 4 pages. 4 * 64 = 256.
        // 4 * 6 = 24 bits, 24 - 23 = 1 unused. 256 + 1 = 257.
        assert_eq!(program_cost(23, p(6)), cost(4, 256, 1, 257));
    }

    #[test]
    fn r23_p15() {
        // 23 / 15 = 1.53, ceiling 2 pages. 2 * 64 = 128.
        // 2 * 15 = 30 bits, 30 - 23 = 7 unused. 128 + 7 = 135.
        assert_eq!(program_cost(23, p(15)), cost(2, 128, 7, 135));
    }

    #[test]
    fn exactly_divisible_has_no_unused() {
        // 24 / 6 = 4 exactly. 4 * 64 = 256, 24 - 24 = 0 unused. Penalty 256.
        assert_eq!(program_cost(24, p(6)), cost(4, 256, 0, 256));
        // 30 / 10 = 3 exactly. 3 * 64 = 192, 0 unused. Penalty 192.
        assert_eq!(program_cost(30, p(10)), cost(3, 192, 0, 192));
    }

    #[test]
    fn one_bit_past_a_boundary_costs_a_whole_page() {
        // 11 / 10 = 1.1, ceiling 2 pages. 128 bookkeeping, 20 - 11 = 9 unused.
        // Compare r = 10: 1 page, penalty 64. One extra bit adds 73.
        assert_eq!(program_cost(11, p(10)), cost(2, 128, 9, 137));
        assert_eq!(program_cost(10, p(10)), cost(1, 64, 0, 64));
    }

    #[test]
    fn page_size_one_costs_64_per_bit() {
        // P = 1: every bit is its own page, so pages = r, nothing unused,
        // penalty = 64 * r.
        // r = 23: 23 pages, 23 * 64 = 1472.
        assert_eq!(program_cost(23, p(1)), cost(23, 1472, 0, 1472));
        for r in 1..=500 {
            let c = program_cost(r, p(1));
            assert_eq!(c.unused_bits, 0);
            assert_eq!(c.penalty, 64 * r);
        }
    }

    #[test]
    fn page_at_least_request_is_one_page() {
        // P = r: 1 page, 0 unused, penalty 64.
        assert_eq!(program_cost(23, p(23)), cost(1, 64, 0, 64));
        // P = 100 > 23: 1 page, 100 - 23 = 77 unused, 64 + 77 = 141.
        assert_eq!(program_cost(23, p(100)), cost(1, 64, 77, 141));
        // General rule for P >= r: 1 page, unused = P - r.
        for page in 23..=500 {
            let c = program_cost(23, p(page));
            assert_eq!(c.pages, 1);
            assert_eq!(c.unused_bits, page - 23);
        }
    }

    #[test]
    fn largest_request_in_dataset() {
        // max request is 25,165,824 = 24 * 2^20.
        // P = 1: penalty 64 * 25,165,824 = 1,610,612,736. Checks no overflow.
        assert_eq!(
            program_cost(25_165_824, p(1)),
            cost(25_165_824, 1_610_612_736, 0, 1_610_612_736)
        );
        // P = 2^20 = 1,048,576: exactly 24 pages, 24 * 64 = 1536, 0 unused.
        assert_eq!(
            program_cost(25_165_824, p(1_048_576)),
            cost(24, 1536, 0, 1536)
        );
    }

    #[test]
    fn invariants_hold_without_using_ceil() {
        // Checks the definition of "ceiling" directly instead of trusting
        // div_ceil: pages is the SMALLEST count whose capacity covers r.
        for r in 1..=300 {
            for page in 1..=300 {
                let c = program_cost(r, p(page));
                assert!(c.pages * page >= r, "r={r} P={page}: pages too few");
                assert!((c.pages - 1) * page < r, "r={r} P={page}: pages too many");
                assert!(c.unused_bits < page, "r={r} P={page}: a whole page wasted");
                assert_eq!(c.bookkeeping_bits, 64 * c.pages);
                assert_eq!(c.penalty, c.bookkeeping_bits + c.unused_bits);
            }
        }
    }
}

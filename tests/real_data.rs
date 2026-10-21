//! Regression check on the real dataset (the Phase 0 gate). Runs against
//! data/workloads, which cargo test resolves from the package root.

use std::error::Error;
use std::path::Path;

use page_size::workload::load_workloads;

#[test]
fn real_dataset_matches_phase0_gate() -> Result<(), Box<dyn Error>> {
    let w = load_workloads(Path::new("data/workloads"))?;
    assert!(w.warnings.is_empty(), "{:?}", w.warnings);
    assert_eq!(w.days.len(), 32);
    assert!(w.days.iter().all(|d| d.requests.len() == 128));
    assert_eq!(w.all_requests().count(), 4096);
    assert_eq!(w.days[0].requests[..3], [32294, 4302, 13965]);
    assert_eq!(w.all_requests().min(), Some(29));
    assert_eq!(w.all_requests().max(), Some(25_165_824));
    assert_eq!(w.all_requests().sum::<u64>(), 34_392_541_729);
    Ok(())
}
